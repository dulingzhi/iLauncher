// 窗口管理插件
// 功能：切换窗口、最小化全部、显示桌面、窗口置顶等

use crate::plugin::Plugin;
use crate::core::types::{PluginMetadata, QueryContext, QueryResult, Action, WoxImage};
use anyhow::Result;
use sysinfo::System;

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM},
    UI::WindowsAndMessaging::*,
    UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP},
};

pub struct WindowManagerPlugin {
    metadata: PluginMetadata,
}

#[derive(Debug, Clone)]
struct WindowInfo {
    hwnd: isize,
    title: String,
    process_name: String,
    #[allow(dead_code)]
    is_visible: bool,
}

impl Default for WindowManagerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowManagerPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "window_manager".to_string(),
                name: "窗口管理".to_string(),
                description: "管理窗口：切换、最小化、显示桌面、置顶等".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                trigger_keywords: vec![
                    "win".to_string(),
                    "window".to_string(),
                    "窗口".to_string(),
                    "chuangkou".to_string(),
                ],
                icon: WoxImage::emoji("🪟".to_string()),
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: crate::core::types::PluginType::Native,
            },
        }
    }

    /// 获取所有可见窗口列表（Windows）
    #[cfg(target_os = "windows")]
    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        use std::sync::Mutex;

        let windows: std::sync::Arc<Mutex<Vec<WindowInfo>>> = std::sync::Arc::new(Mutex::new(Vec::new()));
        let windows_clone = windows.clone();

        unsafe {
            EnumWindows(
                Some(enum_windows_callback),
                LPARAM(&*windows_clone as *const _ as isize),
            )?;
        }

        let mut result = windows.lock().unwrap().clone();

        // 获取进程名称
        let mut system = System::new_all();
        system.refresh_all();

        for win in &mut result {
            if let Some(proc) = system
                .processes()
                .values()
                .find(|p| p.pid().as_u32() as isize == win.hwnd)
            {
                win.process_name = proc.name().to_string_lossy().to_string();
            }
        }

        // 按标题排序
        result.sort_by(|a, b| a.title.cmp(&b.title));

        Ok(result)
    }

    /// macOS/Linux 窗口列表（占位符）
    #[cfg(not(target_os = "windows"))]
    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        // TODO: 实现 macOS/Linux 窗口枚举
        Ok(Vec::new())
    }

    /// 执行窗口操作
    #[cfg(target_os = "windows")]
    async fn execute_window_action(&self, action: &str, hwnd: Option<isize>) -> Result<()> {
        match action {
            "minimize_all" => self.minimize_all_windows()?,
            "show_desktop" => self.show_desktop()?,
            "switch" => {
                if let Some(h) = hwnd {
                    self.switch_to_window(h)?;
                }
            }
            "minimize" => {
                if let Some(h) = hwnd {
                    self.minimize_window(h)?;
                }
            }
            "maximize" => {
                if let Some(h) = hwnd {
                    self.maximize_window(h)?;
                }
            }
            "close" => {
                if let Some(h) = hwnd {
                    self.close_window(h)?;
                }
            }
            "always_on_top" => {
                if let Some(h) = hwnd {
                    self.set_always_on_top(h)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn minimize_all_windows(&self) -> Result<()> {
        unsafe {
            // 模拟 Win+D（显示桌面）
            keybd_event(0x5B, 0, Default::default(), 0); // Win key down
            keybd_event(0x44, 0, Default::default(), 0); // D key down
            keybd_event(0x44, 0, KEYEVENTF_KEYUP, 0); // D key up
            keybd_event(0x5B, 0, KEYEVENTF_KEYUP, 0); // Win key up
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn show_desktop(&self) -> Result<()> {
        self.minimize_all_windows()
    }

    #[cfg(target_os = "windows")]
    fn switch_to_window(&self, hwnd: isize) -> Result<()> {
        unsafe {
            let hwnd = HWND(hwnd as *mut _);
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
            let _ = BringWindowToTop(hwnd);
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn minimize_window(&self, hwnd: isize) -> Result<()> {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), SW_MINIMIZE);
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn maximize_window(&self, hwnd: isize) -> Result<()> {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), SW_MAXIMIZE);
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn close_window(&self, hwnd: isize) -> Result<()> {
        unsafe {
            PostMessageW(HWND(hwnd as *mut _), WM_CLOSE, windows::Win32::Foundation::WPARAM(0), LPARAM(0))?;
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn set_always_on_top(&self, hwnd: isize) -> Result<()> {
        unsafe {
            let hwnd = HWND(hwnd as *mut _);
            // 切换置顶状态
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            let is_topmost = (ex_style & WS_EX_TOPMOST.0 as i32) != 0;

            let insert_after = if is_topmost {
                HWND_NOTOPMOST
            } else {
                HWND_TOPMOST
            };

            SetWindowPos(
                hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )?;
        }
        Ok(())
    }

    /// 非 Windows 系统的占位实现
    #[cfg(not(target_os = "windows"))]
    async fn execute_window_action(&self, _action: &str, _hwnd: Option<isize>) -> Result<()> {
        // TODO: 实现 macOS/Linux 窗口操作
        Ok(())
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
    };

    let windows = &*(lparam.0 as *const std::sync::Mutex<Vec<WindowInfo>>);

    // 只枚举可见窗口
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    // 获取窗口标题
    let length = GetWindowTextLengthW(hwnd);
    if length == 0 {
        return BOOL(1);
    }

    let mut buffer = vec![0u16; (length + 1) as usize];
    let copied = GetWindowTextW(hwnd, &mut buffer);
    if copied == 0 {
        return BOOL(1);
    }

    let title = String::from_utf16_lossy(&buffer[..copied as usize]);

    // 过滤掉系统窗口和无标题窗口
    if title.is_empty() || title.starts_with("MSCTFIME") || title == "Default IME" {
        return BOOL(1);
    }

    windows.lock().unwrap().push(WindowInfo {
        hwnd: hwnd.0 as isize,
        title,
        process_name: String::new(),
        is_visible: true,
    });

    BOOL(1)
}

#[async_trait::async_trait]
impl Plugin for WindowManagerPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query_lower = ctx.search.to_lowercase();
        let mut results = Vec::new();

        // 系统命令
        let system_commands = vec![
            ("minimize_all", "最小化所有窗口", "显示桌面", "🪟"),
            ("show_desktop", "显示桌面", "最小化所有窗口", "🖥️"),
        ];

        for (id, title, subtitle, emoji) in system_commands {
            if query_lower.contains("min")
                || query_lower.contains("desktop")
                || query_lower.contains("桌面")
                || query_lower.contains("最小化")
                || query_lower.contains("zuixiaohua")
                || query_lower.contains("zhuomian")
            {
                results.push(
                    QueryResult::new(title.to_string())
                        .with_subtitle(subtitle.to_string())
                        .with_icon(WoxImage::emoji(emoji.to_string()))
                        .with_action(Action::new(id.to_string()).default())
                );
            }
        }

        // 窗口列表搜索
        if query_lower.contains("switch")
            || query_lower.contains("窗口")
            || query_lower.contains("win")
            || query_lower.contains("chuangkou")
            || ctx.search.len() > 2
        {
            let windows = self.list_windows()?;
            let search_query = ctx.search.to_lowercase();

            for win in windows {
                let title_lower = win.title.to_lowercase();
                let process_lower = win.process_name.to_lowercase();

                // 模糊匹配窗口标题或进程名
                if title_lower.contains(&search_query) || process_lower.contains(&search_query) {
                    results.push(
                        QueryResult::new(win.title.clone())
                            .with_subtitle(format!("进程: {} | 切换到此窗口", win.process_name))
                            .with_icon(WoxImage::emoji("🪟".to_string()))
                            .with_action(Action::new("switch".to_string()).default())
                            .with_action(Action::new("minimize".to_string()))
                            .with_action(Action::new("maximize".to_string()))
                            .with_action(Action::new("close".to_string()))
                            .with_action(Action::new("always_on_top".to_string())),
                    );
                    
                    // 保存 hwnd 到 result.id 中
                    if let Some(last) = results.last_mut() {
                        last.id = format!("window_{}", win.hwnd);
                    }
                }
            }
        }

        Ok(results)
    }

    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        let hwnd = if result_id.starts_with("window_") {
            // 从 result_id 中提取 hwnd
            result_id
                .strip_prefix("window_")
                .and_then(|s| s.parse::<isize>().ok())
        } else {
            None
        };

        self.execute_window_action(action_id, hwnd).await?;
        Ok(())
    }
}
