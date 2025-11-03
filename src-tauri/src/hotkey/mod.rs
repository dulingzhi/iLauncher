// 全局热键管理

use anyhow::Result;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager, Emitter};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, BringWindowToTop, ShowWindow, SW_SHOW, GetForegroundWindow};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::GetCurrentThreadId;

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    main_hotkey: Option<HotKey>,
}

impl HotkeyManager {
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        Ok(Self {
            manager,
            main_hotkey: None,
        })
    }

    /// 注册主热键 (默认 Alt+Space)
    pub fn register_main_hotkey(&mut self) -> Result<HotKey> {
        // Windows/Linux: Alt+Space
        // macOS: Command+Space
        #[cfg(target_os = "macos")]
        let hotkey = HotKey::new(Some(Modifiers::SUPER), Code::Space);
        
        #[cfg(not(target_os = "macos"))]
        let hotkey = HotKey::new(Some(Modifiers::ALT), Code::Space);

        // 尝试注册，如果已存在则忽略错误
        match self.manager.register(hotkey) {
            Ok(_) => {
                self.main_hotkey = Some(hotkey);
                tracing::info!("Registered main hotkey: {:?}", hotkey);
                Ok(hotkey)
            }
            Err(e) => {
                // 如果热键已经注册，也认为成功
                if e.to_string().contains("already registered") {
                    tracing::warn!("Hotkey already registered, continuing: {:?}", hotkey);
                    self.main_hotkey = Some(hotkey);
                    Ok(hotkey)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// 取消注册热键
    pub fn unregister(&mut self) -> Result<()> {
        if let Some(hotkey) = self.main_hotkey {
            self.manager.unregister(hotkey)?;
            self.main_hotkey = None;
        }
        Ok(())
    }

    /// 监听热键事件
    pub fn start_listener(app_handle: AppHandle) {
        std::thread::spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            tracing::info!("Hotkey listener started");
            
            loop {
                if let Ok(event) = receiver.recv() {
                    // 只处理按键按下事件，忽略释放事件
                    if event.state == global_hotkey::HotKeyState::Pressed {
                        tracing::info!("Hotkey pressed! Event: {:?}", event);
                        
                        // 切换窗口显示状态
                        if let Some(window) = app_handle.get_webview_window("main") {
                            match window.is_visible() {
                                Ok(visible) => {
                                    if visible {
                                        tracing::info!("Hiding window");
                                        let _ = window.hide();
                                    } else {
                                        // 在新线程中处理窗口显示，避免阻塞热键监听
                                        let window_clone = window.clone();
                                        std::thread::spawn(move || {
                                            tracing::info!("Showing window - START");
                                            
                                            // 1. 请求用户注意（这会强制激活窗口）
                                            #[cfg(target_os = "windows")]
                                            {
                                                use tauri::UserAttentionType;
                                                let _ = window_clone.request_user_attention(Some(UserAttentionType::Informational));
                                            }
                                            
                                            // 2. 显示窗口
                                            window_clone.show().unwrap();
                                            tracing::info!("window.show() called");
                                            
                                            // 3. Windows API 激活
                                            #[cfg(target_os = "windows")]
                                            {
                                                if let Ok(hwnd) = window_clone.hwnd() {
                                                    unsafe {
                                                        let hwnd = HWND(hwnd.0 as _);
                                                        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
                                                        let _ = ShowWindow(hwnd, SW_SHOW);
                                                        let _ = BringWindowToTop(hwnd);
                                                        let _ = SetForegroundWindow(hwnd);
                                                    }
                                                }
                                            }
                                            
                                            // 4. 设置焦点
                                            std::thread::sleep(std::time::Duration::from_millis(50));
                                            window_clone.set_focus().unwrap();
                                            tracing::info!("window.set_focus() called");
                                            
                                            // 5. 发送事件到前端
                                            std::thread::sleep(std::time::Duration::from_millis(50));
                                            if let Err(e) = window_clone.emit("focus-input", ()) {
                                                tracing::error!("Failed to emit focus-input: {}", e);
                                            } else {
                                                tracing::info!("focus-input event emitted");
                                            }
                                            
                                            tracing::info!("Showing window - COMPLETE");
                                        });
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Failed to get visibility: {}", e);
                                }
                            }
                        } else {
                            tracing::warn!("Window 'main' not found!");
                        }
                    }
                }
            }
        });
    }
}
