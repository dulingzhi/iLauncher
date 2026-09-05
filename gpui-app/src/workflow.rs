//! 工作流引擎：自动化任务编排（对齐 src-tauri/src/plugin/workflow_engine.rs 模型）。
//! 无 gpui 依赖，可单元测试（HttpRequest 走注入的 MockHttpClient）。
//!
//! 相对 Tauri 版的刻意偏离：
//!   - 删死变体：PluginQuery / PluginExecute（Tauri execute_step 落到
//!     `_ => Err("Unsupported action")`，从未实现）
//!   - 副作用收集：CopyToClipboard / ShowNotification 推入 Vec<WorkflowEffect>
//!     由调用层（Launcher/UI）执行——与插件 ExecuteOutcome 同哲学，引擎无
//!     arboard/系统通知依赖、可单测（Tauri 版内联 arboard，ShowNotification
//!     只是 tracing log）
//!   - OpenFile 用 opener crate（替代 cmd /c start 黑窗闪烁）
//!   - TimeRange 真正实现（Tauri 落入 `_ => Ok(true)` 默认放行）；
//!     Expression 保持默认放行（表达式语言 Tauri 版同样不存在）
//!   - Retry 保持 Tauri 语义：全部尝试失败仍继续工作流（注释标记，属上游行为）
//!   - chrono → Unix 秒；Delay 用自写 timer future（无 tokio）；
//!     ProcessRunning 用 Windows Toolhelp32 快照（非 Windows 返回 false）

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use futures::future::BoxFuture;
use futures::AsyncReadExt;
use gpui_kit::http_client::{AsyncBody, HttpClient};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// 工作流定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub trigger: WorkflowTrigger,
    pub steps: Vec<WorkflowStep>,
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub enabled: bool,
    /// Unix 秒（偏离 Tauri chrono::DateTime；旧 Tauri 工作流的时间戳字段
    /// 需手工改为整数秒才能被本引擎加载）
    pub created_at: u64,
    pub updated_at: u64,
}

/// 工作流触发器（serde tag 格式与 Tauri 版一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowTrigger {
    /// 手动触发（关键词精确匹配，启动器唯一有调度设施的触发器；
    /// Hotkey/Schedule/Event 仅存储定义，Tauri 版同样无调度实现）
    Manual { keyword: String },
    /// 热键触发（存储定义，调度设施未实现——Tauri 同）
    Hotkey { key: String },
    /// 定时触发（存储定义，调度设施未实现——Tauri 同）
    Schedule { cron: String },
    /// 事件触发（存储定义，调度设施未实现——Tauri 同）
    Event { event_type: String },
}

/// 工作流步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub action: WorkflowAction,
    #[serde(default)]
    pub condition: Option<WorkflowCondition>,
    #[serde(default)]
    pub on_error: ErrorHandling,
}

/// 工作流动作（PluginQuery/PluginExecute 为 Tauri 死变体，不迁移）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowAction {
    /// 执行命令
    ExecuteCommand { command: String, args: Vec<String>, working_dir: Option<String> },
    /// 打开文件/URL
    OpenFile { path: String },
    /// 复制到剪贴板（副作用：推入 effects，调用层执行）
    CopyToClipboard { content: String },
    /// 显示通知（副作用：推入 effects，调用层执行；Tauri 版仅 tracing log）
    ShowNotification { title: String, message: String },
    /// HTTP 请求
    HttpRequest { method: String, url: String, headers: HashMap<String, String>, body: Option<String> },
    /// 设置变量
    SetVariable { name: String, value: String },
    /// 延迟执行
    Delay { milliseconds: u64 },
    /// 条件分支
    If { condition: WorkflowCondition, then_steps: Vec<WorkflowStep>, else_steps: Option<Vec<WorkflowStep>> },
    /// 循环
    Loop { count: Option<usize>, condition: Option<WorkflowCondition>, steps: Vec<WorkflowStep> },
}

/// 工作流条件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowCondition {
    /// 变量比较
    VariableEquals { name: String, value: String },
    /// 变量包含
    VariableContains { name: String, substring: String },
    /// 文件存在
    FileExists { path: String },
    /// 进程运行中（Windows Toolhelp32 枚举）
    ProcessRunning { name: String },
    /// 时间范围（"HH:MM"-"HH:MM"，本地时间，支持跨午夜）
    TimeRange { start: String, end: String },
    /// 自定义表达式（未实现：恒 true，与 Tauri 版行为一致）
    Expression { expr: String },
    /// 逻辑与
    And { conditions: Vec<WorkflowCondition> },
    /// 逻辑或
    Or { conditions: Vec<WorkflowCondition> },
    /// 逻辑非
    Not { condition: Box<WorkflowCondition> },
}

/// 错误处理策略
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub enum ErrorHandling {
    /// 继续执行
    Continue,
    /// 停止工作流
    #[default]
    Stop,
    /// 重试
    Retry { max_attempts: u32, delay_ms: u64 },
    /// 执行替代步骤
    Fallback { steps: Vec<WorkflowStep> },
}


/// 执行期间收集的副作用（调用层执行：剪贴板写入 / 系统通知）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowEffect {
    CopyToClipboard(String),
    ShowNotification { title: String, message: String },
}

/// 工作流执行上下文
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    pub variables: HashMap<String, serde_json::Value>,
    pub step_outputs: HashMap<String, serde_json::Value>,
}

impl WorkflowContext {
    pub fn new(initial_vars: HashMap<String, serde_json::Value>) -> Self {
        Self { variables: initial_vars, step_outputs: HashMap::new() }
    }

    pub fn get_variable(&self, name: &str) -> Option<&serde_json::Value> {
        self.variables.get(name)
    }

    pub fn set_variable(&mut self, name: String, value: serde_json::Value) {
        self.variables.insert(name, value);
    }

    pub fn set_step_output(&mut self, step_id: String, output: serde_json::Value) {
        self.step_outputs.insert(step_id, output);
    }

    /// 替换字符串中的变量引用 ${var_name}（仅字符串值参与替换，对齐 Tauri）
    pub fn resolve_string(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.variables {
            let placeholder = format!("${{{key}}}");
            if let Some(s) = value.as_str() {
                result = result.replace(&placeholder, s);
            }
        }
        result
    }
}

/// 工作流引擎（同步锁 + 注入 HttpClient）
pub struct WorkflowEngine {
    workflows: RwLock<HashMap<String, Workflow>>,
    storage_path: PathBuf,
    http: Arc<dyn HttpClient>,
}

impl WorkflowEngine {
    pub fn new(storage_path: PathBuf, http: Arc<dyn HttpClient>) -> Self {
        Self { workflows: RwLock::new(HashMap::new()), storage_path, http }
    }

    /// 加载 storage_path 下全部 .json 工作流
    pub fn load_workflows(&self) -> Result<()> {
        if !self.storage_path.exists() {
            std::fs::create_dir_all(&self.storage_path)?;
            return Ok(());
        }
        let mut workflows = self.workflows.write();
        workflows.clear();
        for entry in std::fs::read_dir(&self.storage_path)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let workflow: Workflow = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
                workflows.insert(workflow.id.clone(), workflow);
            }
        }
        Ok(())
    }

    pub fn list_workflows(&self) -> Vec<Workflow> {
        self.workflows.read().values().cloned().collect()
    }

    pub fn get_workflow(&self, id: &str) -> Option<Workflow> {
        self.workflows.read().get(id).cloned()
    }

    /// 保存（内存 + <id>.json 落盘）
    pub fn save_workflow(&self, workflow: Workflow) -> Result<()> {
        self.workflows.write().insert(workflow.id.clone(), workflow.clone());
        std::fs::create_dir_all(&self.storage_path)?;
        let file_path = self.storage_path.join(format!("{}.json", workflow.id));
        std::fs::write(file_path, serde_json::to_string_pretty(&workflow)?)?;
        Ok(())
    }

    /// 删除（内存 + 文件）
    pub fn delete_workflow(&self, id: &str) -> Result<()> {
        self.workflows.write().remove(id);
        let file_path = self.storage_path.join(format!("{id}.json"));
        if file_path.exists() {
            std::fs::remove_file(file_path)?;
        }
        Ok(())
    }

    /// 执行工作流：步骤副作用收集到 effects，返回最终上下文
    pub async fn execute_workflow(
        &self,
        id: &str,
        initial_vars: HashMap<String, serde_json::Value>,
        effects: &mut Vec<WorkflowEffect>,
    ) -> Result<WorkflowContext> {
        let workflow = self.get_workflow(id).ok_or_else(|| anyhow!("Workflow not found: {id}"))?;
        if !workflow.enabled {
            return Err(anyhow!("Workflow is disabled: {id}"));
        }

        let mut context = WorkflowContext::new(initial_vars);
        for step in &workflow.steps {
            if let Err(e) = self.execute_step(step, &mut context, effects).await {
                eprintln!("⚠ 工作流步骤 '{}' 失败: {e:#}", step.name);
                match &step.on_error {
                    ErrorHandling::Continue => continue,
                    ErrorHandling::Stop => return Err(e),
                    ErrorHandling::Retry { max_attempts, delay_ms } => {
                        // 对齐 Tauri 语义：全部尝试失败仍继续工作流（不传播错误）
                        for _ in 1..=*max_attempts {
                            sleep_ms(*delay_ms).await;
                            if self.execute_step(step, &mut context, effects).await.is_ok() {
                                break;
                            }
                        }
                    }
                    ErrorHandling::Fallback { steps } => {
                        for fallback_step in steps {
                            self.execute_step(fallback_step, &mut context, effects).await?;
                        }
                    }
                }
            }
        }
        Ok(context)
    }

    /// 执行单个步骤（BoxFuture：If/Loop 递归）
    fn execute_step<'a>(
        &'a self,
        step: &'a WorkflowStep,
        context: &'a mut WorkflowContext,
        effects: &'a mut Vec<WorkflowEffect>,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            // 条件不满足 → 跳过（Ok，不算错误）
            if let Some(condition) = &step.condition
                && !self.evaluate_condition(condition, context).await? {
                    return Ok(());
                }

            match &step.action {
                WorkflowAction::ExecuteCommand { command, args, working_dir } => {
                    let resolved_command = context.resolve_string(command);
                    let resolved_args: Vec<String> =
                        args.iter().map(|a| context.resolve_string(a)).collect();
                    let mut cmd = std::process::Command::new(&resolved_command);
                    cmd.args(&resolved_args);
                    if let Some(dir) = working_dir {
                        cmd.current_dir(context.resolve_string(dir));
                    }
                    let output = cmd.output()?;
                    context.set_step_output(
                        step.id.clone(),
                        serde_json::json!({
                            "stdout": String::from_utf8_lossy(&output.stdout).to_string(),
                            "stderr": String::from_utf8_lossy(&output.stderr).to_string(),
                            "status": output.status.code(),
                        }),
                    );
                }
                WorkflowAction::OpenFile { path } => {
                    opener::open(context.resolve_string(path))?;
                }
                WorkflowAction::CopyToClipboard { content } => {
                    effects.push(WorkflowEffect::CopyToClipboard(context.resolve_string(content)));
                }
                WorkflowAction::ShowNotification { title, message } => {
                    effects.push(WorkflowEffect::ShowNotification {
                        title: context.resolve_string(title),
                        message: context.resolve_string(message),
                    });
                }
                WorkflowAction::HttpRequest { method, url, headers, body } => {
                    let resolved_url = context.resolve_string(url);
                    let async_body = match body {
                        Some(b) => AsyncBody::from(context.resolve_string(b)),
                        None => AsyncBody::empty(),
                    };
                    let mut request = gpui_kit::http_client::http::Request::builder()
                        .method(match method.as_str() {
                            "GET" => gpui_kit::http_client::http::Method::GET,
                            "POST" => gpui_kit::http_client::http::Method::POST,
                            "PUT" => gpui_kit::http_client::http::Method::PUT,
                            "DELETE" => gpui_kit::http_client::http::Method::DELETE,
                            other => return Err(anyhow!("Unsupported HTTP method: {other}")),
                        })
                        .uri(&resolved_url)
                        .body(async_body)?;
                    for (key, value) in headers {
                        request.headers_mut().insert(
                            gpui_kit::http_client::http::HeaderName::from_bytes(key.as_bytes())?,
                            gpui_kit::http_client::http::HeaderValue::from_str(
                                &context.resolve_string(value),
                            )?,
                        );
                    }
                    let mut resp = self.http.send(request).await?;
                    let status = resp.status().as_u16();
                    let mut bytes = Vec::new();
                    resp.body_mut().read_to_end(&mut bytes).await?;
                    context.set_step_output(
                        step.id.clone(),
                        serde_json::json!({
                            "status": status,
                            "body": String::from_utf8_lossy(&bytes).to_string(),
                        }),
                    );
                }
                WorkflowAction::SetVariable { name, value } => {
                    let resolved = context.resolve_string(value);
                    context.set_variable(name.clone(), serde_json::json!(resolved));
                }
                WorkflowAction::Delay { milliseconds } => {
                    sleep_ms(*milliseconds).await;
                }
                WorkflowAction::If { condition, then_steps, else_steps } => {
                    if self.evaluate_condition(condition, context).await? {
                        for sub in then_steps {
                            self.execute_step(sub, context, effects).await?;
                        }
                    } else if let Some(else_steps) = else_steps {
                        for sub in else_steps {
                            self.execute_step(sub, context, effects).await?;
                        }
                    }
                }
                WorkflowAction::Loop { count, condition, steps } => {
                    if let Some(max_count) = count {
                        for _ in 0..*max_count {
                            if let Some(cond) = condition
                                && !self.evaluate_condition(cond, context).await? {
                                    break;
                                }
                            for sub in steps {
                                self.execute_step(sub, context, effects).await?;
                            }
                        }
                    } else if let Some(cond) = condition {
                        // 无 count 有 condition：循环上限防护（Tauri 版无上限，死循环风险）
                        for _ in 0..10_000 {
                            if !self.evaluate_condition(cond, context).await? {
                                break;
                            }
                            for sub in steps {
                                self.execute_step(sub, context, effects).await?;
                            }
                        }
                    }
                }
            }
            Ok(())
        })
    }

    /// 评估条件（BoxFuture：And/Or/Not 递归）
    fn evaluate_condition<'a>(
        &'a self,
        condition: &'a WorkflowCondition,
        context: &'a WorkflowContext,
    ) -> BoxFuture<'a, Result<bool>> {
        Box::pin(async move {
            match condition {
                WorkflowCondition::VariableEquals { name, value } => Ok(context
                    .get_variable(name)
                    .and_then(|v| v.as_str())
                    .map(|v| v == value)
                    .unwrap_or(false)),
                WorkflowCondition::VariableContains { name, substring } => Ok(context
                    .get_variable(name)
                    .and_then(|v| v.as_str())
                    .map(|v| v.contains(substring.as_str()))
                    .unwrap_or(false)),
                WorkflowCondition::FileExists { path } => {
                    Ok(std::path::Path::new(&context.resolve_string(path)).exists())
                }
                WorkflowCondition::ProcessRunning { name } => Ok(process_running(name)),
                WorkflowCondition::TimeRange { start, end } => {
                    let (start_m, end_m) = parse_time_range(start, end)?;
                    Ok(time_in_range(start_m, end_m, now_local_minutes()))
                }
                // 表达式语言不存在（Tauri 版同）：恒 true
                WorkflowCondition::Expression { .. } => Ok(true),
                WorkflowCondition::And { conditions } => {
                    for cond in conditions {
                        if !self.evaluate_condition(cond, context).await? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                WorkflowCondition::Or { conditions } => {
                    for cond in conditions {
                        if self.evaluate_condition(cond, context).await? {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                WorkflowCondition::Not { condition } => {
                    Ok(!self.evaluate_condition(condition, context).await?)
                }
            }
        })
    }

    /// 关键词精确匹配启用的手动触发工作流（启动器查询路径用）
    pub fn find_by_keyword(&self, keyword: &str) -> Vec<Workflow> {
        self.workflows
            .read()
            .values()
            .filter(|w| {
                w.enabled && matches!(&w.trigger, WorkflowTrigger::Manual { keyword: k } if k == keyword)
            })
            .cloned()
            .collect()
    }
}

/// 解析 "HH:MM" → 分钟数（0..=1439）
fn parse_hhmm(s: &str) -> Result<u16> {
    let (h, m) = s
        .trim()
        .split_once(':')
        .ok_or_else(|| anyhow!("时间格式非法（应为 HH:MM）: {s}"))?;
    let (h, m): (u16, u16) = (h.trim().parse()?, m.trim().parse()?);
    if h > 23 || m > 59 {
        return Err(anyhow!("时间越界: {s}"));
    }
    Ok(h * 60 + m)
}

fn parse_time_range(start: &str, end: &str) -> Result<(u16, u16)> {
    Ok((parse_hhmm(start)?, parse_hhmm(end)?))
}

/// 时间范围判断（start > end 视为跨午夜；纯函数便于测试）
fn time_in_range(start: u16, end: u16, now: u16) -> bool {
    if start <= end {
        (start..=end).contains(&now)
    } else {
        now >= start || now <= end
    }
}

/// 本地当前分钟数（0..=1439）。Windows 走 GetLocalTime；其余平台退回 UTC。
fn now_local_minutes() -> u16 {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::SystemInformation::GetLocalTime;
        let st = unsafe { GetLocalTime() };
        st.wHour as u16 * 60 + st.wMinute as u16
    }
    #[cfg(not(target_os = "windows"))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        ((secs / 60) % 1440) as u16 // UTC 退回（注释：非 Windows 无时区信息）
    }
}

/// 进程名枚举（Windows Toolhelp32 快照；非 Windows 恒 false）
#[cfg(target_os = "windows")]
fn process_running(name: &str) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = match unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
        Ok(s) => s,
        Err(_) => return false,
    };
    let mut found = false;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut has = unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok();
    while has {
        let exe = String::from_utf16_lossy(&entry.szExeFile);
        if exe.to_lowercase().contains(&name.to_lowercase()) {
            found = true;
            break;
        }
        has = unsafe { Process32NextW(snapshot, &mut entry) }.is_ok();
    }
    let _ = unsafe { CloseHandle(snapshot) };
    found
}

#[cfg(not(target_os = "windows"))]
fn process_running(_name: &str) -> bool {
    false // 非 Windows 无进程枚举实现
}

/// 简单 timer future（无 tokio）：首次 poll 挂起并派睡眠线程唤醒
fn sleep_ms(ms: u64) -> impl Future<Output = ()> {
    struct Sleep {
        deadline: Instant,
        armed: bool,
    }
    impl Future for Sleep {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if Instant::now() >= self.deadline {
                return Poll::Ready(());
            }
            if !self.armed {
                self.armed = true;
                let waker = cx.waker().clone();
                let remaining = self.deadline.saturating_duration_since(Instant::now());
                std::thread::spawn(move || {
                    std::thread::sleep(remaining);
                    waker.wake();
                });
            }
            Poll::Pending
        }
    }
    Sleep { deadline: Instant::now() + Duration::from_millis(ms), armed: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{self, MockHttp};

    fn engine(tag: &str, http: Arc<dyn HttpClient>) -> WorkflowEngine {
        WorkflowEngine::new(test_util::tempdir(tag), http)
    }

    fn wf(id: &str, keyword: &str, steps: Vec<WorkflowStep>) -> Workflow {
        Workflow {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            trigger: WorkflowTrigger::Manual { keyword: keyword.to_string() },
            steps,
            variables: HashMap::new(),
            enabled: true,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn step(id: &str, action: WorkflowAction) -> WorkflowStep {
        WorkflowStep { id: id.into(), name: id.into(), action, condition: None, on_error: ErrorHandling::Stop }
    }

    #[test]
    fn resolve_string_replaces_placeholders() {
        let mut ctx = WorkflowContext::new(HashMap::new());
        ctx.set_variable("name".into(), serde_json::json!("世界"));
        ctx.set_variable("num".into(), serde_json::json!(42)); // 非字符串不参与
        assert_eq!(ctx.resolve_string("你好 ${name} ${num}"), "你好 世界 ${num}");
    }

    #[test]
    fn persistence_roundtrip() {
        let e = engine("persist", MockHttp::arc(&[], 200));
        e.save_workflow(wf("w1", "go", vec![])).unwrap();
        // 新引擎从同一目录加载（模拟重启）
        let e2 = WorkflowEngine::new(e.storage_path.clone(), MockHttp::arc(&[], 200));
        e2.load_workflows().unwrap();
        assert_eq!(e2.list_workflows().len(), 1);
        e2.delete_workflow("w1").unwrap();
        assert!(e2.list_workflows().is_empty());
        assert!(!e.storage_path.join("w1.json").exists());
        let _ = std::fs::remove_dir_all(&e.storage_path);
    }

    #[test]
    fn find_by_keyword_requires_enabled_manual() {
        let e = engine("kw", MockHttp::arc(&[], 200));
        e.save_workflow(wf("a", "deploy", vec![])).unwrap();
        let mut disabled = wf("b", "deploy", vec![]);
        disabled.enabled = false;
        e.save_workflow(disabled).unwrap();
        let mut hotkey = wf("c", "deploy", vec![]);
        hotkey.trigger = WorkflowTrigger::Hotkey { key: "F9".into() };
        e.save_workflow(hotkey).unwrap();

        let hits = e.find_by_keyword("deploy");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "a");
        assert!(e.find_by_keyword("other").is_empty());
        let _ = std::fs::remove_dir_all(&e.storage_path);
    }

    #[test]
    fn execution_set_variable_and_condition_skip() {
        let e = engine("exec", MockHttp::arc(&[], 200));
        e.save_workflow(wf("w", "go", vec![
            step("s1", WorkflowAction::SetVariable { name: "env".into(), value: "prod".into() }),
            WorkflowStep {
                id: "s2".into(),
                name: "s2".into(),
                action: WorkflowAction::SetVariable { name: "marker".into(), value: "yes".into() },
                condition: Some(WorkflowCondition::VariableEquals { name: "env".into(), value: "prod".into() }),
                on_error: ErrorHandling::Stop,
            },
            WorkflowStep {
                id: "s3".into(),
                name: "s3".into(),
                action: WorkflowAction::SetVariable { name: "marker2".into(), value: "yes".into() },
                condition: Some(WorkflowCondition::VariableEquals { name: "env".into(), value: "dev".into() }),
                on_error: ErrorHandling::Stop,
            },
        ]))
        .unwrap();

        let mut effects = Vec::new();
        let ctx = futures::executor::block_on(e.execute_workflow("w", HashMap::new(), &mut effects)).unwrap();
        assert_eq!(ctx.get_variable("marker").unwrap().as_str().unwrap(), "yes");
        assert!(ctx.get_variable("marker2").is_none(), "条件不满足的步骤应跳过");
    }

    #[test]
    fn disabled_workflow_rejected() {
        let e = engine("disabled", MockHttp::arc(&[], 200));
        let mut w = wf("w", "go", vec![]);
        w.enabled = false;
        e.save_workflow(w).unwrap();
        let mut effects = Vec::new();
        assert!(futures::executor::block_on(e.execute_workflow("w", HashMap::new(), &mut effects)).is_err());
        let _ = std::fs::remove_dir_all(&e.storage_path);
    }

    #[test]
    fn error_handling_continue_and_fallback() {
        let failing = step("boom", WorkflowAction::ExecuteCommand {
            command: "definitely_not_a_real_binary_ilauncher".into(),
            args: vec![],
            working_dir: None,
        });
        // Continue：失败后继续
        let e = engine("cont", MockHttp::arc(&[], 200));
        e.save_workflow(wf("w", "go", vec![
            WorkflowStep { on_error: ErrorHandling::Continue, ..failing.clone() },
            step("after", WorkflowAction::SetVariable { name: "after".into(), value: "1".into() }),
        ]))
        .unwrap();
        let mut effects = Vec::new();
        let ctx = futures::executor::block_on(e.execute_workflow("w", HashMap::new(), &mut effects)).unwrap();
        assert_eq!(ctx.get_variable("after").unwrap(), &serde_json::json!("1"));

        // Fallback：失败后执行替代步骤
        let e2 = engine("fb", MockHttp::arc(&[], 200));
        e2.save_workflow(wf("w", "go", vec![WorkflowStep {
            on_error: ErrorHandling::Fallback {
                steps: vec![step("fb", WorkflowAction::SetVariable { name: "fb".into(), value: "ran".into() })],
            },
            ..failing.clone()
        }]))
        .unwrap();
        let mut effects2 = Vec::new();
        let ctx2 = futures::executor::block_on(e2.execute_workflow("w", HashMap::new(), &mut effects2)).unwrap();
        assert_eq!(ctx2.get_variable("fb").unwrap().as_str().unwrap(), "ran");

        // Stop：失败即整体失败
        let e3 = engine("stop", MockHttp::arc(&[], 200));
        e3.save_workflow(wf("w", "go", vec![failing.clone()])).unwrap();
        let mut effects3 = Vec::new();
        assert!(futures::executor::block_on(e3.execute_workflow("w", HashMap::new(), &mut effects3)).is_err());
    }

    #[test]
    fn if_and_loop_structures() {
        let e = engine("ifloop", MockHttp::arc(&[], 200));
        e.save_workflow(wf("w", "go", vec![
            step("init", WorkflowAction::SetVariable { name: "ran".into(), value: "no".into() }),
            WorkflowStep {
                id: "if".into(),
                name: "if".into(),
                action: WorkflowAction::If {
                    condition: WorkflowCondition::VariableEquals { name: "ran".into(), value: "no".into() },
                    then_steps: vec![step("t", WorkflowAction::SetVariable { name: "ran".into(), value: "then".into() })],
                    else_steps: Some(vec![step("e", WorkflowAction::SetVariable { name: "ran".into(), value: "else".into() })]),
                },
                condition: None,
                on_error: ErrorHandling::Stop,
            },
            WorkflowStep {
                id: "loop".into(),
                name: "loop".into(),
                action: WorkflowAction::Loop {
                    count: Some(2),
                    condition: None,
                    steps: vec![step("d", WorkflowAction::Delay { milliseconds: 0 })],
                },
                condition: None,
                on_error: ErrorHandling::Stop,
            },
        ]))
        .unwrap();
        let mut effects = Vec::new();
        let ctx = futures::executor::block_on(e.execute_workflow("w", HashMap::new(), &mut effects)).unwrap();
        assert_eq!(ctx.get_variable("ran").unwrap().as_str().unwrap(), "then");
    }

    #[test]
    fn clipboard_and_notification_effects_collected() {
        let e = engine("effects", MockHttp::arc(&[], 200));
        e.save_workflow(wf("w", "go", vec![
            step("v", WorkflowAction::SetVariable { name: "x".into(), value: "42".into() }),
            step("c", WorkflowAction::CopyToClipboard { content: "值=${x}".into() }),
            step("n", WorkflowAction::ShowNotification { title: "完成".into(), message: "${x}".into() }),
        ]))
        .unwrap();
        let mut effects = Vec::new();
        futures::executor::block_on(e.execute_workflow("w", HashMap::new(), &mut effects)).unwrap();
        assert_eq!(
            effects,
            vec![
                WorkflowEffect::CopyToClipboard("值=42".into()),
                WorkflowEffect::ShowNotification { title: "完成".into(), message: "42".into() },
            ]
        );
    }

    #[test]
    fn http_request_step_with_mock() {
        let e = engine("http", MockHttp::arc(br#"{"ok":true}"#, 200));
        e.save_workflow(wf("w", "go", vec![step(
            "req",
            WorkflowAction::HttpRequest {
                method: "GET".into(),
                url: "https://api.example.com/ping".into(),
                headers: HashMap::new(),
                body: None,
            },
        )]))
        .unwrap();
        let mut effects = Vec::new();
        let ctx = futures::executor::block_on(e.execute_workflow("w", HashMap::new(), &mut effects)).unwrap();
        let out = ctx.step_outputs.get("req").unwrap();
        assert_eq!(out["status"], serde_json::json!(200));
        assert_eq!(out["body"], serde_json::json!(r#"{"ok":true}"#));
    }

    #[test]
    fn conditions_logic_composition() {
        let e = engine("conds", MockHttp::arc(&[], 200));
        let ctx = WorkflowContext::new(HashMap::from([("s".to_string(), serde_json::json!("hello world"))]));
        let cond = |c: &WorkflowCondition| {
            futures::executor::block_on(e.evaluate_condition(c, &ctx)).expect("条件求值")
        };
        assert!(cond(&WorkflowCondition::VariableContains { name: "s".into(), substring: "world".into() }));
        assert!(!cond(&WorkflowCondition::VariableEquals { name: "s".into(), value: "no".into() }));
        assert!(cond(&WorkflowCondition::And {
            conditions: vec![
                WorkflowCondition::VariableContains { name: "s".into(), substring: "hello".into() },
                WorkflowCondition::VariableContains { name: "s".into(), substring: "world".into() },
            ],
        }));
        assert!(cond(&WorkflowCondition::Not {
            condition: Box::new(WorkflowCondition::VariableEquals { name: "s".into(), value: "no".into() }),
        }));
        assert!(!cond(&WorkflowCondition::Or {
            conditions: vec![
                WorkflowCondition::VariableEquals { name: "s".into(), value: "no".into() },
                WorkflowCondition::FileExists { path: "definitely_missing_file_ilauncher.txt".into() },
            ],
        }));
        assert!(cond(&WorkflowCondition::Expression { expr: "anything".into() }));
    }

    #[test]
    fn time_in_range_logic() {
        assert!(time_in_range(540, 1080, 700)); // 同日 09:00-18:00, 11:40
        assert!(!time_in_range(540, 1080, 1200)); // 20:00 在外
        assert!(time_in_range(1320, 240, 1380), "跨午夜: 22:00-04:00, 23:00");
        assert!(time_in_range(1320, 240, 120), "跨午夜: 02:00 在内");
        assert!(!time_in_range(1320, 240, 700), "跨午夜: 11:40 在外");
        // 边界包含
        assert!(time_in_range(540, 540, 540));
    }

    #[test]
    fn parse_hhmm_validation() {
        assert_eq!(parse_hhmm("09:30").unwrap(), 570);
        assert_eq!(parse_hhmm("0:05").unwrap(), 5);
        assert!(parse_hhmm("24:00").is_err());
        assert!(parse_hhmm("12:60").is_err());
        assert!(parse_hhmm("noon").is_err());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn process_running_finds_self() {
        // 当前测试进程必在快照里（名字取 exe 文件名主干）
        let exe = std::env::current_exe().unwrap();
        let stem = exe.file_stem().unwrap().to_string_lossy().to_string();
        assert!(process_running(&stem), "应能在进程快照中找到自身（{stem}）");
        assert!(!process_running("definitely_not_running_ilauncher_xyz.exe"));
    }
}
