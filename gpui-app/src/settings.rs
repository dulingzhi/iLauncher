//! 应用设置持久化：HKCU\Software\iLauncher（REG_SZ 键值，与自启模块共用注册表原语）
//! 不依赖 gpui，可单测。

#[cfg(windows)]
mod imp {
    use crate::autostart::imp::{read_value, write_value};

    /// 设置子键（相对于 HKCU）
    const SETTINGS_KEY: &str = r"Software\iLauncher";
    /// 主题偏好值名："dark" / "light"；不存在 = 跟随系统
    const THEME_VALUE: &str = "ThemeMode";
    /// Windows 系统个性化设置（AppsUseLightTheme: 0 = 深色）
    const PERSONALIZE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    const APPS_USE_LIGHT: &str = "AppsUseLightTheme";
    /// 测试专用子键
    #[cfg(test)]
    const TEST_KEY: &str = r"Software\iLauncher_gpui_settings_test";

    /// 读取用户主题偏好：Some(true)=深色 / Some(false)=浅色 / None=未设置（跟随系统）
    pub fn load_theme_dark() -> Option<bool> {
        match read_value(SETTINGS_KEY, THEME_VALUE)?.to_lowercase().as_str() {
            "dark" => Some(true),
            "light" => Some(false),
            _ => None,
        }
    }

    /// 保存用户主题偏好
    pub fn save_theme_dark(dark: bool) -> anyhow::Result<()> {
        write_value(SETTINGS_KEY, THEME_VALUE, if dark { "dark" } else { "light" })
    }

    /// Windows 系统当前是否深色模式（AppsUseLightTheme=0）。
    /// 读取失败时返回 false（浅色），与 gpui-component 默认一致。
    pub fn system_prefers_dark() -> bool {
        read_value(PERSONALIZE_KEY, APPS_USE_LIGHT)
            .and_then(|v| v.parse::<u32>().ok())
            .map(|v| v == 0)
            .unwrap_or(false)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::autostart::imp::delete_value;

        fn cleanup() {
            let _ = delete_value(TEST_KEY, THEME_VALUE);
        }

        #[test]
        fn save_then_load_roundtrip() {
            cleanup();
            write_value(TEST_KEY, THEME_VALUE, "dark").unwrap();
            assert_eq!(load_theme_dark_from(TEST_KEY), Some(true));
            write_value(TEST_KEY, THEME_VALUE, "light").unwrap();
            assert_eq!(load_theme_dark_from(TEST_KEY), Some(false));
            cleanup();
        }

        #[test]
        fn missing_pref_returns_none() {
            cleanup();
            assert_eq!(load_theme_dark_from(TEST_KEY), None);
            cleanup();
        }

        #[test]
        fn invalid_value_returns_none() {
            cleanup();
            write_value(TEST_KEY, THEME_VALUE, "neon").unwrap();
            assert_eq!(load_theme_dark_from(TEST_KEY), None);
            cleanup();
        }

        /// 测试走独立子键版的 load（生产 load_theme_dark 硬编码 SETTINGS_KEY）
        fn load_theme_dark_from(key: &str) -> Option<bool> {
            match read_value(key, THEME_VALUE)?.to_lowercase().as_str() {
                "dark" => Some(true),
                "light" => Some(false),
                _ => None,
            }
        }

        #[test]
        fn system_prefers_dark_returns_bool() {
            // 不断言具体值（随系统设置变化），只验证读取路径不 panic
            let _ = system_prefers_dark();
        }
    }
}

#[cfg(windows)]
pub use imp::{load_theme_dark, save_theme_dark, system_prefers_dark};
