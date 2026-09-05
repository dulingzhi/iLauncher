//! 无边框窗口拖动支持。
//! gpui 的 `start_window_move` 只有 Linux/macOS 实现（Windows 平台是空操作），
//! Windows 上用经典技巧：向窗口发送 `WM_NCLBUTTONDOWN/HTCAPTION`，
//! 让系统按"点在标题栏"处理，进入原生拖动流程（含贴边分屏）。
//! 拖动条是显式的顶部横条——整窗监听会冒泡到输入框/按钮，破坏点击。

use gpui_kit::*;

/// 进入系统窗口拖动流程（Windows）；其他平台暂为无操作
#[cfg(target_os = "windows")]
pub fn begin(window: &mut Window) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, SendMessageW, WM_NCLBUTTONDOWN};

    if let Ok(handle) = window.window_handle()
        && let RawWindowHandle::Win32(h) = handle.as_raw()
    {
        let hwnd = HWND(h.hwnd.get() as *mut core::ffi::c_void);
        unsafe {
            SendMessageW(hwnd, WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(0));
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn begin(_window: &mut Window) {}

/// 窗口顶部拖动条：左键按住即可拖动窗口（无边框窗口无原生标题栏）
pub fn drag_strip(label: impl Into<SharedString>, theme: &gpui_kit::component::theme::Theme) -> impl IntoElement {
    div()
        .id("window-drag-strip")
        .h(px(28.))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .px_3()
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label.into()),
        )
        .on_mouse_down(MouseButton::Left, |_, window, _| begin(window))
}
