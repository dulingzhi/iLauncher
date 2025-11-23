// 工作流引擎 - 自动化任务编排系统
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 工作流触发器
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowTrigger {
    /// 手动触发（快捷指令）
    Manual { keyword: String },
    /// 热键触发
    Hotkey { key: String },
    /// 定时触发
    Schedule { cron: String },
    /// 事件触发
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

/// 工作流动作
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowAction {
    /// 执行命令
    ExecuteCommand {
        command: String,
        args: Vec<String>,
        working_dir: Option<String>,
    },
    /// 打开文件/URL
    OpenFile {
        path: String,
    },
    /// 复制到剪贴板
    CopyToClipboard {
        content: String,
    },
    /// 显示通知
    ShowNotification {
        title: String,
        message: String,
    },
    /// HTTP 请求
    HttpRequest {
        method: String,
        url: String,
        headers: HashMap<String, String>,
        body: Option<String>,
    },
    /// 插件查询
    PluginQuery {
        plugin_id: String,
        query: String,
    },
    /// 插件执行
    PluginExecute {
        plugin_id: String,
        result_id: String,
        action_id: String,
    },
    /// 设置变量
    SetVariable {
        name: String,
        value: String,
    },
    /// 延迟执行
    Delay {
        milliseconds: u64,
    },
    /// 条件分支
    If {
        condition: WorkflowCondition,
        then_steps: Vec<WorkflowStep>,
        else_steps: Option<Vec<WorkflowStep>>,
    },
    /// 循环
    Loop {
        count: Option<usize>,
        condition: Option<WorkflowCondition>,
        steps: Vec<WorkflowStep>,
    },
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
    /// 进程运行中
    ProcessRunning { name: String },
    /// 时间范围
    TimeRange { start: String, end: String },
    /// 自定义表达式
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
pub enum ErrorHandling {
    /// 继续执行
    Continue,
    /// 停止工作流
    Stop,
    /// 重试
    Retry { max_attempts: u32, delay_ms: u64 },
    /// 执行替代步骤
    Fallback { steps: Vec<WorkflowStep> },
}

impl Default for ErrorHandling {
    fn default() -> Self {
        ErrorHandling::Stop
    }
}

/// 工作流执行上下文
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    pub variables: HashMap<String, serde_json::Value>,
    pub step_outputs: HashMap<String, serde_json::Value>,
}

impl WorkflowContext {
    pub fn new(initial_vars: HashMap<String, serde_json::Value>) -> Self {
        Self {
            variables: initial_vars,
            step_outputs: HashMap::new(),
        }
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

    /// 替换字符串中的变量引用 ${var_name}
    pub fn resolve_string(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.variables {
            let placeholder = format!("${{{}}}", key);
            if let Some(s) = value.as_str() {
                result = result.replace(&placeholder, s);
            }
        }
        result
    }
}

/// 工作流引擎
pub struct WorkflowEngine {
    workflows: Arc<RwLock<HashMap<String, Workflow>>>,
    storage_path: std::path::PathBuf,
}

impl WorkflowEngine {
    pub fn new(storage_path: std::path::PathBuf) -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            storage_path,
        }
    }

    /// 加载所有工作流
    pub async fn load_workflows(&self) -> Result<()> {
        if !self.storage_path.exists() {
            std::fs::create_dir_all(&self.storage_path)?;
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.storage_path)?;
        let mut workflows = self.workflows.write().await;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = tokio::fs::read_to_string(&path).await?;
                let workflow: Workflow = serde_json::from_str(&content)?;
                workflows.insert(workflow.id.clone(), workflow);
            }
        }

        tracing::info!("Loaded {} workflows", workflows.len());
        Ok(())
    }

    /// 获取所有工作流
    pub async fn list_workflows(&self) -> Vec<Workflow> {
        self.workflows.read().await.values().cloned().collect()
    }

    /// 获取单个工作流
    pub async fn get_workflow(&self, id: &str) -> Option<Workflow> {
        self.workflows.read().await.get(id).cloned()
    }

    /// 保存工作流
    pub async fn save_workflow(&self, workflow: Workflow) -> Result<()> {
        // 更新内存
        let mut workflows = self.workflows.write().await;
        workflows.insert(workflow.id.clone(), workflow.clone());

        // 持久化到文件
        std::fs::create_dir_all(&self.storage_path)?;
        let file_path = self.storage_path.join(format!("{}.json", workflow.id));
        let content = serde_json::to_string_pretty(&workflow)?;
        tokio::fs::write(file_path, content).await?;

        tracing::info!("Saved workflow: {}", workflow.name);
        Ok(())
    }

    /// 删除工作流
    pub async fn delete_workflow(&self, id: &str) -> Result<()> {
        // 从内存删除
        self.workflows.write().await.remove(id);

        // 删除文件
        let file_path = self.storage_path.join(format!("{}.json", id));
        if file_path.exists() {
            tokio::fs::remove_file(file_path).await?;
        }

        tracing::info!("Deleted workflow: {}", id);
        Ok(())
    }

    /// 执行工作流
    pub async fn execute_workflow(&self, id: &str, initial_vars: HashMap<String, serde_json::Value>) -> Result<WorkflowContext> {
        let workflow = self.get_workflow(id).await
            .ok_or_else(|| anyhow!("Workflow not found: {}", id))?;

        if !workflow.enabled {
            return Err(anyhow!("Workflow is disabled: {}", id));
        }

        tracing::info!("Executing workflow: {}", workflow.name);
        let mut context = WorkflowContext::new(initial_vars);

        for step in &workflow.steps {
            if let Err(e) = self.execute_step(step, &mut context).await {
                tracing::error!("Step '{}' failed: {}", step.name, e);
                match &step.on_error {
                    ErrorHandling::Continue => continue,
                    ErrorHandling::Stop => return Err(e),
                    ErrorHandling::Retry { max_attempts, delay_ms } => {
                        for attempt in 1..=*max_attempts {
                            tracing::info!("Retrying step '{}' (attempt {}/{})", step.name, attempt, max_attempts);
                            tokio::time::sleep(tokio::time::Duration::from_millis(*delay_ms)).await;
                            if self.execute_step(step, &mut context).await.is_ok() {
                                break;
                            }
                        }
                    }
                    ErrorHandling::Fallback { steps } => {
                        for fallback_step in steps {
                            self.execute_step(fallback_step, &mut context).await?;
                        }
                    }
                }
            }
        }

        tracing::info!("Workflow '{}' completed successfully", workflow.name);
        Ok(context)
    }

    /// 执行单个步骤
    fn execute_step<'a>(
        &'a self,
        step: &'a WorkflowStep,
        context: &'a mut WorkflowContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            // 检查条件
            if let Some(condition) = &step.condition {
                if !self.evaluate_condition(condition, context).await? {
                    tracing::debug!("Step '{}' skipped (condition not met)", step.name);
                    return Ok(());
                }
            }

        tracing::debug!("Executing step: {}", step.name);

        match &step.action {
            WorkflowAction::ExecuteCommand { command, args, working_dir } => {
                let resolved_command = context.resolve_string(command);
                let resolved_args: Vec<String> = args.iter().map(|a| context.resolve_string(a)).collect();
                
                let mut cmd = std::process::Command::new(&resolved_command);
                cmd.args(&resolved_args);
                if let Some(dir) = working_dir {
                    cmd.current_dir(context.resolve_string(dir));
                }
                
                let output = cmd.output()?;
                context.set_step_output(step.id.clone(), serde_json::json!({
                    "stdout": String::from_utf8_lossy(&output.stdout).to_string(),
                    "stderr": String::from_utf8_lossy(&output.stderr).to_string(),
                    "status": output.status.code(),
                }));
            }
            WorkflowAction::OpenFile { path } => {
                let resolved_path = context.resolve_string(path);
                #[cfg(target_os = "windows")]
                std::process::Command::new("cmd")
                    .args(["/c", "start", "", &resolved_path])
                    .spawn()?;
                #[cfg(target_os = "macos")]
                std::process::Command::new("open")
                    .arg(&resolved_path)
                    .spawn()?;
                #[cfg(target_os = "linux")]
                std::process::Command::new("xdg-open")
                    .arg(&resolved_path)
                    .spawn()?;
            }
            WorkflowAction::CopyToClipboard { content } => {
                let resolved_content = context.resolve_string(content);
                use arboard::Clipboard;
                let mut clipboard = Clipboard::new()?;
                clipboard.set_text(&resolved_content)?;
            }
            WorkflowAction::ShowNotification { title, message } => {
                let resolved_title = context.resolve_string(title);
                let resolved_message = context.resolve_string(message);
                tracing::info!("Notification: {} - {}", resolved_title, resolved_message);
                // TODO: 实现系统通知
            }
            WorkflowAction::HttpRequest { method, url, headers, body } => {
                let client = reqwest::Client::new();
                let resolved_url = context.resolve_string(url);
                let mut request = match method.as_str() {
                    "GET" => client.get(&resolved_url),
                    "POST" => client.post(&resolved_url),
                    "PUT" => client.put(&resolved_url),
                    "DELETE" => client.delete(&resolved_url),
                    _ => return Err(anyhow!("Unsupported HTTP method: {}", method)),
                };

                for (key, value) in headers {
                    request = request.header(key, context.resolve_string(value));
                }

                if let Some(body_content) = body {
                    request = request.body(context.resolve_string(body_content));
                }

                let response = request.send().await?;
                let status = response.status().as_u16();
                let body = response.text().await?;

                context.set_step_output(step.id.clone(), serde_json::json!({
                    "status": status,
                    "body": body,
                }));
            }
            WorkflowAction::SetVariable { name, value } => {
                let resolved_value = context.resolve_string(value);
                context.set_variable(name.clone(), serde_json::json!(resolved_value));
            }
            WorkflowAction::Delay { milliseconds } => {
                tokio::time::sleep(tokio::time::Duration::from_millis(*milliseconds)).await;
            }
            WorkflowAction::If { condition, then_steps, else_steps } => {
                if self.evaluate_condition(condition, context).await? {
                    for sub_step in then_steps {
                        self.execute_step(sub_step, context).await?;
                    }
                } else if let Some(else_steps) = else_steps {
                    for sub_step in else_steps {
                        self.execute_step(sub_step, context).await?;
                    }
                }
            }
            WorkflowAction::Loop { count, condition, steps } => {
                if let Some(max_count) = count {
                    for _ in 0..*max_count {
                        if let Some(cond) = condition {
                            if !self.evaluate_condition(cond, context).await? {
                                break;
                            }
                        }
                        for sub_step in steps {
                            self.execute_step(sub_step, context).await?;
                        }
                    }
                } else if let Some(cond) = condition {
                    while self.evaluate_condition(cond, context).await? {
                        for sub_step in steps {
                            self.execute_step(sub_step, context).await?;
                        }
                    }
                }
            }
            _ => {
                return Err(anyhow!("Unsupported action: {:?}", step.action));
            }
        }

        Ok(())
        })
    }

    /// 评估条件
    fn evaluate_condition<'a>(
        &'a self,
        condition: &'a WorkflowCondition,
        context: &'a WorkflowContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move {
            match condition {
            WorkflowCondition::VariableEquals { name, value } => {
                Ok(context.get_variable(name)
                    .and_then(|v| v.as_str())
                    .map(|v| v == value)
                    .unwrap_or(false))
            }
            WorkflowCondition::VariableContains { name, substring } => {
                Ok(context.get_variable(name)
                    .and_then(|v| v.as_str())
                    .map(|v| v.contains(substring))
                    .unwrap_or(false))
            }
            WorkflowCondition::FileExists { path } => {
                let resolved_path = context.resolve_string(path);
                Ok(std::path::Path::new(&resolved_path).exists())
            }
            WorkflowCondition::ProcessRunning { name } => {
                use sysinfo::System;
                let mut sys = System::new_all();
                sys.refresh_all();
                Ok(sys.processes().values().any(|p| {
                    p.name().to_string_lossy().contains(name)
                }))
            }
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
            _ => Ok(true), // 未实现的条件默认为 true
            }
        })
    }

    /// 查找可以通过关键词触发的工作流
    pub async fn find_by_keyword(&self, keyword: &str) -> Vec<Workflow> {
        self.workflows
            .read()
            .await
            .values()
            .filter(|w| {
                w.enabled && matches!(&w.trigger, WorkflowTrigger::Manual { keyword: k } if k == keyword)
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_execution() {
        let engine = WorkflowEngine::new(std::path::PathBuf::from("test_workflows"));
        
        let workflow = Workflow {
            id: "test".to_string(),
            name: "Test Workflow".to_string(),
            description: "A test workflow".to_string(),
            trigger: WorkflowTrigger::Manual { keyword: "test".to_string() },
            steps: vec![
                WorkflowStep {
                    id: "step1".to_string(),
                    name: "Set Variable".to_string(),
                    action: WorkflowAction::SetVariable {
                        name: "greeting".to_string(),
                        value: "Hello, World!".to_string(),
                    },
                    condition: None,
                    on_error: ErrorHandling::Stop,
                },
            ],
            variables: HashMap::new(),
            enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        engine.save_workflow(workflow).await.unwrap();
        let result = engine.execute_workflow("test", HashMap::new()).await.unwrap();
        
        assert_eq!(result.get_variable("greeting").unwrap().as_str().unwrap(), "Hello, World!");
    }
}
