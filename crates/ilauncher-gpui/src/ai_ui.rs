//! AI 对话窗口（Windows）。
//! 左侧会话列表 + 右侧聊天区（Markdown 渲染）+ 可折叠设置区。
//! 引擎在 `ai.rs`（跨平台可单测），本文件仅装配 UI。

use std::sync::Arc;

use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::theme::Theme;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::ai::{AiChat, DEFAULT_TITLE};
use crate::markdown::{MdBlock, MdInline, parse_blocks};

/// provider 循环切换顺序（与 ai.rs 支持集一致）
const PROVIDERS: [&str; 7] = ["openai", "anthropic", "github", "deepseek", "gemini", "ollama", "custom"];

pub struct AiChatPanel {
    chat: Arc<AiChat>,
    input: Entity<InputState>,
    key_input: Entity<InputState>,
    model_input: Entity<InputState>,
    base_input: Entity<InputState>,
    settings_open: bool,
    busy: bool,
    status: String,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl AiChatPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, chat: Arc<AiChat>) -> Self {
        chat.load();
        let config = chat.config();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("输入消息，Enter 发送…"));
        let key_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("API Key（本地明文存储）")
        });
        let model_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("模型，如 gpt-3.5-turbo / claude-3-sonnet")
        });
        let base_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Base URL（可选，留空用 provider 默认）")
        });
        let focus = cx.focus_handle();

        let mut this = Self {
            chat,
            input: input.clone(),
            key_input,
            model_input,
            base_input,
            settings_open: false,
            busy: false,
            status: String::new(),
            focus,
            _subscriptions: Vec::new(),
        };
        this.fill_settings_inputs(&config, window, cx);

        this._subscriptions.push(cx.subscribe_in(&input, window, {
            move |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary: false, shift: false } = ev {
                    this.send(window, cx);
                }
            }
        }));
        this
    }

    fn set_status(&mut self, cx: &mut Context<Self>, status: impl Into<String>) {
        self.status = status.into();
        cx.notify();
    }

    fn fill_settings_inputs(
        &mut self,
        config: &crate::ai::AIConfig,
        window: &mut Window,
        cx: &mut App,
    ) {
        let base = config.base_url.clone().unwrap_or_default();
        for (entity, value) in [
            (&self.key_input, config.api_key.clone()),
            (&self.model_input, config.model.clone()),
            (&self.base_input, base),
        ] {
            entity.update(cx, |s, cx| s.set_value(value, window, cx));
        }
    }

    /// 发送当前输入（Enter 或按钮）
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let message = self.input.read(cx).value().trim().to_string();
        if message.is_empty() {
            return;
        }
        if self.chat.config().api_key.is_empty() {
            self.settings_open = true;
            self.set_status(cx, "请先在设置区填写 API Key");
            return;
        }
        self.input.update(cx, |s, cx| s.set_value(String::new(), window, cx));
        self.busy = true;
        self.set_status(cx, "等待 AI 回复…");
        let chat = self.chat.clone();
        cx.spawn(async move |this, cx| {
            let result = chat.send_message(&message).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(_) => this.status = String::new(),
                    Err(e) => this.status = format!("发送失败: {e:#}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn new_conversation(&mut self, cx: &mut Context<Self>) {
        self.chat.create_conversation(DEFAULT_TITLE.into());
        self.status = String::new();
        cx.notify();
    }

    fn switch_conversation(&mut self, id: String, cx: &mut Context<Self>) {
        self.chat.switch_conversation(id);
        cx.notify();
    }

    fn delete_conversation(&mut self, id: String, cx: &mut Context<Self>) {
        self.chat.delete_conversation(&id);
        self.set_status(cx, "已删除");
    }

    /// provider 循环到下一个
    fn cycle_provider(&mut self, cx: &mut Context<Self>) {
        let mut config = self.chat.config();
        let next = PROVIDERS
            .iter()
            .position(|p| *p == config.provider)
            .map(|i| PROVIDERS[(i + 1) % PROVIDERS.len()])
            .unwrap_or(PROVIDERS[0]);
        config.provider = next.into();
        if let Err(e) = self.chat.save_config(config) {
            self.set_status(cx, format!("保存配置失败: {e:#}"));
        }
        cx.notify();
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let mut config = self.chat.config();
        config.api_key = self.key_input.read(cx).value().trim().to_string();
        config.model = self.model_input.read(cx).value().trim().to_string();
        let base = self.base_input.read(cx).value().trim().to_string();
        config.base_url = if base.is_empty() { None } else { Some(base) };
        match self.chat.save_config(config) {
            Ok(()) => self.set_status(cx, "✓ 配置已保存"),
            Err(e) => self.set_status(cx, format!("保存配置失败: {e:#}")),
        }
    }
}

// ── Markdown 渲染：块/行内 → gpui 元素 ─────────────────────────────────────

fn render_inlines(runs: &[MdInline], theme: &Theme) -> Vec<AnyElement> {
    runs.iter()
        .map(|run| {
            // 软换行折叠为空格（聊天场景阅读友好）
            let text = run.text.replace('\n', " ");
            let mut el = div().child(text);
            if run.style.bold {
                el = el.font_weight(FontWeight::SEMIBOLD);
            }
            if run.style.italic {
                el = el.italic();
            }
            if run.style.code {
                el = el
                    .font_family(theme.mono_font_family.clone())
                    .text_xs()
                    .px_1()
                    .rounded(theme.radius)
                    .bg(theme.secondary)
                    .text_color(theme.primary);
            }
            if run.style.link.is_some() {
                el = el.text_color(theme.primary).underline();
            }
            el.into_any_element()
        })
        .collect()
}

fn render_blocks(text: &str, theme: &Theme) -> Vec<AnyElement> {
    parse_blocks(text)
        .iter()
        .map(|block| {
            match block {
                MdBlock::Heading(level, runs) => {
                    let size = match level {
                        1 => div().text_base(),
                        2 => div().text_sm(),
                        _ => div().text_xs(),
                    };
                    size.font_weight(FontWeight::BOLD)
                        .py_1()
                        .child(h_flex().flex_wrap().children(render_inlines(runs, theme)))
                        .into_any_element()
                }
                MdBlock::Paragraph(runs) => div()
                    .text_sm()
                    .py_0p5()
                    .child(h_flex().flex_wrap().children(render_inlines(runs, theme)))
                    .into_any_element(),
                MdBlock::Code { lang, text } => {
                    let label = if lang.is_empty() { String::new() } else { format!("{lang} · ") };
                    v_flex()
                        .gap_1()
                        .py_1()
                        .child(
                            div().text_xs().text_color(theme.muted_foreground).child(format!("{label}代码")),
                        )
                        .child(
                            div()
                                .w_full()
                                .p_2()
                                .rounded(theme.radius)
                                .bg(theme.muted)
                                .font_family(theme.mono_font_family.clone())
                                .text_xs()
                                .children(text.lines().map(|line| div().child(line.to_string()))),
                        )
                        .into_any_element()
                }
                MdBlock::List { ordered, items } => v_flex()
                    .gap_0p5()
                    .py_0p5()
                    .children(items.iter().enumerate().map(|(i, runs)| {
                        let marker = if *ordered { format!("{}.", i + 1) } else { "·".into() };
                        h_flex()
                            .gap_2()
                            .child(div().text_xs().text_color(theme.muted_foreground).child(marker).w_4())
                            .child(h_flex().flex_wrap().children(render_inlines(runs, theme)))
                    }))
                    .into_any_element(),
                MdBlock::Quote(runs) => div()
                    .my_1()
                    .pl_2()
                    .border_l_1()
                    .border_color(theme.border)
                    .text_color(theme.muted_foreground)
                    .child(h_flex().flex_wrap().children(render_inlines(runs, theme)))
                    .into_any_element(),
                MdBlock::Rule => {
                    div().h_px().w_full().my_2().bg(theme.border).into_any_element()
                }
            }
        })
        .collect()
}

impl Render for AiChatPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let conversations = self.chat.conversations();
        let current = self.chat.current_conversation();
        let current_id = current.as_ref().map(|c| c.id.clone());
        let busy = self.busy;
        let status = self.status.clone();
        let settings_open = self.settings_open;
        let config = self.chat.config();

        // 左侧会话栏
        let sidebar = v_flex()
            .w(px(200.))
            .h_full()
            .gap_1()
            .p_2()
            .border_r_1()
            .border_color(theme.border)
            .child(
                Button::new("ai-new")
                    .small()
                    .label("＋ 新对话")
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| this.new_conversation(cx))),
            )
            .child(
                div().id("ai-conversations").flex_1().overflow_y_scroll().child(
                    uniform_list("ai-conv-list", conversations.len(), {
                        let panel = cx.entity();
                        let theme = theme.clone();
                        let current_id = current_id.clone();
                        move |visible_range, _window, _cx| {
                            visible_range
                                .map(|ix| {
                                    let conv = &conversations[ix];
                                    let id = conv.id.clone();
                                    let selected = current_id.as_deref() == Some(conv.id.as_str());
                                    let last = conv
                                        .messages
                                        .last()
                                        .map(|m| {
                                            let preview: String =
                                                m.content.chars().take(24).collect();
                                            format!("{}: {}", m.role, preview)
                                        })
                                        .unwrap_or_else(|| "（空对话）".into());
                                    ListItem::new(ix)
                                        .child(
                                            v_flex()
                                                .w_full()
                                                .gap_0p5()
                                                .child(
                                                    h_flex()
                                                        .w_full()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .truncate()
                                                                .font_weight(if selected {
                                                                    FontWeight::SEMIBOLD
                                                                } else {
                                                                    FontWeight::NORMAL
                                                                })
                                                                .child(conv.title.clone()),
                                                        )
                                                        .child(
                                                            Button::new(("del", ix))
                                                                .xsmall()
                                                                .label("✕")
                                                                .on_click({
                                                                    let panel = panel.clone();
                                                                    let id = id.clone();
                                                                    move |_, _, cx| {
                                                                        panel.update(cx, |this: &mut AiChatPanel, cx| {
                                                                            this.delete_conversation(id.clone(), cx);
                                                                        });
                                                                    }
                                                                }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .truncate()
                                                        .text_color(theme.muted_foreground)
                                                        .child(last),
                                                ),
                                        )
                                        .on_click({
                                            let panel = panel.clone();
                                            let id = id.clone();
                                            move |_, _, cx| {
                                                panel.update(cx, |this: &mut AiChatPanel, cx| {
                                                    this.switch_conversation(id.clone(), cx);
                                                });
                                            }
                                        })
                                })
                                .collect::<Vec<_>>()
                        }
                    })
                    .size_full(),
                ),
            );

        // 右侧主区：消息 + 设置 + 输入
        let mut main = v_flex().flex_1().h_full().gap_2().p_3();

        main = main.child(
            h_flex()
                .w_full()
                .justify_between()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(
                    current.as_ref().map(|c| c.title.clone()).unwrap_or_else(|| "AI 对话".into()),
                ))
                .child(
                    Button::new("ai-settings-toggle")
                        .small()
                        .label(if settings_open { "● 设置" } else { "设置" })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings_open = !this.settings_open;
                            cx.notify();
                        })),
                ),
        );

        // 消息区
        let messages_el = match &current {
            Some(conv) if !conv.messages.is_empty() => div()
                .id("ai-messages")
                .flex_1()
                .overflow_y_scroll()
                .gap_3()
                .children(conv.messages.iter().map(|m| {
                    let is_user = m.role == "user";
                    let bubble = v_flex()
                        .w_full()
                        .gap_1()
                        .child(
                            div().text_xs().text_color(theme.muted_foreground).child(if is_user {
                                "你"
                            } else {
                                "AI"
                            }),
                        )
                        .child(
                            div()
                                .max_w(px(560.))
                                .px_3()
                                .py_2()
                                .rounded(theme.radius_lg)
                                .bg(if is_user { theme.secondary } else { theme.muted })
                                .children(render_blocks(&m.content, &theme)),
                        );
                    if is_user {
                        bubble.items_end()
                    } else {
                        bubble
                    }
                })),
            _ => div()
                .id("ai-messages-empty")
                .flex_1()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("输入消息开始对话，或在左侧选择历史会话"),
                ),
        };
        main = main.child(messages_el);

        // 设置区
        if settings_open {
            main = main.child(
                v_flex()
                    .w_full()
                    .gap_2()
                    .p_2()
                    .rounded(theme.radius)
                    .bg(theme.muted)
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(div().text_xs().text_color(theme.muted_foreground).child("Provider"))
                            .child(
                                Button::new("ai-provider")
                                    .small()
                                    .label(format!("{}（点击切换）", config.provider))
                                    .on_click(cx.listener(|this, _, _, cx| this.cycle_provider(cx))),
                            ),
                    )
                    .child(Input::new(&self.key_input).w_full())
                    .child(Input::new(&self.model_input).w_full())
                    .child(Input::new(&self.base_input).w_full())
                    .child(
                        Button::new("ai-save-settings")
                            .small()
                            .label("保存配置")
                            .on_click(cx.listener(|this, _, _, cx| this.save_settings(cx))),
                    ),
            );
        }

        // 输入行
        main = main.child(
            h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .child(Input::new(&self.input).flex_1())
                .child(
                    Button::new("ai-send")
                        .small()
                        .label(if busy { "…" } else { "发送" })
                        .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
                ),
        );

        // 状态栏
        main = main.child(
            h_flex()
                .w_full()
                .child(
                    div().text_xs().text_color(theme.muted_foreground).child(if status.is_empty() {
                        "Esc 关闭 · Enter 发送 · Markdown 渲染（无语法高亮）".to_string()
                    } else {
                        status
                    }),
                ),
        );

        h_flex()
            .id("ai-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_this, ev: &KeyDownEvent, window, _cx| {
                if ev.keystroke.key.as_str() == "escape" {
                    window.remove_window();
                }
            }))
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(sidebar)
            .child(main)
    }
}
