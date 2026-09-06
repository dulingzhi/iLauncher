// iLauncher GPUI P1 主循环原型
//
// 架构：Watcher 实体统一持有窗口生命周期
//   - Esc → window.remove_window() 销毁窗口（隐藏）
//   - Ctrl+Space 热键 / 托盘"显示" → Watcher 轮询通道，前台激活或重建窗口
//   - 输入防抖 80ms 后搜索（单字符查询 33ms 不掉帧）
//   - ↑↓ 选择、Enter 启动（opener）、托盘"退出"
//
// 用法：
//   ilauncher-gpui                      Demo 数据运行（自包含，无需外部数据）
//   ILAUNCHER_SNAPSHOT=<path> ilauncher-gpui   真实快照搜索（需 --features ilauncher 构建）
//   ilauncher-gpui --bench              列表滚动帧率基准
//   ilauncher-gpui --snapshot <path>    LiveIndex 进程内搜索基准（需 feature ilauncher）
//   ilauncher-gpui --dump-icon <path>   导出 128×128 RGBA 品牌像素（生成 exe 图标用）

// release 用 GUI 子系统：双击启动不再弹控制台窗口（println! 在无 stdout 时
// 被 Rust std 静默丢弃；重定向到文件/管道不受影响，冒烟日志照常用）。
// debug 构建保留控制台，方便开发期看日志。
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod audit;
mod autostart;
mod http_util;
mod i18n;
mod tray_icon_gen;
mod window_drag;
mod plugin;
mod preview;
mod preview_ui;
mod search;
mod settings;
mod skins;
mod ai;
mod markdown;

// i18n 文案嵌入（必须在 crate 根：t! 展开引用 crate::_rust_i18n_t）
rust_i18n::i18n!("locales", fallback = "zh-CN");

#[cfg(test)]
mod test_util;

#[cfg(windows)]
mod ai_ui;
#[cfg(windows)]
mod settings_ui;
mod updater;

#[cfg(windows)]
mod audit_ui;
#[cfg(windows)]
mod plugin_ui;
#[cfg(all(feature = "clipboard", target_os = "windows"))]
mod clipboard_ui;

#[cfg(all(feature = "ilauncher", target_os = "windows"))]
mod index_service;
#[cfg(target_os = "windows")]
mod workflow;
#[cfg(target_os = "windows")]
mod workflow_ui;

use std::sync::mpsc;
use std::time::Instant;

use gpui_kit::assets::Assets;
use gpui_kit::component::list::ListItem;
use gpui_kit::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
    *,
};
use gpui_kit::*;
use search::{Entry, LiveSet, SearchSource};

#[cfg(all(feature = "clipboard", target_os = "windows"))]
use ilauncher_clipboard::ClipboardStore;
use parking_lot::Mutex;
use std::sync::Arc;

const DEMO_COUNT: usize = 100_000;
const PAGE_LIMIT: usize = 50;
/// 输入防抖：暂停输入这么久后才真正执行搜索
const DEBOUNCE_MS: u64 = 80;
/// 预览防抖：方向键连按时不重复读盘，停止 120ms 后才读
const PREVIEW_DEBOUNCE_MS: u64 = 120;
/// 唤起信号轮询周期（热键 → 窗口激活的附加延迟 ≤ 该值）
const SIGNAL_POLL_MS: u64 = 16;

// ── 全局启动时刻（冷启动测量） ────────────────────────────────────────────────

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn start_time() -> Instant {
    *START.get_or_init(Instant::now)
}

fn demo_entries() -> Vec<Entry> {
    let words = ["report", "notes", "简历", "配置", "相册", "terminal", "浏览器", "计算器"];
    (0..DEMO_COUNT)
        .map(|i| Entry::new(format!("{}_{:06}.txt", words[i % words.len()], i), format!("C:\\demo\\{}_{:06}.txt", words[i % words.len()], i)))
        .collect()
}

// ── 唤起信号（热键 / 托盘菜单共用） ──────────────────────────────────────────

enum AppSignal {
    /// 唤起窗口（Instant 为按下时刻，用于测量唤起延迟）
    Show(Instant),
    /// 托盘"重建索引"：提权全量重扫 + 自动重载
    RebuildIndex,
    /// 托盘"剪贴板历史"：打开/激活历史窗口
    ShowClipboard,
    /// 托盘"审计日志"：打开/激活审计查看器
    ShowAudit,
    /// 托盘"插件"：打开/激活插件市场/管理窗口
    ShowPlugins,
    /// 托盘"工作流"：打开/激活工作流管理窗口
    ShowWorkflows,
    /// 托盘"AI 助手"：打开/激活 AI 对话窗口
    ShowAi,
    /// 托盘"设置"：打开/激活设置窗口（窗口逻辑仅 Windows 编译）
    ShowSettings,
    /// 托盘"深色主题"：切换主题模式（true = 深色）
    SetTheme(bool),
    /// Alt+P：切换文件预览窗口（独立窗口，贴主窗口右边框）
    TogglePreview,
    /// 主窗口 bounds 变化：预览窗口重新贴靠（gpui-pre 无窗口移动 API，重建实现）
    RepositionPreview,
}

// ── 主视图 ──────────────────────────────────────────────────────────────────

struct Launcher {
    input: Entity<InputState>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    source: SearchSource,
    entries: std::rc::Rc<Vec<Entry>>,
    selected: usize,
    query_gen: usize,
    first_render_done: Option<Instant>,
    render_count: usize,
    render_total_ms: f64,
    frame_count: usize,
    bench_t0: Option<Instant>,
    bench: bool,
    bench_tick: usize,
    /// 预览：路径 + 读取结果（Err 为展示用错误文本）
    preview: Option<(std::path::PathBuf, Result<preview::FilePreview, String>)>,
    preview_gen: usize,
    /// Lua 命令反馈文本（底部状态栏左侧展示；下次输入清空）
    notice: Option<String>,
    /// 审计日志（启动文件记 ProgramExecution；插件沙盒事件经 plugin 模块同管道写入）
    audit_logger: Arc<Mutex<audit::AuditLogger>>,
    /// 插件管理器（搜索扇出 + 执行分发；沙盒权限检查写审计）
    plugins: Arc<plugin::PluginManager>,
    /// 向 WindowGuard 发信号（Alt+P 预览切换 / 设置入口 / 预览重贴靠）
    tx: mpsc::Sender<AppSignal>,
    /// 预览窗口是否打开（WindowGuard 切换时回写；关闭时跳过预览读盘）
    preview_open: bool,
    /// 工作流引擎（关键词精确匹配触发 + 后台执行）
    #[cfg(target_os = "windows")]
    workflows: Arc<workflow::WorkflowEngine>,
    _subscriptions: Vec<Subscription>,
}

impl Launcher {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        source: SearchSource,
        audit_logger: Arc<Mutex<audit::AuditLogger>>,
        plugins: Arc<plugin::PluginManager>,
        tx: mpsc::Sender<AppSignal>,
        #[cfg(target_os = "windows")] workflows: Arc<workflow::WorkflowEngine>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::i18n::t!("main.placeholder").to_string()));
        let focus = cx.focus_handle();

        // 失去焦点自动隐藏（启动器惯例）：销毁窗口，唤起时由 WindowGuard 重建。
        // on_focus_lost 只在「焦点从有变无（元素被移除）」时触发，窗口 deactivate
        // （点别的应用）不触发，必须用 observe_window_activation。
        // 订阅必须持有（ dropped 即失效），挂到 _subscriptions。
        // ILAUNCHER_NO_AUTOHIDE=1 时禁用（自动化冒烟用，窗口不随失焦销毁）
        let focus_lost_sub = if std::env::var("ILAUNCHER_NO_AUTOHIDE").is_err() {
            Some(cx.observe_window_activation(window, |_, window, _| {
                if !window.is_window_active() {
                    window.remove_window();
                }
            }))
        } else {
            None
        };
        let bench = std::env::args().any(|a| a == "--bench");
        // bench 模式保持 10 万条全量以测虚拟列表；正常运行空查询显示空（启动器惯例）
        let entries = if bench { std::rc::Rc::new(demo_entries()) } else { std::rc::Rc::new(Vec::new()) };
        let tx_bounds = tx.clone();

        let mut this = Self {
            input: input.clone(),
            scroll: UniformListScrollHandle::new(),
            focus,
            source,
            entries,
            selected: 0,
            query_gen: 0,
            first_render_done: None,
            render_count: 0,
            render_total_ms: 0.0,
            frame_count: 0,
            bench_t0: None,
            bench,
            bench_tick: 0,
            preview: None,
            preview_gen: 0,
            notice: None,
            audit_logger,
            plugins,
            tx,
            preview_open: false,
            #[cfg(target_os = "windows")]
            workflows,
            _subscriptions: Vec::new(),
        };
        if this.bench {
            this.run_bench(window, cx);
        }

        this._subscriptions.push(cx.subscribe_in(&input, window, {
            let input = input.clone();
            move |this, _, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    this.apply_query_debounced(value, cx);
                }
            }
        }));
        if let Some(sub) = focus_lost_sub {
            this._subscriptions.push(sub);
        }
        // 主窗口移动/缩放 → 预览窗口重新贴靠（gpui-pre 无窗口移动 API，
        // 由 WindowGuard 重建预览窗口实现"跟随"）
        this._subscriptions.push(cx.observe_window_bounds(window, move |_, _, _| {
            let _ = tx_bounds.send(AppSignal::RepositionPreview);
        }));
        // 开发调试：ILAUNCHER_DEV_QUERY 预填查询并立即搜索（自动化截图/冒烟用）
        if let Ok(q) = std::env::var("ILAUNCHER_DEV_QUERY") {
            if !q.is_empty() && !this.bench {
                this.input.update(cx, |state, cx| state.set_value(q.clone(), window, cx));
                this.apply_query(q, cx);
            }
        }
        this
    }

    /// 预览窗口镜像状态（preview_ui 消费）
    pub(crate) fn preview_state(&self) -> PreviewState {
        let entry = self
            .entries
            .get(self.selected)
            .filter(|e| e.origin == search::EntryOrigin::File)
            .map(|e| (e.name.clone(), e.path.clone()));
        PreviewState {
            entry,
            preview: self.preview.clone(),
        }
    }

    /// WindowGuard 切换预览窗口时回写
    pub(crate) fn set_preview_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.preview_open = open;
        if !open {
            self.preview = None;
        }
        cx.notify();
    }

    /// 输入防抖：只执行最新一代查询，暂停输入 DEBOUNCE_MS 后真正搜索
    fn apply_query_debounced(&mut self, query: String, cx: &mut Context<Self>) {
        self.query_gen = self.query_gen.wrapping_add(1);
        let gen_id = self.query_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(DEBOUNCE_MS))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.query_gen == gen_id {
                    this.apply_query(query, cx);
                }
            });
        })
        .detach();
    }

    /// 防抖后到期的真正搜索：空查询清空结果，非空最多 PAGE_LIMIT 条。
    /// 排序：前缀命令（Lua 命令插件，COMMAND_SCORE）置顶 → 文件结果 →
    /// 其余插件结果按 score 降序 → 工作流关键词精确匹配
    fn apply_query(&mut self, query: String, cx: &mut Context<Self>) {
        // 上下文命令（Listary 风格）：重建结果集前，从旧列表捕获当前选中文件路径，
        // 命令插件对选中项操作（如 hash 计算选中文件 SHA256）
        let selected_path = self
            .entries
            .get(self.selected)
            .filter(|e| e.origin == search::EntryOrigin::File)
            .map(|e| e.path.clone());
        self.notice = None;

        let mut entries = self.source.search(&query, PAGE_LIMIT);
        // bench 模式跳过插件扇出：避免插件结果混入滚动/渲染性能基线
        if !self.bench {
            let plugin_entries = self.plugins.query_entries(&query, selected_path, PAGE_LIMIT);
            // 前缀命令置顶，其余插件结果按原 score 降序留在文件结果之后
            let (commands, others): (Vec<_>, Vec<_>) = plugin_entries
                .into_iter()
                .partition(|e| e.score >= plugin::COMMAND_SCORE as i64);
            let mut ordered = commands;
            ordered.append(&mut entries);
            ordered.extend(others);
            entries = ordered;
            // 工作流：仅 query 与 Manual 关键词完全一致时命中（与 Tauri 语义一致）
            #[cfg(target_os = "windows")]
            for wf in self.workflows.find_by_keyword(&query) {
                entries.push(search::Entry {
                    name: wf.name.clone(),
                    path: if wf.description.is_empty() { crate::i18n::t!("main.workflow_fallback").into() } else { wf.description.clone() },
                    score: 0,
                    origin: search::EntryOrigin::Workflow { workflow_id: wf.id.clone() },
                });
            }
        }
        self.entries = std::rc::Rc::new(entries);
        self.selected = 0;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.schedule_preview(cx);
        cx.notify();
    }

    /// 选中变化 → 防抖读取预览（方向键连按不重复读盘；preview_gen 丢弃过期结果）。
    /// 预览窗口关闭时跳过读盘（状态经 set_preview_open 清空）
    fn schedule_preview(&mut self, cx: &mut Context<Self>) {
        self.preview_gen = self.preview_gen.wrapping_add(1);
        let gen_id = self.preview_gen;
        // 插件结果无文件可预览（path 字段是副标题）
        let is_file = self
            .entries
            .get(self.selected)
            .map(|e| e.origin == search::EntryOrigin::File)
            .unwrap_or(false);
        if !self.preview_open || !is_file {
            self.preview = None;
            cx.notify();
            return;
        }
        let Some(path) = self.entries.get(self.selected).map(|e| std::path::PathBuf::from(&e.path)) else {
            self.preview = None;
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(PREVIEW_DEBOUNCE_MS))
                .await;
            // 同步小文件读取（≤1MB，毫秒级）直接跑在后台执行器
            let result = preview::read_file_preview(&path).map_err(|e| format!("{e:#}"));
            let _ = this.update(cx, |this, cx| {
                if this.preview_gen == gen_id {
                    this.preview = Some((path, result));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 移动选择（↑↓ 键）
    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        self.selected = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        self.scroll.scroll_to_item(self.selected, ScrollStrategy::Nearest);
        self.schedule_preview(cx);
        cx.notify();
    }

    /// 启动当前选中项：文件 → opener；插件 → PluginManager 分发，
    /// 副作用按 ExecuteOutcome 在此层真正执行（插件侧保持纯函数）
    fn launch_selected(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get(self.selected) else { return };
        match &entry.origin {
            search::EntryOrigin::File => {
                let path = entry.path.clone();
                println!("LAUNCH {}", path);
                // ⚠ opener 的 ShellExecuteW 会泵窗口消息：浏览器/资源管理器前台化
                // 触发失焦 → observe_window_activation 回调 remove_window，与当前
                // 事件派发的 App RefCell 可变借用重入 → gpui-pre "RefCell already
                // borrowed" panic（不可展开）→ 0xC0000409 fastfail 崩溃。
                // 副作用推迟到派发结束后执行（与下方工作流分支同款）。
                let audit_logger = self.audit_logger.clone();
                cx.spawn(async move |_, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(50))
                        .await;
                    let opened = opener::open(&path).is_ok();
                    if !opened {
                        eprintln!("⚠ 打开失败: {path}");
                    }
                    audit_logger.lock().log(
                        audit::AuditEventType::ProgramExecution {
                            plugin_id: "ilauncher-core".into(),
                            program: path,
                            allowed: opened,
                        },
                        if opened { audit::AuditSeverity::Info } else { audit::AuditSeverity::Warning },
                    );
                })
                .detach();
            }
            search::EntryOrigin::Plugin { plugin_id, result_id, action_id, .. } => {
                let (plugin_id, result_id, action_id) =
                    (plugin_id.clone(), result_id.clone(), action_id.clone());
                match self.plugins.execute(&plugin_id, &result_id, &action_id) {
                    Ok(plugin::ExecuteOutcome::Open(target)) => {
                        println!("PLUGIN_OPEN {}", target);
                        // 同 File 分支：ShellExecute 泵消息 + 失焦销毁重入会崩溃，推迟执行
                        let audit_logger = self.audit_logger.clone();
                        cx.spawn(async move |_, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(50))
                                .await;
                            let opened = opener::open(&target).is_ok();
                            if !opened {
                                eprintln!("⚠ 打开失败: {target}");
                            }
                            audit_logger.lock().log(
                                audit::AuditEventType::ProgramExecution {
                                    plugin_id,
                                    program: target,
                                    allowed: opened,
                                },
                                if opened {
                                    audit::AuditSeverity::Info
                                } else {
                                    audit::AuditSeverity::Warning
                                },
                            );
                        })
                        .detach();
                    }
                    Ok(plugin::ExecuteOutcome::Copy(text)) => {
                        // 剪贴板权限检查已在插件 execute 内完成（事件落审计管道）；
                        // 写入走 App 级剪贴板（等价 gpui 版 opener 的职责上移）
                        println!("PLUGIN_COPY {}", text);
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                    Ok(plugin::ExecuteOutcome::Notify(text)) => {
                        // Lua 命令反馈：显示在底部状态栏左侧（下次输入清空）
                        if !text.is_empty() {
                            println!("PLUGIN_NOTIFY {}", text);
                            self.notice = Some(text);
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠ 插件执行失败（{plugin_id}）: {e:#}");
                        // 权限拒绝/执行失败记 ProgramExecution 拒绝（插件 id 归属真实来源）
                        self.audit_logger.lock().log(
                            audit::AuditEventType::ProgramExecution {
                                plugin_id,
                                program: result_id,
                                allowed: false,
                            },
                            audit::AuditSeverity::Warning,
                        );
                    }
                }
            }
            // 工作流：后台执行（步骤可能含 Delay/HTTP），副作用回主线程执行
            #[cfg(target_os = "windows")]
            search::EntryOrigin::Workflow { workflow_id } => {
                let workflow_id = workflow_id.clone();
                println!("WORKFLOW_RUN {}", workflow_id);
                let engine = self.workflows.clone();
                let audit_logger = self.audit_logger.clone();
                cx.spawn(async move |this, cx| {
                    let mut effects = Vec::new();
                    let result = engine.execute_workflow(&workflow_id, Default::default(), &mut effects).await;
                    let _ = this.update(cx, |_, cx| {
                        match result {
                            Ok(_) => {
                                workflow_ui::consume_effects(effects, cx);
                                workflow_ui::log_run(&audit_logger, &workflow_id, true);
                            }
                            Err(e) => {
                                eprintln!("⚠ 工作流执行失败（{workflow_id}）: {e:#}");
                                workflow_ui::log_run(&audit_logger, &workflow_id, false);
                            }
                        }
                    });
                })
                .detach();
            }
            // 非 Windows 无工作流引擎：该来源不会出现（穷尽性占位）
            #[cfg(not(target_os = "windows"))]
            search::EntryOrigin::Workflow { .. } => {}
        }
        cx.notify();
    }

    /// 唤起时聚焦输入框（由 Watcher 调）
    fn focus_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| state.focus(window, cx));
    }

    /// --bench：后台 8ms 一次滚动驱动 + on_next_frame 自续计数，测真实交付帧率
    fn run_bench(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let total = self.entries.len();
        self.bench_t0 = Some(Instant::now());
        window.activate_window();
        self.count_frame(window, cx);

        cx.spawn(async move |this, cx| {
            let mut pos = 0usize;
            let mut ticks = 0usize;
            // 预热一帧
            cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
            loop {
                let done = this
                    .update(cx, |this, _| this.bench_t0.map(|t| t.elapsed().as_secs() >= 5).unwrap_or(false))
                    .unwrap_or(true);
                if done {
                    break;
                }
                pos = (pos + 7) % total;
                ticks += 1;
                let _ = this.update(cx, |this, cx| {
                    this.bench_tick = ticks;
                    this.scroll.scroll_to_item(pos, ScrollStrategy::Top);
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(8))
                    .await;
            }
        })
        .detach();
    }

    /// on_next_frame 自续帧计数：帧率 = 合成器实际交付的帧数 / 时间
    fn count_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.frame_count += 1;
        if let Some(t0) = self.bench_t0
            && t0.elapsed().as_secs() >= 5 {
                let fps = self.frame_count as f64 / t0.elapsed().as_secs_f64();
                let avg_render_ms = if self.render_count > 0 {
                    self.render_total_ms / self.render_count as f64
                } else {
                    0.0
                };
                let result = serde_json::json!({
                    "bench": "uniform_list_scroll_rAF",
                    "rows": self.entries.len(),
                    "duration_secs": t0.elapsed().as_secs_f64(),
                    "frames": self.frame_count,
                    "fps": fps,
                    "renders": self.render_count,
                    "avg_render_ms": avg_render_ms,
                    "cold_start_ms": start_time().elapsed().as_secs_f64() * 1000.0,
                });
                println!("BENCH_RESULT {}", serde_json::to_string_pretty(&result).unwrap());
                std::fs::write("gpui-p0-result.json", serde_json::to_string_pretty(&result).unwrap()).ok();
                std::process::exit(0);
            }
        cx.on_next_frame(window, |this, window, cx| this.count_frame(window, cx));
    }
}

/// 预览窗口镜像快照（Launcher.preview_state 返回）
pub(crate) struct PreviewState {
    pub entry: Option<(String, String)>,
    pub preview: Option<(std::path::PathBuf, Result<preview::FilePreview, String>)>,
}

/// 空状态：无查询时的主区内容——邀请行动，而不是留白。
/// 图标 + 一句话，居中，全部用弱化色。
fn empty_state(theme: &gpui_kit::component::theme::Theme) -> gpui_kit::AnyElement {
    v_flex()
        .id("empty-state")
        .size_full()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            Icon::new(IconName::Search)
                .size(px(28.))
                .text_color(theme.muted_foreground),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(crate::i18n::t!("main.empty_hint").to_string()),
        )
        .into_any_element()
}

impl Render for Launcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.first_render_done.is_none() {
            self.first_render_done = Some(Instant::now());
            println!(
                "COLD_START_TO_FIRST_RENDER_MS {:.1}",
                start_time().elapsed().as_secs_f64() * 1000.0
            );
        }
        self.render_count += 1;
        let render_t0 = Instant::now();

        let theme = cx.theme().clone();
        let entries = self.entries.clone();
        let selected = self.selected;
        let result_count = entries.len();
        let theme_for_list = theme.clone();
        let theme_for_icons = theme.clone();
        let launcher = cx.entity();
        // 空查询（非 bench）→ 主区显示空状态；bench 保持全量列表跑基线
        let show_empty = result_count == 0 && !self.bench;

        let root = v_flex()
            .id("root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let ks = &ev.keystroke;
                // Alt+P：切换文件预览窗口（独立窗口，贴主窗口右边框）
                if ks.modifiers.alt && ks.key.eq_ignore_ascii_case("p") {
                    let _ = this.tx.send(AppSignal::TogglePreview);
                    return;
                }
                // Ctrl+1..9：直接启动对应行（Listary 风格快捷键提示的真实行为）
                if ks.modifiers.control && !ks.modifiers.alt && ks.key.len() == 1 {
                    if let Some(d) = ks.key.chars().next().and_then(|c| c.to_digit(10)) {
                        if (1..=9).contains(&d) {
                            let ix = d as usize - 1;
                            if ix < this.entries.len() {
                                this.selected = ix;
                                this.launch_selected(cx);
                            }
                            return;
                        }
                    }
                }
                match ks.key.as_str() {
                    "up" => this.move_selection(-1, cx),
                    "down" => this.move_selection(1, cx),
                    "enter" => this.launch_selected(cx),
                    // gpui-pre 无 hide API：销毁窗口，唤起时由 Watcher 重建
                    "escape" => window.remove_window(),
                    _ => {}
                }
            }))
            .size_full()
            .p_3()
            .gap_2()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(window_drag::drag_strip("iLauncher", &theme))
            .child(
                // 命令栏：Listary 风格——无框无底色输入 + 底部一条分隔线，
                // 右侧齿轮进设置；视觉焦点全靠下划线与字号
                v_flex()
                    .w_full()
                    .child(
                        Input::new(&self.input)
                            .w_full()
                            .appearance(false)
                            .cleanable(true)
                            .prefix(
                                Icon::new(IconName::Search)
                                    .size(px(15.))
                                    .text_color(theme.muted_foreground),
                            )
                            .suffix(
                                Button::new("open-settings")
                                    .outline()
                                    .small()
                                    .icon(IconName::Settings)
                                    .on_click({
                                        let tx = self.tx.clone();
                                        move |_, _, _| {
                                            let _ = tx.send(AppSignal::ShowSettings);
                                        }
                                    }),
                            ),
                    )
                    .child(div().w_full().h(px(1.)).bg(theme.border)),
            )
            .child(
                // Listary 风格结果区：节标题 + 两行式大图标行 + Ctrl+N 提示
                div()
                    .id("results-wrap")
                    .flex_1()
                    .size_full()
                    .child(if show_empty {
                        empty_state(&theme)
                    } else {
                        v_flex()
                            .size_full()
                            .child(
                                div()
                                    .px_1()
                                    .pb_1()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(theme.muted_foreground)
                                    .child(crate::i18n::t!("main.recent_files").to_string()),
                            )
                            .child(
                                uniform_list("result-list", entries.len(), {
                                    let launcher = launcher.clone();
                                    let theme_for_icons = theme_for_icons.clone();
                                    move |visible_range, _window, _cx| {
                                        visible_range
                                            .map(|ix| {
                                                let entry = &entries[ix];
                                                let is_selected = ix == selected;
                                                // 行首图标：插件渲染 emoji 图标，文件用统一文档图标
                                                let row_icon: gpui_kit::AnyElement =
                                                    match &entry.origin {
                                                        search::EntryOrigin::Plugin {
                                                            icon: Some(i),
                                                            ..
                                                        } => div().child(i.clone()).into_any_element(),
                                                        _ => Icon::new(IconName::FileText)
                                                            .size(px(18.))
                                                            .text_color(theme_for_icons.muted_foreground)
                                                            .into_any_element(),
                                                    };
                                                // 快捷键提示：前 9 行给 Ctrl+N（Listary 同款）
                                                let shortcut = std::cell::RefCell::new(if ix < 9 {
                                                    Some(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme_for_list.muted_foreground)
                                                            .child(format!("Ctrl+{}", ix + 1))
                                                            .into_any_element(),
                                                    )
                                                } else {
                                                    None
                                                });
                                                // gpui-component ListItem：选中/悬停色全部由
                                                // theme tokens（list_active / list_hover）驱动
                                                ListItem::new(ix)
                                                    .selected(is_selected)
                                                    .h(px(54.))
                                                    .on_click({
                                                        let launcher = launcher.clone();
                                                        move |_, _, cx| {
                                                            launcher.update(cx, |this: &mut Launcher, cx| {
                                                                this.selected = ix;
                                                                this.schedule_preview(cx);
                                                                cx.notify();
                                                            });
                                                        }
                                                    })
                                                    .suffix(move |_, _| {
                                                        shortcut
                                                            .borrow_mut()
                                                            .take()
                                                            .unwrap_or_else(|| div().into_any_element())
                                                    })
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .h_full()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(row_icon)
                                                            .child(
                                                                v_flex()
                                                                    .flex_1()
                                                                    .min_w_0()
                                                                    .gap_0p5()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_medium()
                                                                            .truncate()
                                                                            .child(entry.name.clone()),
                                                                    )
                                                                    .child(
                                                                        // 路径占剩余全部宽度，超长时截断
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(theme_for_list.muted_foreground)
                                                                            .truncate()
                                                                            .child(entry.path.clone()),
                                                                    ),
                                                            ),
                                                    )
                                            })
                                            .collect::<Vec<_>>()
                                    }
                                })
                                .size_full()
                                .track_scroll(&self.scroll)
                                .into_any_element(),
                            )
                            .into_any_element()
                    }),
            )
            .child(
                h_flex()
                    .w_full()
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .pt_1()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                // Listary 风格汉堡菜单：设置入口
                                Button::new("menu")
                                    .outline()
                                    .small()
                                    .icon(IconName::Menu)
                                    .on_click({
                                        let tx = self.tx.clone();
                                        move |_, _, _| {
                                            let _ = tx.send(AppSignal::ShowSettings);
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(if let Some(notice) = &self.notice {
                                        notice.clone()
                                    } else {
                                        crate::i18n::t!(
                                            "main.result_count",
                                            count = result_count,
                                            elapsed = self.source.status_text(),
                                            extra = if self.bench { format!(" · tick {}", self.bench_tick) } else { String::new() }
                                        )
                                        .to_string()
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(window_drag::kbd_pill(crate::i18n::t!("main.hint_select"), &theme))
                            .child(window_drag::kbd_pill(crate::i18n::t!("main.hint_open"), &theme))
                            .child(window_drag::kbd_pill(crate::i18n::t!("main.hint_preview"), &theme))
                            .child(window_drag::kbd_pill(crate::i18n::t!("main.hint_hide"), &theme))
                            .child(div().w(px(1.)).h(px(12.)).bg(theme.border))
                            .child(
                                Button::new("quit")
                                    .small()
                                    .outline()
                                    .label(crate::i18n::t!("main.quit_button").as_ref())
                                    .on_click(|_, _, _| std::process::exit(0)),
                            ),
                    ),
            );
        self.render_total_ms += render_t0.elapsed().as_secs_f64() * 1000.0;
        root
    }
}

// ── 窗口生命周期守护：隐藏后唤起重建 + 信号轮询 ─────────────────────────────
//
// 不用 GPUI 实体承载（实体在 app 初始化闭包返回后会被释放），
// 改为纯异步任务直接持有状态（rx + 窗口句柄），随任务存活。

/// 窗口间共享的单例依赖（WindowGuard 与各类 summon 窗口统一入口）
struct Deps {
    /// 审计日志（启动器核心 + 插件沙盒 + 工作流共用管道）
    audit_logger: Arc<Mutex<audit::AuditLogger>>,
    /// 插件管理器（主窗口搜索扇出 + 执行分发）
    plugins: Arc<plugin::PluginManager>,
    /// 插件市场状态（注册表 + 安装器 + 商店客户端）
    #[cfg(windows)]
    market: Arc<plugin_ui::MarketState>,
    /// 工作流引擎（关键词触发 + 管理窗口共享）
    #[cfg(target_os = "windows")]
    workflows: Arc<workflow::WorkflowEngine>,
    /// AI 对话引擎（配置 + 会话持久化）
    #[cfg(windows)]
    ai_chat: Arc<ai::AiChat>,
    /// 剪贴板历史存储（feature clipboard）
    #[cfg(all(feature = "clipboard", target_os = "windows"))]
    clipboard_store: Arc<Mutex<ClipboardStore>>,
}

struct WindowGuard {
    rx: mpsc::Receiver<AppSignal>,
    /// 信号发送端：设置页"重建索引"等 UI 内按钮复用同一通道
    tx: mpsc::Sender<AppSignal>,
    window: Option<(WindowHandle<Root>, Entity<Launcher>)>,
    #[cfg(feature = "ilauncher")]
    index_set: LiveSet,
    deps: Deps,
    /// 剪贴板历史窗口（feature clipboard）
    #[cfg(all(feature = "clipboard", target_os = "windows"))]
    clipboard_window: Option<(WindowHandle<Root>, Entity<clipboard_ui::ClipboardPanel>)>,
    /// 设置窗口
    #[cfg(windows)]
    settings_window: Option<(WindowHandle<Root>, Entity<settings_ui::SettingsView>)>,
    /// 审计日志查看器窗口
    #[cfg(windows)]
    audit_window: Option<(WindowHandle<Root>, Entity<audit_ui::AuditPanel>)>,
    /// 插件市场/管理窗口
    #[cfg(windows)]
    plugins_window: Option<(WindowHandle<Root>, Entity<plugin_ui::MarketPanel>)>,
    /// 工作流管理窗口
    #[cfg(windows)]
    workflow_window: Option<(WindowHandle<Root>, Entity<workflow_ui::WorkflowPanel>)>,
    /// AI 对话窗口
    #[cfg(windows)]
    ai_window: Option<(WindowHandle<Root>, Entity<ai_ui::AiChatPanel>)>,
    /// 文件预览窗口（独立顶级窗口，贴主窗口右边框；Alt+P 切换，默认隐藏）
    preview_window: Option<(WindowHandle<Root>, Entity<preview_ui::PreviewPanel>)>,
    /// 预览窗口显隐偏好（主窗口重建后恢复）
    preview_visible: bool,
}

impl WindowGuard {
    fn new(
        rx: mpsc::Receiver<AppSignal>,
        tx: mpsc::Sender<AppSignal>,
        index_set: LiveSet,
        deps: Deps,
    ) -> Self {
        #[cfg(not(feature = "ilauncher"))]
        let _ = index_set;
        Self {
            rx,
            tx,
            window: None,
            #[cfg(feature = "ilauncher")]
            index_set,
            deps,
            #[cfg(all(feature = "clipboard", target_os = "windows"))]
            clipboard_window: None,
            #[cfg(windows)]
            settings_window: None,
            #[cfg(windows)]
            audit_window: None,
            #[cfg(windows)]
            plugins_window: None,
            #[cfg(windows)]
            workflow_window: None,
            #[cfg(windows)]
            ai_window: None,
            preview_window: None,
            preview_visible: false,
        }
    }

    /// 构造搜索源：feature 开启走实时索引（env 可指定单快照调试），否则 Demo
    fn make_source(&self) -> SearchSource {
        #[cfg(feature = "ilauncher")]
        {
            if std::env::var("ILAUNCHER_SNAPSHOT").is_ok() {
                return SearchSource::from_env_single(demo_entries());
            }
            #[cfg(target_os = "windows")]
            return SearchSource::Live(self.index_set.clone());
            #[cfg(not(target_os = "windows"))]
            return SearchSource::Demo(demo_entries());
        }
        #[cfg(not(feature = "ilauncher"))]
        SearchSource::Demo(demo_entries())
    }

    /// 唤起：窗口还在就前台激活 + 聚焦输入；已被 Esc 销毁则重建
    fn summon(&mut self, pressed_at: Instant, cx: &mut AsyncApp) {
        if let Some((handle, launcher)) = &self.window {
            let activated = handle
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    launcher.update(cx, |l, cx| l.focus_input(window, cx));
                })
                .is_ok();
            if activated {
                println!("HOTKEY_TO_HANDLE_MS {:.1}", pressed_at.elapsed().as_secs_f64() * 1000.0);
                return;
            }
            self.window = None;
        }

        // 重建窗口（Esc/失焦销毁后首次唤起 / 初始唤起），主显示器居中
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(centered_bounds(cx, 1000., 520.))),
            ..make_window_options()
        };
        let source = self.make_source();
        let audit_logger = self.deps.audit_logger.clone();
        let plugins = self.deps.plugins.clone();
        let tx = self.tx.clone();
        #[cfg(target_os = "windows")]
        let workflows = self.deps.workflows.clone();
        let mut launcher_slot: Option<Entity<Launcher>> = None;
        let result = cx.open_window(options, |window, cx| {
            let launcher = cx.new(|cx| {
                Launcher::new(
                    window,
                    cx,
                    source,
                    audit_logger,
                    plugins,
                    tx,
                    #[cfg(target_os = "windows")]
                    workflows,
                )
            });
            launcher_slot = Some(launcher.clone());
            cx.new(|cx| Root::new(launcher, window, cx).bg(cx.theme().background))
        });
        match result {
            Ok(handle) => {
                let launcher = launcher_slot.expect("launcher entity");
                // 新窗口显式前台激活 + 聚焦搜索框——否则热键/托盘唤起后
                // 键盘焦点不在输入框，用户还得先点一下（Esc/失焦销毁后的
                // 重建路径是唤起的主路径，必须与 activate_existing 路径一致）
                let _ = handle.update(cx, |_, window, cx| {
                    window.activate_window();
                    launcher.update(cx, |l, cx| l.focus_input(window, cx));
                });
                println!("SUMMON_REOPEN_MS {:.1}", pressed_at.elapsed().as_secs_f64() * 1000.0);
                self.window = Some((handle, launcher.clone()));
                // 预览窗口偏好开启时跟随重建（贴新主窗口右边框）
                launcher.update(cx, |l, cx| {
                    l.set_preview_open(self.preview_visible, cx);
                    if self.preview_visible {
                        l.schedule_preview(cx);
                    }
                });
                if self.preview_visible {
                    self.open_preview(cx, &launcher);
                }
            }
            Err(e) => eprintln!("⚠ 重建窗口失败: {e:#}"),
        }
    }

    /// Alt+P：翻转预览窗口显隐偏好并落窗口
    fn toggle_preview(&mut self, cx: &mut AsyncApp) {
        self.preview_visible = !self.preview_visible;
        if self.preview_visible {
            if let Some((_, launcher)) = &self.window {
                let launcher = launcher.clone();
                launcher.update(cx, |l, cx| {
                    l.set_preview_open(true, cx);
                    // 立即补一次预览读取（此前关闭时跳过读盘，preview 为空）
                    l.schedule_preview(cx);
                });
                self.open_preview(cx, &launcher);
            }
        } else {
            self.close_preview(cx);
            if let Some((_, launcher)) = &self.window {
                launcher.update(cx, |l, cx| l.set_preview_open(false, cx));
            }
        }
    }

    /// 关闭预览窗口（保留显隐偏好）
    fn close_preview(&mut self, cx: &mut AsyncApp) {
        if let Some((handle, _)) = self.preview_window.take() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }

    /// 打开预览窗口：贴主窗口右边框（右侧放不下则贴左侧），等高于主窗口
    fn open_preview(&mut self, cx: &mut AsyncApp, launcher: &Entity<Launcher>) {
        // 已存在则先销毁重建（bounds 跟随：gpui-pre 无窗口移动 API）
        self.close_preview(cx);
        let Some((main_handle, _)) = &self.window else { return };
        let Ok(main_bounds) = main_handle.update(cx, |_, window, _| window.bounds()) else {
            return;
        };
        let preview_w: gpui_kit::Pixels = px(380.);
        let gap: gpui_kit::Pixels = px(8.);
        let visible = cx.update(|cx| cx.primary_display().map(|d| d.visible_bounds()));
        // 右侧放不下时贴左边
        let x = match visible {
            Some(v) if main_bounds.origin.x + main_bounds.size.width + gap + preview_w
                > v.origin.x + v.size.width =>
            {
                (main_bounds.origin.x - gap - preview_w).max(px(0.))
            }
            _ => main_bounds.origin.x + main_bounds.size.width + gap,
        };
        let bounds = Bounds {
            origin: Point { x, y: main_bounds.origin.y },
            size: size(preview_w, main_bounds.size.height),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..make_panel_window_options()
        };
        let launcher = launcher.clone();
        let mut panel_slot: Option<Entity<preview_ui::PreviewPanel>> = None;
        let result = cx.open_window(options, |window, cx| {
            let panel = cx.new(|cx| preview_ui::PreviewPanel::new(window, cx, launcher));
            panel_slot = Some(panel.clone());
            cx.new(|cx| Root::new(panel, window, cx).bg(cx.theme().background))
        });
        match result {
            Ok(handle) => {
                // 预览窗口不抢焦点（启动器焦点保持在搜索框）
                self.preview_window = Some((handle, panel_slot.expect("preview panel entity")));
            }
            Err(e) => eprintln!("⚠ 打开预览窗口失败: {e:#}"),
        }
    }

    /// 打开/激活剪贴板历史窗口（feature clipboard）
    #[cfg(all(feature = "clipboard", target_os = "windows"))]
    fn summon_clipboard(&mut self, cx: &mut AsyncApp) {
        if Self::activate_existing(&mut self.clipboard_window, cx) {
            return;
        }
        let store = self.deps.clipboard_store.clone();
        let mut panel_slot: Option<Entity<clipboard_ui::ClipboardPanel>> = None;
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(centered_bounds(cx, 1000., 520.))),
            ..make_panel_window_options()
        };
        let result = cx.open_window(options, |window, cx| {
            let panel = cx.new(|cx| clipboard_ui::ClipboardPanel::new(window, cx, store));
            panel_slot = Some(panel.clone());
            cx.new(|cx| Root::new(panel, window, cx).bg(cx.theme().background))
        });
        match result {
            Ok(handle) => {
                // 新窗口显式前台激活：不激活时 PopUp/Normal 都可能落在别的窗口后面
                let _ = handle.update(cx, |_, window, _| window.activate_window());
                println!("✓ 剪贴板历史窗口已打开");
                self.clipboard_window = Some((handle, panel_slot.expect("clipboard panel entity")));
            }
            Err(e) => eprintln!("⚠ 打开剪贴板历史窗口失败: {e:#}"),
        }
    }

    /// 打开/激活设置窗口（gpui-component Settings 组件，五分区）
    #[cfg(windows)]
    fn summon_settings(&mut self, cx: &mut AsyncApp) {
        if Self::activate_existing(&mut self.settings_window, cx) {
            return;
        }
        let index_set = {
            #[cfg(feature = "ilauncher")]
            {
                self.index_set.clone()
            }
            #[cfg(not(feature = "ilauncher"))]
            {
                LiveSet::empty()
            }
        };
        let tx = self.tx.clone();
        #[cfg(all(feature = "clipboard", target_os = "windows"))]
        let store = self.deps.clipboard_store.clone();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(centered_bounds(cx, 860., 560.))),
            ..make_panel_window_options()
        };
        let mut view_slot: Option<Entity<settings_ui::SettingsView>> = None;
        let result = cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| {
                settings_ui::SettingsView::new(
                    cx,
                    index_set,
                    tx,
                    #[cfg(all(feature = "clipboard", target_os = "windows"))]
                    store,
                )
            });
            view_slot = Some(view.clone());
            cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
        });
        match result {
            Ok(handle) => {
                // 新窗口显式前台激活：不激活时 PopUp/Normal 都可能落在别的窗口后面
                let _ = handle.update(cx, |_, window, _| window.activate_window());
                println!("✓ 设置窗口已打开");
                self.settings_window = Some((handle, view_slot.expect("settings view entity")));
            }
            Err(e) => eprintln!("⚠ 打开设置窗口失败: {e:#}"),
        }
    }

    /// 副窗口通用前缀：句柄存活则前台激活并返回 true；失效则清槽位返回 false
    fn activate_existing<T: 'static>(
        slot: &mut Option<(WindowHandle<Root>, Entity<T>)>,
        cx: &mut AsyncApp,
    ) -> bool {
        if let Some((handle, _)) = slot {
            if handle.update(cx, |_, window, _| window.activate_window()).is_ok() {
                return true;
            }
            *slot = None;
        }
        false
    }

    /// 打开/激活审计日志查看器
    #[cfg(windows)]
    fn summon_audit(&mut self, cx: &mut AsyncApp) {
        if Self::activate_existing(&mut self.audit_window, cx) {
            return;
        }
        let logger = self.deps.audit_logger.clone();
        let mut panel_slot: Option<Entity<audit_ui::AuditPanel>> = None;
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(centered_bounds(cx, 1000., 520.))),
            ..make_panel_window_options()
        };
        let result = cx.open_window(options, |window, cx| {
            let panel = cx.new(|cx| audit_ui::AuditPanel::new(window, cx, logger));
            panel_slot = Some(panel.clone());
            cx.new(|cx| Root::new(panel, window, cx).bg(cx.theme().background))
        });
        match result {
            Ok(handle) => {
                // 新窗口显式前台激活：不激活时 PopUp/Normal 都可能落在别的窗口后面
                let _ = handle.update(cx, |_, window, _| window.activate_window());
                println!("✓ 审计日志窗口已打开");
                self.audit_window = Some((handle, panel_slot.expect("audit panel entity")));
            }
            Err(e) => eprintln!("⚠ 打开审计日志窗口失败: {e:#}"),
        }
    }

    /// 打开/激活插件市场/管理窗口
    #[cfg(windows)]
    fn summon_plugins(&mut self, cx: &mut AsyncApp) {
        if Self::activate_existing(&mut self.plugins_window, cx) {
            return;
        }
        let state = self.deps.market.clone();
        let plugins = self.deps.plugins.clone();
        let mut panel_slot: Option<Entity<plugin_ui::MarketPanel>> = None;
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(centered_bounds(cx, 1000., 520.))),
            ..make_panel_window_options()
        };
        let result = cx.open_window(options, |window, cx| {
            let panel = cx.new(|cx| plugin_ui::MarketPanel::new(window, cx, state, plugins));
            panel_slot = Some(panel.clone());
            cx.new(|cx| Root::new(panel, window, cx).bg(cx.theme().background))
        });
        match result {
            Ok(handle) => {
                // 新窗口显式前台激活：不激活时 PopUp/Normal 都可能落在别的窗口后面
                let _ = handle.update(cx, |_, window, _| window.activate_window());
                println!("✓ 插件窗口已打开");
                self.plugins_window = Some((handle, panel_slot.expect("market panel entity")));
            }
            Err(e) => eprintln!("⚠ 打开插件窗口失败: {e:#}"),
        }
    }

    /// 打开/激活工作流管理窗口
    #[cfg(windows)]
    fn summon_workflows(&mut self, cx: &mut AsyncApp) {
        if Self::activate_existing(&mut self.workflow_window, cx) {
            return;
        }
        let engine = self.deps.workflows.clone();
        let audit_logger = self.deps.audit_logger.clone();
        let mut panel_slot: Option<Entity<workflow_ui::WorkflowPanel>> = None;
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(centered_bounds(cx, 1000., 520.))),
            ..make_panel_window_options()
        };
        let result = cx.open_window(options, |window, cx| {
            let panel = cx.new(|cx| workflow_ui::WorkflowPanel::new(window, cx, engine, audit_logger));
            panel_slot = Some(panel.clone());
            cx.new(|cx| Root::new(panel, window, cx).bg(cx.theme().background))
        });
        match result {
            Ok(handle) => {
                // 新窗口显式前台激活：不激活时 PopUp/Normal 都可能落在别的窗口后面
                let _ = handle.update(cx, |_, window, _| window.activate_window());
                println!("✓ 工作流窗口已打开");
                self.workflow_window = Some((handle, panel_slot.expect("workflow panel entity")));
            }
            Err(e) => eprintln!("⚠ 打开工作流窗口失败: {e:#}"),
        }
    }

    /// 打开/激活 AI 对话窗口
    #[cfg(windows)]
    fn summon_ai(&mut self, cx: &mut AsyncApp) {
        if Self::activate_existing(&mut self.ai_window, cx) {
            return;
        }
        let chat = self.deps.ai_chat.clone();
        let mut panel_slot: Option<Entity<ai_ui::AiChatPanel>> = None;
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(centered_bounds(cx, 1000., 520.))),
            ..make_panel_window_options()
        };
        let result = cx.open_window(options, |window, cx| {
            let panel = cx.new(|cx| ai_ui::AiChatPanel::new(window, cx, chat));
            panel_slot = Some(panel.clone());
            cx.new(|cx| Root::new(panel, window, cx).bg(cx.theme().background))
        });
        match result {
            Ok(handle) => {
                // 新窗口显式前台激活：不激活时 PopUp/Normal 都可能落在别的窗口后面
                let _ = handle.update(cx, |_, window, _| window.activate_window());
                println!("✓ AI 对话窗口已打开");
                self.ai_window = Some((handle, panel_slot.expect("ai chat panel entity")));
            }
            Err(e) => eprintln!("⚠ 打开 AI 对话窗口失败: {e:#}"),
        }
    }
}

fn make_window_options() -> WindowOptions {
    WindowOptions {
        kind: WindowKind::PopUp,
        titlebar: None,
        window_background: WindowBackgroundAppearance::Blurred,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Point { x: px(160.), y: px(200.) },
            size: size(px(1000.), px(520.)),
        })),
        ..Default::default()
    }
}

/// 副窗口（设置/剪贴板/审计/插件/工作流/AI）选项：外观与主窗口一致，
/// 但 kind 用 Normal——进任务栏，被其他窗口遮挡时用户仍能从任务栏找回；
/// 主窗口保持 PopUp（启动器语义，不占任务栏）
fn make_panel_window_options() -> WindowOptions {
    WindowOptions {
        kind: WindowKind::Normal,
        ..make_window_options()
    }
}

/// 窗口在主显示器（可见区，排除任务栏）居中的 bounds；
/// 取不到显示器信息时回退原默认位
fn centered_bounds(cx: &mut AsyncApp, width: f32, height: f32) -> Bounds<Pixels> {
    let visible = cx.update(|cx| cx.primary_display().map(|d| d.visible_bounds()));
    let win_size = size(px(width), px(height));
    match visible {
        Some(b) => {
            let center = b.center();
            Bounds::new(
                Point {
                    x: center.x - win_size.width / 2.0,
                    y: center.y - win_size.height / 2.0,
                },
                win_size,
            )
        }
        None => Bounds::new(Point { x: px(160.), y: px(200.) }, win_size),
    }
}

/// Windows 下 gpui 默认 .SystemUIFont 对 CJK 字形回退到宋体（SimSun），
/// 界面发虚发"土"；显式指定微软雅黑。主题每次 change 后需重刷
/// （font_family 不在 ThemeMode 切换的保留字段里）
#[cfg(target_os = "windows")]
fn apply_cjk_font(cx: &mut gpui_kit::App) {
    use gpui_kit::component::theme::Theme;
    cx.update_global::<Theme, _>(|theme, _| {
        theme.font_family = "Microsoft YaHei UI".into();
    });
}

// ── 托盘（菜单事件：显示 / 退出） ────────────────────────────────────────────

fn setup_tray(tx: mpsc::Sender<AppSignal>) {
    // 托盘整体（含 CheckMenuItem 句柄）都在本线程创建和使用——
    // muda 的菜单项内部是 Rc，!Send，跨线程移动会编译失败
    std::thread::spawn(move || {
        use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
        use tray_icon::TrayIconBuilder;
        use crate::i18n::t;

        // 菜单文案在启动时按当前语言定型（切换语言后托盘菜单下次启动更新）
        let menu = Menu::new();
        let _ = menu.append(&MenuItem::with_id("show", t!("tray.show"), true, None));
        let _ = menu.append(&MenuItem::with_id("settings", t!("tray.settings"), true, None));
        let _ = menu.append(&MenuItem::with_id("clipboard", t!("tray.clipboard"), true, None));
        let _ = menu.append(&MenuItem::with_id("audit", t!("tray.audit"), true, None));
        let _ = menu.append(&MenuItem::with_id("plugins", t!("tray.plugins"), true, None));
        let _ = menu.append(&MenuItem::with_id("workflows", t!("tray.workflows"), true, None));
        let _ = menu.append(&MenuItem::with_id("ai", t!("tray.ai"), true, None));
        let _ = menu.append(&MenuItem::with_id("rebuild", t!("tray.rebuild"), true, None));
        // 开机自启：可勾选项，初始状态读注册表
        let autostart_item =
            CheckMenuItem::with_id("autostart", t!("tray.autostart"), true, autostart::is_enabled(), None);
        let _ = menu.append(&autostart_item);
        // 深色主题：可勾选项；未持久化过时跟随系统设置
        let dark_item = CheckMenuItem::with_id(
            "dark_theme",
            t!("tray.dark_theme"),
            true,
            settings::load_theme_dark().unwrap_or_else(settings::system_prefers_dark),
            None,
        );
        let _ = menu.append(&dark_item);
        let _ = menu.append(&MenuItem::with_id("quit", t!("tray.quit"), true, None));
        // 左键唤起主窗口（左键不再弹菜单，仅右键弹；事件在下方循环消费）
        let _tray = match TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("iLauncher")
            .with_icon(tray_icon_gen::build())
            .with_menu_on_left_click(false)
            .build()
        {
            Ok(tray) => {
                println!("✓ 托盘已创建（开机自启={}）", autostart::is_enabled());
                tray
            }
            Err(e) => {
                println!("⚠️ 托盘创建失败: {e}");
                return;
            }
        };

        // 托盘线程主循环：泵 Win32 消息 + 分发 muda 菜单事件 + 托盘图标事件。
        // 关键：tray-icon 的隐藏窗口和 muda 菜单子类都挂在窗口过程（wndproc）上，
        // 而 wndproc 只在创建窗口的线程检索消息时才被调用——本线程不泵消息的话，
        // 点击事件根本到不了 wndproc，左右键会全部无响应（首测复现的正是此问题）
        // 全局热键也注册在本线程：WM_HOTKEY 发到线程消息队列，
        // 只有泵消息的线程能收到。global-hotkey crate 只建隐藏窗口不泵消息，
        // 事件永远到不了 wndproc（与此前托盘点击失灵的根因同类），故直接用 Win32 API
        const HOTKEY_ID: i32 = 0x1A;
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                RegisterHotKey, MOD_CONTROL, MOD_NOREPEAT, VK_SPACE,
            };
            if RegisterHotKey(None, HOTKEY_ID, MOD_CONTROL | MOD_NOREPEAT, VK_SPACE.0 as u32).is_ok() {
                println!("✓ 全局热键已注册: Ctrl+Space");
            } else {
                eprintln!(
                    "⚠ 注册 Ctrl+Space 失败（错误码 {:?}），热键可能被输入法或其他程序占用",
                    windows::core::Error::from_win32()
                );
            }
        }

        let receiver = MenuEvent::receiver();
        let tray_events = tray_icon::TrayIconEvent::receiver();
        loop {
            let mut got_msg = false;
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{
                    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_HOTKEY,
                };
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    got_msg = true;
                    if msg.message == WM_HOTKEY && msg.wParam.0 as i32 == HOTKEY_ID {
                        let _ = tx.send(AppSignal::Show(Instant::now()));
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            if !got_msg {
                // PeekMessage 无等待立即返回，空转时必须休眠，否则忙等烧满一个核
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            // 左键点托盘图标 → 唤起主窗口（菜单仅右键弹出）
            while let Ok(tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Down,
                ..
            }) = tray_events.try_recv()
            {
                let _ = tx.send(AppSignal::Show(Instant::now()));
            }
            while let Ok(event) = receiver.try_recv() {
                match event.id.0.as_ref() {
                    "show" => {
                        let _ = tx.send(AppSignal::Show(Instant::now()));
                    }
                    "settings" => {
                        let _ = tx.send(AppSignal::ShowSettings);
                    }
                    "clipboard" => {
                        let _ = tx.send(AppSignal::ShowClipboard);
                    }
                    "audit" => {
                        let _ = tx.send(AppSignal::ShowAudit);
                    }
                    "plugins" => {
                        let _ = tx.send(AppSignal::ShowPlugins);
                    }
                    "workflows" => {
                        let _ = tx.send(AppSignal::ShowWorkflows);
                    }
                    "ai" => {
                        let _ = tx.send(AppSignal::ShowAi);
                    }
                    "rebuild" => {
                        let _ = tx.send(AppSignal::RebuildIndex);
                    }
                    "autostart" => {
                        // 切换注册表 Run 项并同步勾选状态
                        let now_enabled = if autostart::is_enabled() {
                            match autostart::disable() {
                                Ok(()) => false,
                                Err(e) => {
                                    println!("⚠️ 取消自启失败: {e:#}");
                                    true
                                }
                            }
                        } else {
                            match autostart::enable() {
                                Ok(()) => true,
                                Err(e) => {
                                    println!("⚠️ 设置自启失败: {e:#}");
                                    false
                                }
                            }
                        };
                        autostart_item.set_checked(now_enabled);
                        println!("✓ 开机自启 → {}", if now_enabled { "已启用" } else { "已关闭" });
                    }
                    "dark_theme" => {
                        // muda 点击可勾选项后内部状态已翻转，is_checked() 即目标态
                        let want_dark = dark_item.is_checked();
                        if let Err(e) = settings::save_theme_dark(want_dark) {
                            println!("⚠️ 保存主题偏好失败: {e:#}");
                        }
                        let _ = tx.send(AppSignal::SetTheme(want_dark));
                        println!("✓ 主题 → {}", if want_dark { "深色" } else { "浅色" });
                    }
                    "quit" => std::process::exit(0),
                    _ => {}
                }
            }
            // 消息泵是非阻塞的，节流避免空转烧 CPU
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
}

// ── LiveIndex 真实快照搜索基准（feature ilauncher） ──────────────────────────

#[cfg(feature = "ilauncher")]
fn run_snapshot_bench(path: &str) {
    use ilauncher_index::index_v2::LiveIndex;
    let t0 = Instant::now();
    let idx = LiveIndex::open(std::path::Path::new(path)).expect("open snapshot");
    println!("SNAPSHOT_OPEN_MS {:.1}（{} 行）", t0.elapsed().as_secs_f64() * 1000.0, idx.snapshot().row_count());

    for q in ["report", "notes", "简历", "rpt", "config", "explorer", "wechat", "steam"] {
        let t = Instant::now();
        let hits = idx.search(q, 50).unwrap();
        println!("  QUERY {:>10} → {:>3} hits in {:>7.2?}", q, hits.len(), t.elapsed());
    }
}

#[cfg(not(feature = "ilauncher"))]
fn run_snapshot_bench(_path: &str) {
    eprintln!("需要 --features ilauncher 编译");
}

// ── main ────────────────────────────────────────────────────────────────────

fn main() {
    start_time();
    i18n::apply(); // 语言偏好 → 全局 locale（托盘/窗口文案在此之后定型）
    let args: Vec<String> = std::env::args().collect();

    // ── 常驻 MFT 服务模式（提权子进程，UI 退出后自动停止） ──────────────────
    #[cfg(all(feature = "ilauncher", target_os = "windows"))]
    if args.iter().any(|a| a == "--mft-service") {
        index_service::imp::run_mft_service(&args);
        return;
    }
    #[cfg(all(feature = "ilauncher", target_os = "windows"))]
    if args.iter().any(|a| a == "--rebuild") {
        index_service::imp::run_rebuild_service(&args);
        return;
    }
    #[cfg(not(all(feature = "ilauncher", target_os = "windows")))]
    if args.iter().any(|a| a == "--mft-service" || a == "--rebuild") {
        eprintln!("--mft-service/--rebuild 需要 --features ilauncher 且在 Windows 下编译");
        std::process::exit(2);
    }

    // --snapshot：不启动 UI，直接测 LiveIndex 进程内搜索
    if let Some(pos) = args.iter().position(|a| a == "--snapshot")
        && let Some(path) = args.get(pos + 1) {
            run_snapshot_bench(path);
            return;
        }

    // --dump-icon <path>：把托盘同款 128×128 RGBA 标志导出（生成 exe 图标用），导完即退
    if let Some(pos) = args.iter().position(|a| a == "--dump-icon")
        && let Some(path) = args.get(pos + 1) {
            #[cfg(target_os = "windows")]
            {
                std::fs::write(path, tray_icon_gen::rgba_pixels()).expect("写图标像素失败");
                println!("✓ 图标像素已导出: {path} ({} 字节)", tray_icon_gen::rgba_pixels().len());
            }
            #[cfg(not(target_os = "windows"))]
            eprintln!("--dump-icon 仅 Windows 构建支持");
            return;
        }

    // ── 索引加载：feature 下后台线程拉起服务并填充 LiveSet；否则 Demo ──────
    #[cfg(all(feature = "ilauncher", target_os = "windows"))]
    let index_set = index_service::imp::init_index_loader();
    #[cfg(not(all(feature = "ilauncher", target_os = "windows")))]
    let index_set = search::LiveSet::empty();

    // ── 审计日志：JSONL 持久化（启动器核心事件已接入；插件沙盒事件随 PluginManager） ──
    let audit_logger = {
        let path = std::env::var_os("LOCALAPPDATA")
            .map(|d| std::path::PathBuf::from(d).join("iLauncher").join("audit.jsonl"))
            .unwrap_or_else(|| std::path::PathBuf::from("audit.jsonl"));
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let logger = audit::AuditLogger::with_persist(&path, audit::DEFAULT_MAX_ENTRIES)
            .unwrap_or_else(|e| {
                eprintln!("⚠️ 审计日志加载失败（降级内存存储）: {e:#}");
                audit::AuditLogger::in_memory(audit::DEFAULT_MAX_ENTRIES)
            });
        let logger = Arc::new(Mutex::new(logger));
        println!("✓ 审计日志已加载（{} 条）", logger.lock().len());
        logger
    };

    // ── 插件市场：已安装注册表加载 + 商店缓存目录（Windows 窗口用） ──────────
    // 须在 PluginManager 之前创建：manager 需要从注册表加载第三方 Lua 命令插件
    #[cfg(windows)]
    let market = {
        let data_dir = std::env::var_os("LOCALAPPDATA")
            .map(|d| std::path::PathBuf::from(d).join("iLauncher"))
            .unwrap_or_else(|| std::path::PathBuf::from("iLauncher"));
        let _ = std::fs::create_dir_all(&data_dir);
        let market = Arc::new(plugin_ui::MarketState::new(
            data_dir.join("plugins"),
            data_dir.join("plugin-cache"),
        ));
        if let Err(e) = market.registry.load_installed() {
            eprintln!("⚠️ 已安装插件扫描失败: {e:#}");
        }
        println!("✓ 插件市场已就绪（已安装 {} 个）", market.registry.list().len());
        market
    };

    // ── 插件系统：内置插件注册 + 沙盒权限表（权限检查事件写上方同一审计管道） ──
    // 顺序：先注入设置里的禁用列表，再加载已安装 Lua 插件（其禁用状态合并进禁用集）
    let mut plugins = plugin::PluginManager::new(audit_logger.clone());
    #[cfg(windows)]
    plugins.set_disabled_plugins(settings::load_disabled_plugins());
    #[cfg(windows)]
    let lua_loaded = plugins.load_installed_lua(&market.registry);
    #[cfg(not(windows))]
    let lua_loaded = 0;
    let plugins = Arc::new(plugins);
    println!(
        "✓ 插件已注册: {} 个（沙盒权限 {} 项，第三方 Lua {} 个）",
        plugins.get_plugins().len(),
        plugins.sandbox().registered_count(),
        lua_loaded
    );

    // ── 剪贴板历史：加载 JSONL + 启动事件监听（feature clipboard） ──────────
    #[cfg(all(feature = "clipboard", target_os = "windows"))]
    let clipboard_store = clipboard_ui::init_clipboard();

    // ── 工作流引擎：JSON 定义加载（目录与 旧版一致；编辑器 UI 不做，直接放 JSON） ──
    #[cfg(windows)]
    let workflows = {
        let dir = std::env::var_os("LOCALAPPDATA")
            .map(|d| std::path::PathBuf::from(d).join("iLauncher").join("workflows"))
            .unwrap_or_else(|| std::path::PathBuf::from("iLauncher").join("workflows"));
        let _ = std::fs::create_dir_all(&dir);
        let http = http_util::client("iLauncher/workflow").expect("工作流 HTTP 客户端创建失败");
        let engine = Arc::new(workflow::WorkflowEngine::new(dir, http));
        match engine.load_workflows() {
            Ok(()) => println!("✓ 工作流已加载（{} 个）", engine.list_workflows().len()),
            Err(e) => eprintln!("⚠️ 工作流加载失败（引擎仍可用）: {e:#}"),
        }
        engine
    };

    // ── AI 助手：对话引擎（配置 + 会话 JSON 持久化；窗口经托盘打开） ──
    #[cfg(windows)]
    let ai_chat = {
        let dir = std::env::var_os("LOCALAPPDATA")
            .map(|d| std::path::PathBuf::from(d).join("iLauncher"))
            .unwrap_or_else(|| std::path::PathBuf::from("iLauncher"));
        let _ = std::fs::create_dir_all(&dir);
        let http = http_util::client("iLauncher/ai-chat").expect("AI HTTP 客户端创建失败");
        Arc::new(ai::AiChat::new(http, dir))
    };

    let (tx, rx) = mpsc::channel();
    setup_tray(tx.clone());
    // 启动即显示主窗口（含 bench 模式）；之后 Esc 隐藏、热键/托盘唤起
    let _ = tx.send(AppSignal::Show(Instant::now()));
    // 开发调试：ILAUNCHER_DEV_OPEN=settings|clipboard|audit|plugins|workflows|ai
    // 启动时直接唤起对应副窗口（免点托盘，供本机冒烟/截图用）
    if let Ok(which) = std::env::var("ILAUNCHER_DEV_OPEN") {
        let sig = match which.as_str() {
            "settings" => Some(AppSignal::ShowSettings),
            "clipboard" => Some(AppSignal::ShowClipboard),
            "audit" => Some(AppSignal::ShowAudit),
            "plugins" => Some(AppSignal::ShowPlugins),
            "workflows" => Some(AppSignal::ShowWorkflows),
            "ai" => Some(AppSignal::ShowAi),
            _ => None,
        };
        if let Some(sig) = sig {
            let _ = tx.send(sig);
        }
    }

    let app = gpui_kit::application().with_assets(Assets);
    app.run(move |cx| {
        gpui_kit::init(cx);
        // 默认 QuitMode::LastWindowClosed（非 macOS）：Esc/失焦销毁主窗口会直接退出进程。
        // 常驻托盘应用改为 Explicit——只有托盘「退出」/主窗「退出」按钮结束进程
        cx.set_quit_mode(gpui_kit::QuitMode::Explicit);
        // 主题：皮肤系统统一初始化（持久化皮肤偏好 > 默认皮肤深/浅偏好 > 系统设置）
        skins::apply_saved(cx);
        // 设置页 model 全局实体（字段值闭包的数据源，托盘/设置页共用）
        #[cfg(windows)]
        settings_ui::init_model(cx);
        let deps = Deps {
            audit_logger,
            plugins,
            #[cfg(windows)]
            market,
            #[cfg(windows)]
            workflows,
            #[cfg(windows)]
            ai_chat,
            #[cfg(all(feature = "clipboard", target_os = "windows"))]
            clipboard_store,
        };
        let mut guard = WindowGuard::new(rx, tx, index_set, deps);
        cx.spawn(move |cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
            println!("✓ 窗口守护轮询已启动");
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(SIGNAL_POLL_MS))
                    .await;

                let mut latest: Option<Instant> = None;
                let mut rebuild = false;
                let mut theme_dark: Option<bool> = None;
                let mut show_clipboard = false;
                let mut show_settings = false;
                let mut show_audit = false;
                let mut show_plugins = false;
                let mut show_workflows = false;
                let mut show_ai = false;
                let mut toggle_preview = false;
                let mut reposition_preview = false;
                while let Ok(sig) = guard.rx.try_recv() {
                    match sig {
                        AppSignal::Show(t) => latest = Some(t),
                        AppSignal::RebuildIndex => rebuild = true,
                        AppSignal::ShowClipboard => show_clipboard = true,
                        AppSignal::ShowSettings => show_settings = true,
                        AppSignal::ShowAudit => show_audit = true,
                        AppSignal::ShowPlugins => show_plugins = true,
                        AppSignal::ShowWorkflows => show_workflows = true,
                        AppSignal::ShowAi => show_ai = true,
                        AppSignal::SetTheme(dark) => theme_dark = Some(dark),
                        AppSignal::TogglePreview => toggle_preview = true,
                        AppSignal::RepositionPreview => reposition_preview = true,
                    }
                }
                if rebuild {
                    #[cfg(all(feature = "ilauncher", target_os = "windows"))]
                    index_service::imp::request_rebuild(&guard.index_set);
                }
                #[cfg(all(feature = "clipboard", target_os = "windows"))]
                if show_clipboard {
                    guard.summon_clipboard(&mut cx);
                }
                #[cfg(not(all(feature = "clipboard", target_os = "windows")))]
                if show_clipboard {
                    println!("⚠️ 剪贴板历史需要 --features clipboard 构建");
                }
                #[cfg(windows)]
                if show_settings {
                    guard.summon_settings(&mut cx);
                }
                #[cfg(not(windows))]
                if show_settings {
                    println!("⚠️ 设置窗口仅 Windows 构建");
                }
                #[cfg(windows)]
                if show_audit {
                    guard.summon_audit(&mut cx);
                }
                #[cfg(not(windows))]
                if show_audit {
                    println!("⚠️ 审计查看器仅 Windows 构建");
                }
                #[cfg(windows)]
                if show_plugins {
                    guard.summon_plugins(&mut cx);
                }
                #[cfg(not(windows))]
                if show_plugins {
                    println!("⚠️ 插件市场窗口仅 Windows 构建");
                }
                #[cfg(windows)]
                if show_workflows {
                    guard.summon_workflows(&mut cx);
                }
                #[cfg(not(windows))]
                if show_workflows {
                    println!("⚠️ 工作流窗口仅 Windows 构建");
                }
                #[cfg(windows)]
                if show_ai {
                    guard.summon_ai(&mut cx);
                }
                #[cfg(not(windows))]
                if show_ai {
                    println!("⚠️ AI 对话窗口仅 Windows 构建");
                }
                if let Some(dark) = theme_dark {
                    cx.update(|cx| {
                        // 皮肤系统统一处理：自定义皮肤下切深/浅 = 回退默认皮肤
                        skins::set_dark_mode(dark, cx);
                        // 同步设置页 model，避免托盘切换后设置页显示过期值
                        settings_ui::sync_theme_model(cx, dark);
                    });
                }
                if let Some(t) = latest {
                    guard.summon(t, &mut cx);
                }
                if toggle_preview {
                    guard.toggle_preview(&mut cx);
                }
                // bounds 跟随：预览开着且主窗口在 → 重贴靠（重建预览窗口）
                if reposition_preview && guard.preview_visible {
                    if let Some((_, launcher)) = &guard.window {
                        let launcher = launcher.clone();
                        guard.open_preview(&mut cx, &launcher);
                    }
                }
            }
            }
        })
        .detach();
    });
}
