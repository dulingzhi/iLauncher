//! Windows 剪贴板监听：消息窗口 + WM_CLIPBOARDUPDATE（事件驱动，无轮询）。
//! 收到变更事件后用 arboard 读文本写入 store。薄胶水层，不含可单测逻辑。

#[cfg(target_os = "windows")]
pub mod imp {
    use super::super::store::ClipboardStore;
    use parking_lot::Mutex;
    use std::sync::{Arc, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};
    use windows::core::w;
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::{
        AddClipboardFormatListener, RemoveClipboardFormatListener,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        PostThreadMessageW, RegisterClassW, CW_USEDEFAULT, MSG, WINDOW_EX_STYLE, WINDOW_STYLE,
        WM_CLIPBOARDUPDATE, WM_QUIT, WNDCLASSW,
    };

    /// 退出消息：避开 WM_QUIT 语义（GetMessageW 收到 WM_QUIT 返回 0），用自定义值
    const WM_SHUTDOWN: u32 = WM_QUIT + 0x100;

    static STORE: OnceLock<Arc<Mutex<ClipboardStore>>> = OnceLock::new();
    /// 监听线程 ID（request_shutdown 投递目标）
    static MONITOR_TID: OnceLock<u32> = OnceLock::new();

    fn now_secs() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
    }

    /// 启动监听线程并绑定 store；返回线程句柄。
    /// store 同时存入全局，供窗口过程回调读取（wnd_proc 无自定义参数可用）。
    pub fn start_monitor(store: Arc<Mutex<ClipboardStore>>) -> std::thread::JoinHandle<()> {
        let _ = STORE.set(store);
        std::thread::spawn(move || {
            unsafe {
                let _ = MONITOR_TID.set(GetCurrentThreadId());

                let instance = GetModuleHandleW(None).unwrap_or_default();
                let hinstance = HINSTANCE(instance.0);
                let class_name = w!("iLauncherClipboardMonitor");
                let wc = WNDCLASSW {
                    hInstance: hinstance,
                    lpszClassName: class_name,
                    lpfnWndProc: Some(wnd_proc),
                    ..Default::default()
                };
                if RegisterClassW(&wc) == 0 {
                    eprintln!(
                        "⚠️ 剪贴板监听: RegisterClass 失败 {:#X}",
                        windows::Win32::Foundation::GetLastError().0
                    );
                    return;
                }

                // 消息专用窗口：不可见，只收剪贴板/线程消息
                let hwnd = match CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    class_name,
                    w!("iLauncherClipboardMonitor"),
                    WINDOW_STYLE(0),
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    None,
                    None,
                    hinstance,
                    None,
                ) {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("⚠️ 剪贴板监听: 创建消息窗口失败 {e}");
                        return;
                    }
                };

                if AddClipboardFormatListener(hwnd).is_err() {
                    eprintln!("⚠️ 剪贴板监听: AddClipboardFormatListener 失败");
                    let _ = DestroyWindow(hwnd);
                    return;
                }
                println!("✓ 剪贴板监听已启动（事件驱动）");

                let mut msg = MSG::default();
                loop {
                    // 阻塞取消息；shutdown 时 GetMessageW 返回 0 或收到 WM_SHUTDOWN
                    if GetMessageW(&mut msg, None, 0, 0).0 == 0 || msg.message == WM_SHUTDOWN {
                        break;
                    }
                    DispatchMessageW(&msg);
                }

                let _ = RemoveClipboardFormatListener(hwnd);
                let _ = DestroyWindow(hwnd);
            }
        })
    }

    extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_CLIPBOARDUPDATE {
            read_clipboard_text();
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// 读剪贴板文本写入 store；读不到（剪贴板被占用/非文本）静默跳过
    fn read_clipboard_text() {
        let Some(store) = STORE.get() else { return };
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if let Ok(text) = cb.get_text() {
                store.lock().add_text(&text, now_secs());
            }
        }
    }

    /// 请求监听线程退出（ WM_SHUTDOWN 投递到监听线程的 GetMessageW 队列）
    pub fn request_shutdown() {
        if let Some(tid) = MONITOR_TID.get() {
            unsafe {
                let _ = PostThreadMessageW(*tid, WM_SHUTDOWN, WPARAM(0), LPARAM(0));
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::{request_shutdown, start_monitor};
