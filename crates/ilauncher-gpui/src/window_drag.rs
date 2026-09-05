//! 无边框窗口拖动条 + 关闭按钮。
//!
//! 拖动：gpui 原生机制，元素在绘制期经 `window_control_area(WindowControlArea::Drag)`
//! 注册拖动 hitbox，平台层 WM_NCHITTEST 命中后返回 HTCAPTION，由 Windows
//! 原生拖动（含贴边分屏、抖动最大化），且该路径不经过 client 消息，
//! 不会冒泡到输入框/按钮。
//!
//! 关闭按钮刻意放在拖动区的【兄弟】位置而非子元素（gpui-component 的
//! WindowControls 同款结构），避免点击与拖动热区互相干扰；行为等同 Esc：
//! 销毁窗口，唤起时由 WindowGuard 重建。

use gpui_kit::*;
use gpui_kit::component::*;
use gpui_kit::component::theme::Theme;

/// 窗口顶部条：左侧为拖动区（按住拖动窗口），右侧关闭按钮
pub fn drag_strip(label: impl Into<SharedString>, theme: &Theme) -> impl IntoElement {
    h_flex()
        .id("window-drag-strip")
        .h(px(32.))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .px_3()
        .gap_2()
        .border_b_1()
        .border_color(theme.border)
        // 拖动区：占满除关闭按钮外的全部宽度
        .child(
            h_flex()
                .id("window-drag-area")
                .flex_1()
                .h_full()
                .items_center()
                .gap(px(6.))
                .window_control_area(WindowControlArea::Drag)
                // 品牌圆点：全窗口唯一的彩色标记，其余保持安静
                .child(div().size(px(8.)).rounded_full().bg(theme.primary))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.muted_foreground)
                        .child(label.into()),
                ),
        )
        .child(close_button(theme))
}

/// 关闭按钮：点击销毁窗口（与 Esc 等效，主窗口由 WindowGuard 在唤起时重建）
fn close_button(theme: &Theme) -> impl IntoElement {
    div()
        .id("window-close-btn")
        .w(px(40.))
        .h_full()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .text_xs()
        .text_color(theme.muted_foreground)
        .hover(|style| style.bg(theme.muted).text_color(theme.foreground))
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            cx.stop_propagation();
            window.remove_window();
        })
        .child("✕")
}
