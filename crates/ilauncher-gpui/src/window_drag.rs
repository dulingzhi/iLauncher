//! 无边框窗口拖动条。
//!
//! 用 gpui 原生机制：元素在绘制期经 `window_control_area(WindowControlArea::Drag)`
//! 注册拖动 hitbox，平台层 WM_NCHITTEST 命中后返回 HTCAPTION，由 Windows
//! 原生拖动（含贴边分屏、抖动最大化），且该路径不经过 client 消息，
//! 不会冒泡到输入框/按钮。
//!
//! 曾尝试的 SendMessage(WM_NCLBUTTONDOWN/HTCAPTION) 方案不可行：
//! gpui 的 wndproc 自己消费 WM_NCLBUTTONDOWN（handle_nc_mouse_down_msg），
//! 消息到不了 DefWindowProc 的标题栏拖动分支。

use gpui_kit::*;
use gpui_kit::component::theme::Theme;

/// 窗口顶部拖动条：按住即可拖动窗口（无边框窗口无原生标题栏）
pub fn drag_strip(label: impl Into<SharedString>, theme: &Theme) -> impl IntoElement {
    div()
        .id("window-drag-strip")
        .h(px(28.))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .px_3()
        .window_control_area(WindowControlArea::Drag)
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label.into()),
        )
}
