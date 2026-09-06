//! 工作流管理窗口（Windows）。
//! 单列表：名称 / 触发器 / 步骤数 / 启用开关 + 运行 + 删除。
//! 编辑器 UI 不做（对齐方案）：工作流 JSON 直接放入
//! `%LOCALAPPDATA%\iLauncher\workflows\<id>.json`，重启或点"刷新"加载。

use parking_lot::Mutex;
use std::sync::Arc;

use gpui_kit::component::list::ListItem;
use gpui_kit::component::{button::Button, checkbox::Checkbox, *};
use gpui_kit::*;

use crate::audit::{AuditEventType, AuditLogger, AuditSeverity};
use crate::i18n::t;
use crate::workflow::{Workflow, WorkflowEngine, WorkflowTrigger};

/// 触发器一句话描述（列表副标题用）
fn trigger_summary(trigger: &WorkflowTrigger) -> String {
    match trigger {
        WorkflowTrigger::Manual { keyword } => t!("workflow.trigger_manual", keyword = keyword.as_str()).to_string(),
        WorkflowTrigger::Hotkey { key } => t!("workflow.trigger_hotkey", key = key.as_str()).to_string(),
        WorkflowTrigger::Schedule { cron } => t!("workflow.trigger_schedule", cron = cron.as_str()).to_string(),
        WorkflowTrigger::Event { event_type } => t!("workflow.trigger_event", event = event_type.as_str()).to_string(),
    }
}

pub struct WorkflowPanel {
    workflows: Vec<Workflow>,
    engine: Arc<WorkflowEngine>,
    audit_logger: Arc<Mutex<AuditLogger>>,
    status: String,
    focus: FocusHandle,
}

impl WorkflowPanel {
    pub fn new(
        _window: &mut Window,
        _cx: &mut Context<Self>,
        engine: Arc<WorkflowEngine>,
        audit_logger: Arc<Mutex<AuditLogger>>,
    ) -> Self {
        let workflows = engine.list_workflows();
        Self {
            workflows,
            engine,
            audit_logger,
            status: String::new(),
            focus: _cx.focus_handle(),
        }
    }

    fn set_status(&mut self, cx: &mut Context<Self>, status: impl Into<String>) {
        self.status = status.into();
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        match self.engine.load_workflows() {
            Ok(()) => {
                self.workflows = self.engine.list_workflows();
                self.set_status(cx, t!("workflow.loaded", count = self.workflows.len()).to_string());
            }
            Err(e) => self.set_status(cx, t!("workflow.load_failed", error = format!("{e:#}")).to_string()),
        }
    }

    /// 启用/禁用：改定义后走引擎 save（内存 + 落盘）
    fn toggle_enabled(&mut self, id: String, enabled: bool, cx: &mut Context<Self>) {
        let Some(mut wf) = self.engine.get_workflow(&id) else {
            self.set_status(cx, t!("workflow.not_found", id = id.as_str()).to_string());
            return;
        };
        wf.enabled = enabled;
        match self.engine.save_workflow(wf) {
            Ok(()) => {
                self.workflows = self.engine.list_workflows();
                self.set_status(cx, format!("{id} → {}", if enabled { "启用" } else { "禁用" }));
            }
            Err(e) => self.set_status(cx, t!("workflow.save_failed", error = format!("{e:#}")).to_string()),
        }
    }

    fn delete(&mut self, id: String, cx: &mut Context<Self>) {
        match self.engine.delete_workflow(&id) {
            Ok(()) => {
                self.workflows = self.engine.list_workflows();
                self.set_status(cx, t!("workflow.deleted_ok", id = id.as_str()).to_string());
            }
            Err(e) => self.set_status(cx, t!("workflow.delete_failed", error = format!("{e:#}")).to_string()),
        }
    }

    /// 运行：与启动器 Enter 分支同一语义——后台执行，副作用回主线程
    fn run(&mut self, id: String, cx: &mut Context<Self>) {
        self.set_status(cx, t!("workflow.running", id = id.as_str()).to_string());
        let engine = self.engine.clone();
        let audit_logger = self.audit_logger.clone();
        cx.spawn(async move |this, cx| {
            let mut effects = Vec::new();
            let result = engine.execute_workflow(&id, Default::default(), &mut effects).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        consume_effects(effects, cx);
                        this.set_status(cx, t!("workflow.run_ok", id = id.as_str()).to_string());
                        log_run(&audit_logger, &id, true);
                    }
                    Err(e) => {
                        this.set_status(cx, t!("workflow.run_failed", id = id.as_str(), error = format!("{e:#}")).to_string());
                        log_run(&audit_logger, &id, false);
                    }
                }
            });
        })
        .detach();
    }
}

/// 消费工作流副作用（启动器 Enter 与管理窗口"运行"共用）：
/// 剪贴板写主线程剪贴板；通知走控制台（gpui 无内建通知组件）
pub fn consume_effects(effects: Vec<crate::workflow::WorkflowEffect>, cx: &mut App) {
    for effect in effects {
        match effect {
            crate::workflow::WorkflowEffect::CopyToClipboard(text) => {
                println!("WORKFLOW_COPY {}", text);
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            crate::workflow::WorkflowEffect::ShowNotification { title, message } => {
                println!("WORKFLOW_NOTIFY {}: {}", title, message);
            }
        }
    }
}

/// 工作流执行审计（两入口共用）：ProgramExecution，来源标记 "workflow"
pub fn log_run(audit_logger: &Arc<Mutex<AuditLogger>>, id: &str, allowed: bool) {
    audit_logger.lock().log(
        AuditEventType::ProgramExecution {
            plugin_id: "workflow".into(),
            program: id.into(),
            allowed,
        },
        if allowed { AuditSeverity::Info } else { AuditSeverity::Warning },
    );
}

impl Render for WorkflowPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let theme_for_list = theme.clone();
        let workflows = self.workflows.clone();
        let status = self.status.clone();

        v_flex()
            .id("workflow-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_this, ev: &KeyDownEvent, window, _cx| {
                if ev.keystroke.key.as_str() == "escape" {
                    window.remove_window();
                }
            }))
            .size_full()
            .p_3()
            .gap_2()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().child(t!("workflow.header", count = workflows.len()).to_string()))
                    .child(
                        Button::new("workflow-refresh")
                            .small()
                            .label(t!("workflow.refresh").to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .child(
                div()
                    .id("workflow-results")
                    .flex_1()
                    .child(
                        uniform_list("workflow-list", workflows.len(), {
                            let panel = cx.entity();
                            move |visible_range, _window, _cx| {
                                visible_range
                                    .map(|ix| {
                                        let wf = &workflows[ix];
                                        let id = wf.id.clone();
                                        let title = wf.name.clone();
                                        let subtitle = t!("workflow.subtitle", trigger = trigger_summary(&wf.trigger), count = wf.steps.len()).to_string();
                                        let enabled = wf.enabled;
                                        ListItem::new(ix)
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .justify_between()
                                                    .gap_2()
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .items_center()
                                                            .min_w_0()
                                                            .child(
                                                                Checkbox::new(("enabled", ix))
                                                                    .checked(enabled)
                                                                    .on_click({
                                                                        let panel = panel.clone();
                                                                        let id = id.clone();
                                                                        move |checked: &bool, _window, cx: &mut App| {
                                                                            let checked = *checked;
                                                                            panel.update(cx, |this: &mut WorkflowPanel, cx| {
                                                                                this.toggle_enabled(id.clone(), checked, cx);
                                                                            });
                                                                        }
                                                                    }),
                                                            )
                                                            .child(
                                                                v_flex()
                                                                    .min_w_0()
                                                                    .child(div().text_sm().child(title))
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(theme_for_list.muted_foreground)
                                                                            .truncate()
                                                                            .child(if wf.description.is_empty() {
                                                                                subtitle
                                                                            } else {
                                                                                t!("workflow.subtitle_with_desc", subtitle = subtitle.as_str(), desc = wf.description.as_str()).to_string()
                                                                            }),
                                                                    ),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .gap_1()
                                                            .child(
                                                                Button::new(("run", ix))
                                                                    .small()
                                                                    .label(t!("workflow.run").to_string())
                                                                    .on_click({
                                                                        let panel = panel.clone();
                                                                        let id = id.clone();
                                                                        move |_, _, cx| {
                                                                            panel.update(cx, |this: &mut WorkflowPanel, cx| {
                                                                                this.run(id.clone(), cx);
                                                                            });
                                                                        }
                                                                    }),
                                                            )
                                                            .child(
                                                                Button::new(("delete", ix))
                                                                    .small()
                                                                    .label(t!("workflow.delete").to_string())
                                                                    .on_click({
                                                                        let panel = panel.clone();
                                                                        move |_, _, cx| {
                                                                            panel.update(cx, |this: &mut WorkflowPanel, cx| {
                                                                                this.delete(id.clone(), cx);
                                                                            });
                                                                        }
                                                                    }),
                                                            ),
                                                    ),
                                            )
                                    })
                                    .collect::<Vec<_>>()
                            }
                        })
                        .size_full(),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .pt_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(if status.is_empty() {
                                t!("workflow.footer_status").to_string()
                            } else {
                                status
                            }),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(crate::window_drag::kbd_pill(t!("main.hint_hide"), &theme)),
                    ),
            )
    }
}
