//! 剪贴板历史窗口（feature clipboard）：
//! 搜索过滤 + ListItem 列表 + Enter/双击回写系统剪贴板。
//! 数据来自 ilauncher_clipboard 的共享 store（监听线程持续写入）。

use gpui_kit::component::list::ListItem;
use gpui_kit::component::{input::{Input, InputEvent, InputState}, *};
use gpui_kit::*;
use ilauncher_clipboard::{ClipboardItem, ClipboardStore, DEFAULT_CAPACITY};
use parking_lot::Mutex;
use std::sync::Arc;

const PAGE_LIMIT: usize = 100;
const POLL_MS: u64 = 500;

/// 初始化 store（JSONL 持久化）并启动系统剪贴板监听
pub fn init_clipboard() -> Arc<Mutex<ClipboardStore>> {
    let path = std::env::var_os("LOCALAPPDATA")
        .map(|d| std::path::PathBuf::from(d).join("iLauncher").join("clipboard_history.jsonl"))
        .unwrap_or_else(|| std::path::PathBuf::from("clipboard_history.jsonl"));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // 容量：注册表设置 > crate 默认（设置页可调，见 settings_ui）
    let capacity = crate::settings::load_clipboard_capacity().unwrap_or(DEFAULT_CAPACITY);
    let store = ClipboardStore::with_persist(&path, capacity).unwrap_or_else(|e| {
        eprintln!("⚠️ 剪贴板历史加载失败（降级内存存储）: {e:#}");
        ClipboardStore::in_memory(DEFAULT_CAPACITY)
    });
    let store = Arc::new(Mutex::new(store));
    println!("✓ 剪贴板历史已加载（{} 条，容量 {}）", store.lock().len(), capacity);
    let image_dir = path
        .parent()
        .map(|p| p.join("clipboard_images"))
        .unwrap_or_else(|| std::path::PathBuf::from("clipboard_images"));
    ilauncher_clipboard::monitor::start_monitor(store.clone(), image_dir);
    store
}

pub struct ClipboardPanel {
    input: Entity<InputState>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    store: Arc<Mutex<ClipboardStore>>,
    entries: Vec<ClipboardItem>,
    selected: usize,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl ClipboardPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, store: Arc<Mutex<ClipboardStore>>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索剪贴板历史…"));
        let focus = cx.focus_handle();

        let mut this = Self {
            input: input.clone(),
            scroll: UniformListScrollHandle::new(),
            focus,
            store,
            entries: Vec::new(),
            selected: 0,
            status: String::new(),
            _subscriptions: Vec::new(),
        };
        this.refresh(cx);

        // 输入防抖搜索（与主窗口同模式）
        this._subscriptions.push(cx.subscribe_in(&input, window, {
            let input = input.clone();
            move |this, _, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    this.apply_query_debounced(value, cx);
                }
            }
        }));

        // 轮询刷新：监听线程持续写入 store，窗口侧定时同步最新内容。
        // 500ms 粒度对剪贴板场景足够；后续可改 store 版本号 + 通知
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

    fn query(&self, cx: &mut Context<Self>) -> String {
        self.input.read(cx).value().to_string()
    }

    /// 从 store 重载列表（尊重当前搜索词）
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        let store = self.store.lock();
        self.entries = if query.trim().is_empty() {
            store.list(0, PAGE_LIMIT).to_vec()
        } else {
            store.search(&query, PAGE_LIMIT)
        };
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
        cx.notify();
    }

    fn apply_query_debounced(&mut self, query: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(80))
                .await;
            let _ = this.update(cx, |this, cx| {
                let _ = query; // 词在 refresh 时从 input 重读，天然只执行最新
                this.refresh(cx);
            });
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

    /// 回写选中项到系统剪贴板（按类型分派）
    fn copy_selected(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.entries.get(self.selected) else { return };
        let result = if item.kind == "image" {
            ilauncher_clipboard::copy_image(&item.content)
                .map(|()| format!("已复制图片 #{}（{}）", item.id, item.preview))
        } else {
            ilauncher_clipboard::copy_text(&item.content)
                .map(|()| format!("已复制 #{}（{} 字符）", item.id, item.content.chars().count()))
        };
        match result {
            Ok(msg) => {
                self.status = msg;
                cx.notify();
            }
            Err(e) => self.status = format!("复制失败: {e:#}"),
        }
    }
}

impl Render for ClipboardPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let theme_for_list = theme.clone();
        let entries = self.entries.clone();
        let selected = self.selected;
        let panel = cx.entity();
        let status = self.status.clone();
        let total = self.store.lock().len();

        v_flex()
            .id("clipboard-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                match ev.keystroke.key.as_str() {
                    "up" => this.move_selection(-1, cx),
                    "down" => this.move_selection(1, cx),
                    "enter" => this.copy_selected(cx),
                    "escape" => window.remove_window(),
                    _ => {}
                }
            }))
            .size_full()
            .p_3()
            .gap_2()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(Input::new(&self.input).w_full())
            .child(
                div()
                    .id("clipboard-results")
                    .flex_1()
                    .child(
                        uniform_list("clipboard-list", entries.len(), {
                            let panel = panel.clone();
                            move |visible_range, _window, _cx| {
                                visible_range
                                    .map(|ix| {
                                        let item = &entries[ix];
                                        let is_selected = ix == selected;
                                        ListItem::new(ix)
                                            .selected(is_selected)
                                            .on_click({
                                                let panel = panel.clone();
                                                move |_, _, cx| {
                                                    panel.update(cx, |this: &mut ClipboardPanel, cx| {
                                                        this.selected = ix;
                                                        this.copy_selected(cx);
                                                    });
                                                }
                                            })
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .justify_between()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .truncate()
                                                            .child(item.preview.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme_for_list.muted_foreground)
                                                            .child(format!("#{}", item.id)),
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
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(if status.is_empty() {
                        format!("{} 条历史 · ↑↓ 选择 · Enter/点击 复制 · Esc 关闭", total)
                    } else {
                        status
                    }),
            )
    }
}
