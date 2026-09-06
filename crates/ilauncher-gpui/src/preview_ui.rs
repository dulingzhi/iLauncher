//! 文件预览窗口：独立顶级窗口，贴主窗口右边框（默认隐藏，Alt+P 切换）。
//!
//! 数据不自有：监听主窗口 Launcher 实体（选中变化/预览读取完成均走 cx.notify），
//! 镜像其 preview 状态；文件元信息（大小/创建/修改）与 SHA256 在本窗口后台执行器计算。
//! 主窗口销毁（Esc/失焦）→ Launcher 实体释放 → observe_release 联动关闭本窗口。

use std::path::PathBuf;

use gpui_kit::component::{button::Button, *};
use gpui_kit::*;

use crate::i18n::t;
use crate::preview::{self, FileMeta, FilePreview};
use crate::Launcher;

/// 哈希计算状态机：空闲（None）/ 结果
#[derive(Debug, Clone)]
enum HashState {
    Computing,
    Done(Result<String, String>),
}

pub struct PreviewPanel {
    /// 当前选中项（文件名, 路径）
    entry: Option<(String, String)>,
    /// 主窗口已读取的预览内容（Err 为展示用错误文本）
    preview: Option<(PathBuf, Result<FilePreview, String>)>,
    /// 元信息（后台读取；>1MB/二进制文件也有）
    meta: Option<Result<FileMeta, String>>,
    /// 元信息读取代次（选中快速切换丢弃过期结果）
    meta_gen: usize,
    hash: Option<HashState>,
    hash_gen: usize,
    _subscriptions: Vec<Subscription>,
}

impl PreviewPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, launcher: Entity<Launcher>) -> Self {
        let mut this = Self {
            entry: None,
            preview: None,
            meta: None,
            meta_gen: 0,
            hash: None,
            hash_gen: 0,
            _subscriptions: Vec::new(),
        };
        this.pull_state(&launcher, cx);

        // 主窗口每次 cx.notify（选中移动/预览读取完成/列表刷新）都镜像一次状态
        this._subscriptions.push(cx.observe(&launcher, {
            let launcher = launcher.downgrade();
            move |this, _, cx| {
                if let Some(l) = launcher.upgrade() {
                    this.pull_state(&l, cx);
                }
            }
        }));
        // 主窗口销毁 → 本窗口同步关闭（Esc/失焦自动隐藏语义一致）
        this._subscriptions.push(cx.observe_release_in(&launcher, window, |_, _, window, _| {
            window.remove_window();
        }));
        this
    }

    /// 从 Launcher 镜像选中项与预览状态；选中文件变化时后台读元信息
    fn pull_state(&mut self, launcher: &Entity<Launcher>, cx: &mut Context<Self>) {
        let state = launcher.read(cx).preview_state();
        self.preview = state.preview;
        let new_entry = state.entry;
        if new_entry != self.entry {
            self.entry = new_entry;
            self.hash = None;
            self.meta = None;
            if let Some((_, path)) = &self.entry {
                self.meta_gen += 1;
                let gen_id = self.meta_gen;
                let path = PathBuf::from(path);
                cx.spawn(async move |this, cx| {
                    let result = preview::file_meta(&path).map_err(|e| format!("{e:#}"));
                    let _ = this.update(cx, |this, cx| {
                        if this.meta_gen == gen_id {
                            this.meta = Some(result);
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    /// 计算 SHA256（后台执行器；大文件也可算，无 1MB 限制）
    fn compute_hash(&mut self, cx: &mut Context<Self>) {
        let Some((_, path)) = &self.entry else { return };
        self.hash = Some(HashState::Computing);
        self.hash_gen += 1;
        let gen_id = self.hash_gen;
        let path = PathBuf::from(path);
        cx.spawn(async move |this, cx| {
            let result = std::fs::read(&path)
                .map(|data| {
                    use sha2::Digest as _;
                    let mut out = String::with_capacity(data.len() * 2);
                    for b in sha2::Sha256::digest(&data) {
                        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
                        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
                    }
                    out
                })
                .map_err(|e| e.to_string());
            let _ = this.update(cx, |this, cx| {
                if this.hash_gen == gen_id {
                    this.hash = Some(HashState::Done(result));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 元信息行（label + value 两列，对齐 Listary 预览样式）
    fn meta_row(label: &str, value: String, theme: &gpui_kit::component::theme::Theme) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w(px(64.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(label.to_string()),
            )
            .child(div().flex_1().min_w_0().text_xs().child(value))
    }

    fn render_body(&self, theme: &gpui_kit::component::theme::Theme) -> gpui_kit::AnyElement {
        use preview::FileType;
        let muted = theme.muted_foreground;
        let Some((path, result)) = &self.preview else {
            return div()
                .id("preview-empty")
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Inbox)
                        .size(px(28.))
                        .text_color(muted),
                )
                .child(div().text_xs().text_color(muted).child(t!("main.preview_select").to_string()))
                .into_any_element();
        };
        match result {
            Ok(p) => match p.file_type {
                FileType::Image => img(path.clone()).max_w_full().max_h_full().into_any_element(),
                FileType::Text | FileType::Markdown | FileType::Json | FileType::Code => div()
                    .id("preview-text")
                    .size_full()
                    .overflow_y_scroll()
                    .text_xs()
                    .child(preview::head_lines(&p.content, preview::MAX_PREVIEW_LINES).to_string())
                    .into_any_element(),
                FileType::Binary => v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(
                        Icon::new(IconName::File)
                            .size(px(56.))
                            .text_color(muted),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(t!("main.preview_binary").to_string()),
                    )
                    .into_any_element(),
            },
            Err(e) => div()
                .size_full()
                .items_center()
                .justify_center()
                .p_3()
                .text_xs()
                .text_color(muted)
                .child(e.clone())
                .into_any_element(),
        }
    }
}

impl Render for PreviewPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let meta_block = self.entry.as_ref().map(|(name, path)| {
            let meta = self.meta.clone();
            let hash_block = match &self.hash {
                Some(HashState::Computing) => div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("…")
                    .into_any_element(),
                Some(HashState::Done(Ok(hex))) => h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .font_family("Cascadia Mono")
                            .child(hex.clone()),
                    )
                    .child(
                        Button::new("copy-hash")
                            .small()
                            .outline()
                            .icon(IconName::Copy)
                            .on_click({
                                let hex = hex.clone();
                                move |_, _, cx| {
                                    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(hex.clone()));
                                }
                            }),
                    )
                    .into_any_element(),
                Some(HashState::Done(Err(e))) => div()
                    .text_xs()
                    .text_color(theme.danger)
                    .child(format!("{}: {e}", t!("preview.hash_failed")))
                    .into_any_element(),
                None => Button::new("hash")
                    .small()
                    .outline()
                    .label(t!("preview.hash"))
                    .on_click(cx.listener(|this, _, _, cx| this.compute_hash(cx)))
                    .into_any_element(),
            };
            v_flex()
                .w_full()
                .flex_shrink_0()
                .gap_2()
                .child(div().text_base().font_semibold().child(name.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(path.clone()),
                )
                .child(div().w_full().h(px(1.)).bg(theme.border))
                .children(meta.as_ref().map(|m| match m {
                    Ok(fm) => v_flex()
                        .w_full()
                        .gap_1()
                        .child(Self::meta_row(&t!("preview.size"), preview::human_size(fm.size), &theme))
                        .child(Self::meta_row(&t!("preview.created"), preview::display_time(fm.created_unix), &theme))
                        .child(Self::meta_row(&t!("preview.modified"), preview::display_time(fm.modified_unix), &theme))
                        .into_any_element(),
                    Err(e) => div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child(e.clone())
                        .into_any_element(),
                }))
                .child(hash_block)
        });

        v_flex()
            .id("preview-root")
            .size_full()
            .p_4()
            .gap_3()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(div().flex_1().min_h_0().child(self.render_body(&theme)))
            .children(meta_block)
    }
}
