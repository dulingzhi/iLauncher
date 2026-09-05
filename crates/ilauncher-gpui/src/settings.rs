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
    /// 剪贴板历史容量值名（十进制字符串；不存在 = 用 ilauncher-clipboard 的 DEFAULT_CAPACITY）
    const CLIPBOARD_CAPACITY_VALUE: &str = "ClipboardCapacity";
    /// 容量合法区间（与剪贴板设置页 NumberFieldOptions 一致）
    pub const CLIPBOARD_CAPACITY_MIN: usize = 10;
    pub const CLIPBOARD_CAPACITY_MAX: usize = 1000;

    /// 读取剪贴板历史容量：None = 未设置（调用方回退 DEFAULT_CAPACITY）
    pub fn load_clipboard_capacity() -> Option<usize> {
        load_capacity_from(SETTINGS_KEY, CLIPBOARD_CAPACITY_VALUE)
    }

    fn load_capacity_from(key: &str, name: &str) -> Option<usize> {
        let raw: usize = read_value(key, name)?.parse().ok()?;
        if (CLIPBOARD_CAPACITY_MIN..=CLIPBOARD_CAPACITY_MAX).contains(&raw) {
            Some(raw)
        } else {
            None
        }
    }

    /// 保存剪贴板历史容量（越界自动夹取到合法区间）
    #[cfg(feature = "clipboard")]
    pub fn save_clipboard_capacity(cap: usize) -> anyhow::Result<()> {
        save_capacity_to(SETTINGS_KEY, CLIPBOARD_CAPACITY_VALUE, cap)
    }

    #[cfg(any(feature = "clipboard", test))]
    fn save_capacity_to(key: &str, name: &str, cap: usize) -> anyhow::Result<()> {
        let cap = cap.clamp(CLIPBOARD_CAPACITY_MIN, CLIPBOARD_CAPACITY_MAX);
        write_value(key, name, &cap.to_string())
    }

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

    /// 界面语言值名："zh-CN" / "en" / "system"；不存在或非法 = 跟随系统
    const LANGUAGE_VALUE: &str = "Language";

    /// 读取语言偏好：Some("zh-CN"/"en"/"system")；未设置/非法返回 None（跟随系统）
    pub fn load_language() -> Option<String> {
        let v = read_value(SETTINGS_KEY, LANGUAGE_VALUE)?;
        match v.as_str() {
            "zh-CN" | "en" | "system" => Some(v),
            _ => None,
        }
    }

    /// 保存语言偏好（调用方负责传合法值；i18n::switch_to 会再解析）
    pub fn save_language(lang: &str) -> anyhow::Result<()> {
        write_value(SETTINGS_KEY, LANGUAGE_VALUE, lang)
    }

    /// 皮肤值名：skins.rs SKINS 清单里的 id（"default" / "nord-night"…）；
    /// 不存在或未知 = 默认皮肤（skins::current_skin 再做白名单校验）
    const SKIN_VALUE: &str = "Skin";

    /// 读取皮肤偏好：Some(id)；未设置返回 None
    pub fn load_skin() -> Option<String> {
        read_value(SETTINGS_KEY, SKIN_VALUE).filter(|v| !v.is_empty())
    }

    /// 保存皮肤偏好（调用方负责传合法 id；skins::select_skin 会做白名单校验）
    pub fn save_skin(id: &str) -> anyhow::Result<()> {
        write_value(SETTINGS_KEY, SKIN_VALUE, id)
    }

    /// Windows 系统当前是否深色模式（AppsUseLightTheme=0）。
    /// 读取失败时返回 false（浅色），与 gpui-component 默认一致。
    pub fn system_prefers_dark() -> bool {
        read_value(PERSONALIZE_KEY, APPS_USE_LIGHT)
            .and_then(|v| v.parse::<u32>().ok())
            .map(|v| v == 0)
            .unwrap_or(false)
    }

    /// 禁用插件列表值名：逗号分隔的插件 id；不存在/为空 = 全部启用
    const DISABLED_PLUGINS_VALUE: &str = "DisabledPlugins";

    /// 读取禁用插件列表（解析逗号分隔、去空白、丢弃空项；未设置返回空）
    pub fn load_disabled_plugins() -> Vec<String> {
        load_disabled_from(SETTINGS_KEY, DISABLED_PLUGINS_VALUE)
    }

    fn load_disabled_from(key: &str, name: &str) -> Vec<String> {
        read_value(key, name)
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::autostart::imp::delete_value;

        /// 每个测试独立值名——cargo 并行跑共享同一值名会互相踩
        fn cleanup(name: &str) {
            let _ = delete_value(TEST_KEY, name);
        }

        #[test]
        fn save_then_load_roundtrip() {
            let name = "roundtrip";
            cleanup(name);
            write_value(TEST_KEY, name, "dark").unwrap();
            assert_eq!(load_from(TEST_KEY, name), Some(true));
            write_value(TEST_KEY, name, "light").unwrap();
            assert_eq!(load_from(TEST_KEY, name), Some(false));
            cleanup(name);
        }

        #[test]
        fn missing_pref_returns_none() {
            let name = "missing";
            cleanup(name);
            assert_eq!(load_from(TEST_KEY, name), None);
            cleanup(name);
        }

        #[test]
        fn invalid_value_returns_none() {
            let name = "invalid";
            cleanup(name);
            write_value(TEST_KEY, name, "neon").unwrap();
            assert_eq!(load_from(TEST_KEY, name), None);
            cleanup(name);
        }

        /// 测试走独立（子键, 值名）的 load（生产 load_theme_dark 硬编码 SETTINGS_KEY/THEME_VALUE）
        fn load_from(key: &str, name: &str) -> Option<bool> {
            match read_value(key, name)?.to_lowercase().as_str() {
                "dark" => Some(true),
                "light" => Some(false),
                _ => None,
            }
        }

        #[test]
        fn clipboard_capacity_roundtrip() {
            let name = "cap_roundtrip";
            cleanup(name);
            assert_eq!(load_capacity_from(TEST_KEY, name), None);
            save_capacity_to(TEST_KEY, name, 200).unwrap();
            assert_eq!(load_capacity_from(TEST_KEY, name), Some(200));
            cleanup(name);
        }

        #[test]
        fn clipboard_capacity_invalid_and_out_of_range() {
            let name = "cap_invalid";
            cleanup(name);
            write_value(TEST_KEY, name, "abc").unwrap();
            assert_eq!(load_capacity_from(TEST_KEY, name), None);
            write_value(TEST_KEY, name, "5").unwrap();
            assert_eq!(load_capacity_from(TEST_KEY, name), None); // 低于下限
            write_value(TEST_KEY, name, "99999").unwrap();
            assert_eq!(load_capacity_from(TEST_KEY, name), None); // 高于上限
            cleanup(name);
        }

        #[test]
        fn clipboard_capacity_save_clamps() {
            let name = "cap_clamp";
            cleanup(name);
            save_capacity_to(TEST_KEY, name, 1).unwrap();
            assert_eq!(read_value(TEST_KEY, name).unwrap(), CLIPBOARD_CAPACITY_MIN.to_string());
            save_capacity_to(TEST_KEY, name, 99999).unwrap();
            assert_eq!(read_value(TEST_KEY, name).unwrap(), CLIPBOARD_CAPACITY_MAX.to_string());
            cleanup(name);
        }

        #[test]
        fn system_prefers_dark_returns_bool() {
            // 不断言具体值（随系统设置变化），只验证读取路径不 panic
            let _ = system_prefers_dark();
        }

        #[test]
        fn language_roundtrip_and_invalid() {
            let name = "Language";
            cleanup(name);
            assert_eq!(load_language_from(TEST_KEY, name), None);
            write_value(TEST_KEY, name, "en").unwrap();
            assert_eq!(load_language_from(TEST_KEY, name).as_deref(), Some("en"));
            write_value(TEST_KEY, name, "zh-CN").unwrap();
            assert_eq!(load_language_from(TEST_KEY, name).as_deref(), Some("zh-CN"));
            write_value(TEST_KEY, name, "system").unwrap();
            assert_eq!(load_language_from(TEST_KEY, name).as_deref(), Some("system"));
            // 非法值按未设置处理（跟随系统）
            write_value(TEST_KEY, name, "fr").unwrap();
            assert_eq!(load_language_from(TEST_KEY, name), None);
            cleanup(name);
        }

        /// 测试走独立（子键, 值名）的 load（生产 load_language 硬编码 SETTINGS_KEY/LANGUAGE_VALUE）
        fn load_language_from(key: &str, name: &str) -> Option<String> {
            let v = read_value(key, name)?;
            match v.as_str() {
                "zh-CN" | "en" | "system" => Some(v),
                _ => None,
            }
        }

        #[test]
        fn skin_roundtrip_and_cleanup() {
            let name = "Skin";
            cleanup(name);
            assert_eq!(load_skin_from(TEST_KEY, name), None);
            write_value(TEST_KEY, name, "nord-night").unwrap();
            assert_eq!(load_skin_from(TEST_KEY, name).as_deref(), Some("nord-night"));
            // 空串按未设置处理（skins::current_skin 回退 default）
            write_value(TEST_KEY, name, "").unwrap();
            assert_eq!(load_skin_from(TEST_KEY, name), None);
            cleanup(name);
        }

        /// 测试走独立（子键, 值名）的 load（生产 load_skin 硬编码 SETTINGS_KEY/SKIN_VALUE）
        fn load_skin_from(key: &str, name: &str) -> Option<String> {
            read_value(key, name).filter(|v| !v.is_empty())
        }

        #[test]
        fn disabled_plugins_parse_and_clean() {
            let name = "disabled_plugins";
            cleanup(name);
            assert!(load_disabled_from(TEST_KEY, name).is_empty());
            write_value(TEST_KEY, name, "calculator, web_search ,,ghost").unwrap();
            assert_eq!(
                load_disabled_from(TEST_KEY, name),
                vec![
                    "calculator".to_string(),
                    "web_search".to_string(),
                    "ghost".to_string()
                ]
            );
            // 全空串 → 空列表
            write_value(TEST_KEY, name, " , ,").unwrap();
            assert!(load_disabled_from(TEST_KEY, name).is_empty());
            cleanup(name);
        }
    }
}

#[cfg(windows)]
pub use imp::{
    load_clipboard_capacity, load_disabled_plugins, load_language, load_skin, load_theme_dark,
    save_language, save_skin, save_theme_dark, system_prefers_dark,
};
// 容量写/夹取常量仅在剪贴板设置项存在时使用
#[cfg(all(windows, feature = "clipboard"))]
pub use imp::{save_clipboard_capacity, CLIPBOARD_CAPACITY_MAX, CLIPBOARD_CAPACITY_MIN};
