//! 审计日志查看器（Windows）：统计头 + 搜索过滤 + 列表 + 清空/导出。
//! 数据来自共享 AuditLogger（启动器核心已接入 ProgramExecution 事件；
//! PluginManager 落地后插件沙盒事件同管道注入）。
//! 轮询刷新与剪贴板历史同模式（500ms，后续可改版本号通知）。

use std::sync::Arc;

use gpui_kit::component::list::ListItem;
use gpui_kit::component::{button::Button, input::{Input, InputEvent, InputState}, *};
use gpui_kit::*;
use parking_lot::Mutex;

use crate::audit::{AuditLogEntry, AuditLogger, AuditSeverity};

const PAGE_LIMIT: usize = 200;
const POLL_MS: u64 = 500;

pub struct AuditPanel {
    input: Entity<InputState>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    logger: Arc<Mutex<AuditLogger>>,
    entries: Vec<AuditLogEntry>,
    violations_only: bool,
    selected: usize,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl AuditPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, logger: Arc<Mutex<AuditLogger>>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索插件 / 事件 / 详情…"));
        let focus = cx.focus_handle();

        let mut this = Self {
            input: input.clone(),
            scroll: UniformListScrollHandle::new(),
            focus,
            logger,
            entries: Vec::new(),
            violations_only: false,
            selected: 0,
            status: String::new(),
            _subscriptions: Vec::new(),
        };
        this.refresh(cx);

        // 输入防抖过滤（与剪贴板历史同模式：词在 refresh 时从 input 重读）
        this._subscriptions.push(cx.subscribe_in(&input, window, {
            move |this, _, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.apply_filter_debounced(cx);
                }
            }
        }));

        // 轮询刷新：生产者（启动/后续插件沙盒）持续写入 logger
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(POLL_MS))
                    .await;
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break; // 窗口实体已销毁
                }
            }
        })
        .detach();

        this
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().trim().to_lowercase();
        let logger = self.logger.lock();
        let matched = |e: &AuditLogEntry| {
            query.is_empty()
                || e.event_type.plugin_id().to_lowercase().contains(&query)
                || e.event_type.summarize().to_lowercase().contains(&query)
        };
        let mut entries: Vec<AuditLogEntry> = if self.violations_only {
            logger.violations().into_iter().filter(|e| matched(e)).take(PAGE_LIMIT).cloned().collect()
        } else {
            logger.entries().iter().rev().filter(|e| matched(e)).take(PAGE_LIMIT).cloned().collect()
        };
        // 详情行按时间新→旧；导出 JSON 也按这个顺序
        entries.shrink_to_fit();
        self.entries = entries;
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
        cx.notify();
    }

    fn apply_filter_debounced(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(80))
                .await;
            let _ = this.update(cx, |this, cx| this.refresh(cx));
        })
        .detach();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        self.selected = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        self.scroll.scroll_to_item(self.selected, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn clear_all(&mut self, cx: &mut Context<Self>) {
        self.logger.lock().clear();
        self.status = "审计日志已清空".into();
        self.refresh(cx);
    }

    /// 导出 pretty JSON 到数据目录，并打开所在目录
    fn export(&mut self, cx: &mut Context<Self>) {
        let dir = std::env::var_os("LOCALAPPDATA")
            .map(|d| std::path::PathBuf::from(d).join("iLauncher"))
            .unwrap_or_else(|| std::path::PathBuf::from("iLauncher"));
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("audit_export_{stamp}.json"));
        let result: anyhow::Result<std::path::PathBuf> = (|| {
            let json = self.logger.lock().export_json()?;
            let _ = std::fs::create_dir_all(&dir);
            std::fs::write(&path, json)?;
            opener::open(&dir)?;
            Ok(path)
        })();
        self.status = match result {
            Ok(p) => format!("已导出到 {}", p.display()),
            Err(e) => format!("导出失败: {e:#}"),
        };
        cx.notify();
    }
}

impl Render for AuditPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let theme_for_list = theme.clone();
        let entries = self.entries.clone();
        let selected = self.selected;
        let panel = cx.entity();
        let status = self.status.clone();
        let logger = self.logger.lock();
        let stats = logger.statistics();
        let total = logger.len();
        drop(logger);

        v_flex()
            .id("audit-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                match ev.keystroke.key.as_str() {
                    "up" => this.move_selection(-1, cx),
                    "down" => this.move_selection(1, cx),
                    "escape" => window.remove_window(),
                    _ => {}
                }
            }))
            .size_full()
            .p_3()
            .gap_2()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(stats.summarize()),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("audit-violations-toggle")
                            .small()
                            .label(if self.violations_only { "仅违规：开" } else { "仅违规：关" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.violations_only = !this.violations_only;
                                this.refresh(cx);
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("仅显示违规尝试事件（ViolationAttempt）"),
                    ),
            )
            .child(Input::new(&self.input).w_full())
            .child(
                div()
                    .id("audit-results")
                    .flex_1()
                    .child(
                        uniform_list("audit-list", entries.len(), {
                            let panel = panel.clone();
                            move |visible_range, _window, _cx| {
                                visible_range
                                    .map(|ix| {
                                        let item = &entries[ix];
                                        let is_selected = ix == selected;
                                        let severity_color = match item.severity {
                                            AuditSeverity::Critical => theme_for_list.danger,
                                            AuditSeverity::Warning => theme_for_list.warning,
                                            AuditSeverity::Info => theme_for_list.muted_foreground,
                                        };
                                        ListItem::new(ix)
                                            .selected(is_selected)
                                            .on_click({
                                                let panel = panel.clone();
                                                move |_, _, cx| {
                                                    panel.update(cx, |this: &mut AuditPanel, cx| {
                                                        this.selected = ix;
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .justify_between()
                                                    .gap_2()
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .min_w_0()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(severity_color)
                                                                    .child(item.severity.label()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .truncate()
                                                                    .child(item.event_type.summarize()),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme_for_list.muted_foreground)
                                                            .child(crate::preview::format_unix_utc(item.timestamp)),
                                                    ),
                                            )
                                    })
                                    .collect::<Vec<_>>()
                            }
                        })
                        .size_full()
                        .track_scroll(&self.scroll),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(if status.is_empty() {
                                format!("{} 条审计 · ↑↓ 选择 · Esc 关闭", total)
                            } else {
                                status
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("audit-export")
                                    .small()
                                    .label("导出 JSON")
                                    .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                            )
                            .child(
                                Button::new("audit-clear")
                                    .small()
                                    .label("清空")
                                    .on_click(cx.listener(|this, _, _, cx| this.clear_all(cx))),
                            ),
                    ),
            )
    }
}
