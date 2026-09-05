//! i18n：语言解析与切换（rust-i18n 接入在 crate 根 main.rs，
//! `i18n!` 宏生成的 `_rust_i18n_t` 必须落在 crate 根，t! 展开按此路径解析）。
//!
//! - 文案编译期嵌入（locales/*.yml），`t!("key")` 读全局 locale，
//!   缺失键回退 fallback（zh-CN）
//! - 语言偏好持久化在注册表（settings.rs：Language = zh-CN / en / system；
//!   未设置或非法 = 跟随系统）
//! - 启动时 main() 调 apply() 一次；设置页切换语言即时 set_locale 并刷新窗口。
//!   托盘菜单文本在启动时定型，切换语言后托盘仍显示旧语言（重启更新）。

pub use rust_i18n::t;

/// 注册表值：跟随系统
pub const FOLLOW_SYSTEM: &str = "system";
/// 简体中文
pub const ZH_CN: &str = "zh-CN";
/// 英语
pub const EN: &str = "en";

/// 解析生效语言（纯函数）：偏好未设置/非法 → 系统检测
pub fn resolve(pref: Option<&str>) -> &'static str {
    match pref {
        Some(ZH_CN) => ZH_CN,
        Some(EN) => EN,
        _ => detect_system(),
    }
}

/// 系统 UI 语言检测：主语言是中文 → zh-CN，否则 en
#[cfg(windows)]
pub fn detect_system() -> &'static str {
    use windows::Win32::Globalization::GetUserDefaultUILanguage;
    // LANGID 低 10 位是主语言；0x04 = Chinese（含 zh-CN/zh-TW 等全部中文）
    let primary = unsafe { GetUserDefaultUILanguage() } & 0x3FF;
    if primary == 0x04 { ZH_CN } else { EN }
}

#[cfg(not(windows))]
pub fn detect_system() -> &'static str {
    EN
}

/// 读注册表偏好并应用全局 locale（启动时调用一次）
pub fn apply() {
    rust_i18n::set_locale(resolve(crate::settings::load_language().as_deref()));
}

/// 设置页切换语言：持久化 → 应用 → 由调用方刷新窗口
pub fn switch_to(lang: &str) -> anyhow::Result<()> {
    crate::settings::save_language(lang)?;
    rust_i18n::set_locale(resolve(Some(lang)));
    Ok(())
}

/// 测试固定中文环境：rust-i18n 运行时初始 locale 硬编码为 "en"，
/// 断言中文文案的测试开头调用本函数（set_locale 是全局态，测试都设同一值，并行安全）
#[cfg(test)]
pub fn test_use_zh() {
    rust_i18n::set_locale(ZH_CN);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_valid_pref() {
        assert_eq!(resolve(Some("zh-CN")), ZH_CN);
        assert_eq!(resolve(Some("en")), EN);
    }

    #[test]
    fn resolve_falls_back_on_missing_or_invalid() {
        assert_eq!(resolve(None), detect_system());
        assert_eq!(resolve(Some("")), detect_system());
        assert_eq!(resolve(Some("fr")), detect_system());
        assert_eq!(resolve(Some("SYSTEM")), detect_system()); // 大小写敏感，防呆
    }
}
