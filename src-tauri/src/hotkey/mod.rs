// 全局热键管理

use anyhow::Result;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

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

        self.manager.register(hotkey)?;
        self.main_hotkey = Some(hotkey);
        
        tracing::info!("Registered main hotkey: {:?}", hotkey);
        Ok(hotkey)
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
            loop {
                if let Ok(event) = receiver.recv() {
                    tracing::debug!("Hotkey event: {:?}", event);
                    
                    // 切换窗口显示状态
                    if let Some(window) = app_handle.get_webview_window("main") {
                        if let Ok(visible) = window.is_visible() {
                            if visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                }
            }
        });
    }
}
