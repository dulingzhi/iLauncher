// 全局热键管理

use anyhow::Result;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use tauri::{AppHandle, Manager, Emitter};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, BringWindowToTop, ShowWindow, SW_SHOW, 
    SendMessageW, WM_LBUTTONDOWN, WM_LBUTTONUP, FindWindowExW
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};

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

    /// 从字符串注册热键
    pub fn register_from_string(&mut self, hotkey_str: &str) -> Result<HotKey> {
        let hotkey = Self::parse_hotkey(hotkey_str)?;
        
        match self.manager.register(hotkey) {
            Ok(_) => {
                self.main_hotkey = Some(hotkey);
                tracing::info!("Registered hotkey from string '{}': {:?}", hotkey_str, hotkey);
                Ok(hotkey)
            }
            Err(e) => {
                if e.to_string().contains("already registered") {
                    tracing::warn!("Hotkey already registered: {:?}", hotkey);
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

    /// 解析热键字符串 (例如: "Alt+Space", "Ctrl+Shift+A")
    pub fn parse_hotkey(hotkey_str: &str) -> Result<HotKey> {
        let parts: Vec<&str> = hotkey_str.split('+').map(|s| s.trim()).collect();
        
        if parts.is_empty() {
            anyhow::bail!("Empty hotkey string");
        }
        
        let mut modifiers = Modifiers::empty();
        let mut key_code: Option<Code> = None;
        
        for part in parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
                "alt" => modifiers |= Modifiers::ALT,
                "shift" => modifiers |= Modifiers::SHIFT,
                "super" | "win" | "cmd" | "command" => modifiers |= Modifiers::SUPER,
                // 字母键
                "a" => key_code = Some(Code::KeyA),
                "b" => key_code = Some(Code::KeyB),
                "c" => key_code = Some(Code::KeyC),
                "d" => key_code = Some(Code::KeyD),
                "e" => key_code = Some(Code::KeyE),
                "f" => key_code = Some(Code::KeyF),
                "g" => key_code = Some(Code::KeyG),
                "h" => key_code = Some(Code::KeyH),
                "i" => key_code = Some(Code::KeyI),
                "j" => key_code = Some(Code::KeyJ),
                "k" => key_code = Some(Code::KeyK),
                "l" => key_code = Some(Code::KeyL),
                "m" => key_code = Some(Code::KeyM),
                "n" => key_code = Some(Code::KeyN),
                "o" => key_code = Some(Code::KeyO),
                "p" => key_code = Some(Code::KeyP),
                "q" => key_code = Some(Code::KeyQ),
                "r" => key_code = Some(Code::KeyR),
                "s" => key_code = Some(Code::KeyS),
                "t" => key_code = Some(Code::KeyT),
                "u" => key_code = Some(Code::KeyU),
                "v" => key_code = Some(Code::KeyV),
                "w" => key_code = Some(Code::KeyW),
                "x" => key_code = Some(Code::KeyX),
                "y" => key_code = Some(Code::KeyY),
                "z" => key_code = Some(Code::KeyZ),
                // 特殊键
                "space" => key_code = Some(Code::Space),
                "enter" | "return" => key_code = Some(Code::Enter),
                "tab" => key_code = Some(Code::Tab),
                "escape" | "esc" => key_code = Some(Code::Escape),
                "backspace" => key_code = Some(Code::Backspace),
                // 数字键
                "0" => key_code = Some(Code::Digit0),
                "1" => key_code = Some(Code::Digit1),
                "2" => key_code = Some(Code::Digit2),
                "3" => key_code = Some(Code::Digit3),
                "4" => key_code = Some(Code::Digit4),
                "5" => key_code = Some(Code::Digit5),
                "6" => key_code = Some(Code::Digit6),
                "7" => key_code = Some(Code::Digit7),
                "8" => key_code = Some(Code::Digit8),
                "9" => key_code = Some(Code::Digit9),
                // F键
                "f1" => key_code = Some(Code::F1),
                "f2" => key_code = Some(Code::F2),
                "f3" => key_code = Some(Code::F3),
                "f4" => key_code = Some(Code::F4),
                "f5" => key_code = Some(Code::F5),
                "f6" => key_code = Some(Code::F6),
                "f7" => key_code = Some(Code::F7),
                "f8" => key_code = Some(Code::F8),
                "f9" => key_code = Some(Code::F9),
                "f10" => key_code = Some(Code::F10),
                "f11" => key_code = Some(Code::F11),
                "f12" => key_code = Some(Code::F12),
                _ => anyhow::bail!("Unknown key: {}", part),
            }
        }
        
        let code = key_code.ok_or_else(|| anyhow::anyhow!("No key code found in hotkey string"))?;
        
        let modifier_opt = if modifiers.is_empty() {
            None
        } else {
            Some(modifiers)
        };
        
        Ok(HotKey::new(modifier_opt, code))
    }

    /// 更新热键
    pub fn update_hotkey(&mut self, hotkey_str: &str) -> Result<()> {
        // 先取消注册旧热键
        self.unregister()?;
        
        // 解析新热键
        let hotkey = Self::parse_hotkey(hotkey_str)?;
        
        // 注册新热键
        self.manager.register(hotkey)?;
        self.main_hotkey = Some(hotkey);
        
        tracing::info!("Updated hotkey to: {:?}", hotkey);
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
                                            // 设置置顶
                                            let _ = window_clone.set_always_on_top(true);
                                            
                                            // 居中窗口
                                            let _ = window_clone.center();
                                            
                                            // Windows: 请求用户注意（强制激活窗口）
                                            #[cfg(target_os = "windows")]
                                            {
                                                use tauri::UserAttentionType;
                                                let _ = window_clone.request_user_attention(Some(UserAttentionType::Informational));
                                            }
                                            
                                            // 显示窗口
                                            let _ = window_clone.show();
                                            
                                            // Windows API 激活
                                            #[cfg(target_os = "windows")]
                                            {
                                                if let Ok(hwnd) = window_clone.hwnd() {
                                                    unsafe {
                                                        let hwnd = HWND(hwnd.0 as _);
                                                        
                                                        // 释放 Alt 键
                                                        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
                                                        
                                                        // 激活窗口
                                                        ShowWindow(hwnd, SW_SHOW);
                                                        BringWindowToTop(hwnd);
                                                        SetForegroundWindow(hwnd);
                                                        
                                                        std::thread::sleep(std::time::Duration::from_millis(10));
                                                    }
                                                }
                                            }
                                            
                                            // 设置焦点
                                            std::thread::sleep(std::time::Duration::from_millis(50));
                                            let _ = window_clone.set_focus();
                                            
                                            // 等待窗口完全激活
                                            std::thread::sleep(std::time::Duration::from_millis(150));
                                            
                                            // Windows: 发送点击消息到 WebView 子窗口激活输入
                                            #[cfg(target_os = "windows")]
                                            {
                                                if let Ok(hwnd) = window_clone.hwnd() {
                                                    use windows::Win32::Foundation::{LPARAM, WPARAM};
                                                    
                                                    unsafe {
                                                        let hwnd = HWND(hwnd.0 as _);
                                                        
                                                        // 查找 WebView 子窗口 (3层嵌套)
                                                        let target_hwnd = match FindWindowExW(hwnd, None, None, None) {
                                                            Ok(child1) if !child1.is_invalid() => {
                                                                match FindWindowExW(child1, None, None, None) {
                                                                    Ok(child2) if !child2.is_invalid() => {
                                                                        match FindWindowExW(child2, None, None, None) {
                                                                            Ok(child3) if !child3.is_invalid() => child3,
                                                                            _ => child2
                                                                        }
                                                                    }
                                                                    _ => child1
                                                                }
                                                            }
                                                            _ => hwnd
                                                        };
                                                        
                                                        // 输入框坐标
                                                        let x = 350i32;
                                                        let y = 50i32;
                                                        let lparam = LPARAM(((y as u32) << 16 | (x as u32 & 0xFFFF)) as isize);
                                                        let wparam = WPARAM(0);
                                                        
                                                        // 发送点击消息
                                                        SendMessageW(target_hwnd, WM_LBUTTONDOWN, wparam, lparam);
                                                        std::thread::sleep(std::time::Duration::from_millis(10));
                                                        SendMessageW(target_hwnd, WM_LBUTTONUP, wparam, lparam);
                                                    }
                                                }
                                            }
                                            
                                            // 发送事件到前端
                                            std::thread::sleep(std::time::Duration::from_millis(50));
                                            let _ = window_clone.emit("focus-input", ());
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
