//! 皮肤（主题）系统：在 gpui-component 默认深/浅主题之上提供多套内置配色皮肤。
//!
//! 机制：每套皮肤是一份 gpui-component `ThemeSet` JSON（内嵌常量），
//! 应用时把 `Theme.light_theme` / `Theme.dark_theme` 双槽都指向该配置并
//! `Theme::change(皮肤自带 mode)`；未覆盖的颜色键回退到 gpui-component
//! 内置明暗色板（见 gpui-component schema.rs `apply_config`）。
//! "default" 皮肤 = 恢复注册表默认主题 + 跟随已持久化的深/浅偏好。
//!
//! 皮肤的 mode 是固定的（如 Nord 只有深色）；此时深/浅开关 = 回退默认皮肤，
//! 由 `set_dark_mode` 统一处理。

use gpui_kit::component::theme::{Theme, ThemeConfig, ThemeMode, ThemeRegistry, ThemeSet};
use gpui_kit::App;
use std::rc::Rc;
/// 内置皮肤清单：(id, i18n 名称键)。id 即注册表持久化值。
pub const SKINS: &[(&str, &str)] = &[
    ("default", "skin.default"),
    ("nord-night", "skin.nord_night"),
    ("cyber-neon", "skin.cyber_neon"),
    ("catppuccin", "skin.catppuccin"),
    ("warm-paper", "skin.warm_paper"),
    ("mint-mist", "skin.mint_mist"),
];

/// 默认皮肤 id：gpui-component 内置明暗主题，深/浅可切换
pub const DEFAULT_SKIN: &str = "default";

/// 当前生效皮肤 id（未设置/注册表残留未知值 → default）
pub fn current_skin() -> String {
    match crate::settings::load_skin() {
        Some(id) if SKINS.iter().any(|(sid, _)| *sid == id) => id,
        _ => DEFAULT_SKIN.to_string(),
    }
}

/// 启动时应用持久化的皮肤（main 的主题初始化块调用）
pub fn apply_saved(cx: &mut App) {
    let id = current_skin();
    apply(&id, cx);
    println!(
        "✓ 主题初始化 → {}",
        if id == DEFAULT_SKIN {
            let dark = Theme::global(cx).is_dark();
            if dark { "深色（默认）" } else { "浅色（默认）" }
        } else {
            "自定义皮肤"
        }
    );
}

/// 切换皮肤：持久化 → 应用 → 同步设置页 model
pub fn select_skin(id: &str, cx: &mut App) {
    let id = if SKINS.iter().any(|(sid, _)| *sid == id) {
        id
    } else {
        DEFAULT_SKIN
    };
    if let Err(e) = crate::settings::save_skin(id) {
        eprintln!("⚠️ 保存皮肤偏好失败: {e:#}");
    }
    apply(id, cx);
    #[cfg(windows)]
    crate::settings_ui::sync_skin_model(cx, id);
}

/// 深/浅开关（托盘菜单与设置页共用）。
/// 自定义皮肤的明暗是固定的：开关爱咋切 = 回退默认皮肤后按开关应用。
pub fn set_dark_mode(dark: bool, cx: &mut App) {
    if let Err(e) = crate::settings::save_theme_dark(dark) {
        eprintln!("⚠️ 保存主题偏好失败: {e:#}");
    }
    if current_skin() != DEFAULT_SKIN {
        let _ = crate::settings::save_skin(DEFAULT_SKIN);
        #[cfg(windows)]
        crate::settings_ui::sync_skin_model(cx, DEFAULT_SKIN);
    }
    apply(DEFAULT_SKIN, cx);
}

/// 应用指定皮肤（不写持久化；持久化由 select_skin/set_dark_mode 负责）
fn apply(id: &str, cx: &mut App) {
    if id == DEFAULT_SKIN {
        let (light, dark) = {
            let reg = ThemeRegistry::global(cx);
            (reg.default_light_theme().clone(), reg.default_dark_theme().clone())
        };
        let dark_mode =
            crate::settings::load_theme_dark().unwrap_or_else(crate::settings::system_prefers_dark);
        Theme::global_mut(cx).light_theme = light;
        Theme::global_mut(cx).dark_theme = dark;
        Theme::change(
            if dark_mode { ThemeMode::Dark } else { ThemeMode::Light },
            None,
            cx,
        );
    } else if let Some(config) = parse_skin(id) {
        // 注册进 registry（观察回调/排查可用），并把明暗双槽都指向该皮肤，
        // 这样 Theme::change 无论按哪个 mode 解析都落到同一配置
        if let Some(json) = skin_json(id) {
            let _ = ThemeRegistry::global_mut(cx).load_themes_from_str(json);
        }
        let mode = config.mode;
        let config = Rc::new(config);
        Theme::global_mut(cx).light_theme = config.clone();
        Theme::global_mut(cx).dark_theme = config;
        Theme::change(mode, None, cx);
    } else {
        eprintln!("⚠️ 未知皮肤 {id}，保持当前主题");
        return;
    }
    // Theme::change 会重置 font_family，CJK 字体需重刷
    #[cfg(target_os = "windows")]
    crate::apply_cjk_font(cx);
    // 手动刷新全部已开窗口让背景色等一次性生效
    cx.refresh_windows();
}

/// 解析皮肤 JSON 为 ThemeConfig（单个皮肤的 ThemeSet 取第一个主题）
fn parse_skin(id: &str) -> Option<ThemeConfig> {
    let set: ThemeSet = serde_json::from_str(skin_json(id)?).ok()?;
    set.themes.into_iter().next()
}

/// 皮肤 JSON（gpui-component ThemeSet 格式）
fn skin_json(id: &str) -> Option<&'static str> {
    match id {
        "nord-night" => Some(NORD_NIGHT_JSON),
        "cyber-neon" => Some(CYBER_NEON_JSON),
        "catppuccin" => Some(CATPPUCCIN_JSON),
        "warm-paper" => Some(WARM_PAPER_JSON),
        "mint-mist" => Some(MINT_MIST_JSON),
        _ => None,
    }
}

/// 夜幕 Nord：冷灰蓝极地夜色，青蓝点缀（深色）
const NORD_NIGHT_JSON: &str = r##"{
  "name": "iLauncher Skins",
  "themes": [{
    "name": "iLauncher Nord Night",
    "mode": "dark",
    "radius": 8,
    "colors": {
      "background": "#2E3440",
      "foreground": "#D8DEE9",
      "border": "#434C5E",
      "caret": "#88C0D0",
      "ring": "#88C0D0",
      "selection.background": "#88C0D04D",
      "muted.background": "#3B4252",
      "muted.foreground": "#7B88A3",
      "accent.background": "#434C5E",
      "accent.foreground": "#88C0D0",
      "accordion.background": "#3B4252",
      "popover.background": "#3B4252",
      "popover.foreground": "#ECEFF4",
      "sidebar.background": "#2B303B",
      "sidebar.foreground": "#D8DEE9",
      "sidebar.border": "#3B4252",
      "sidebar.accent.background": "#434C5E",
      "sidebar.accent.foreground": "#88C0D0",
      "group_box.background": "#363C4A",
      "group_box.foreground": "#ECEFF4",
      "primary.background": "#88C0D0",
      "primary.hover.background": "#8FCDDE",
      "primary.active.background": "#7AB0BF",
      "primary.foreground": "#2E3440",
      "secondary.background": "#4C566A",
      "secondary.hover.background": "#55617A",
      "secondary.active.background": "#434C5E",
      "secondary.foreground": "#ECEFF4",
      "danger.background": "#BF616A",
      "danger.hover.background": "#C9707A",
      "danger.active.background": "#B0545E",
      "danger.foreground": "#2E3440",
      "info.background": "#81A1C1",
      "info.hover.background": "#8EACC9",
      "info.active.background": "#7493B3",
      "info.foreground": "#2E3440",
      "success.background": "#A3BE8C",
      "success.hover.background": "#AFCB98",
      "success.active.background": "#97B17F",
      "success.foreground": "#2E3440",
      "warning.background": "#EBCB8B",
      "warning.hover.background": "#EFD49B",
      "warning.active.background": "#E3C27E",
      "warning.foreground": "#2E3440",
      "list.background": "#2E3440",
      "list.active.background": "#88C0D01F",
      "list.active.border": "#88C0D0",
      "list.hover.background": "#3B4252",
      "list.even.background": "#313747",
      "input.border": "#4C566A",
      "link.foreground": "#88C0D0",
      "link.hover.foreground": "#8FCDDE",
      "link.active.foreground": "#8FCDDE",
      "scrollbar.thumb.background": "#7B88A380",
      "scrollbar.thumb.hover.background": "#7B88A3",
      "drag_border": "#88C0D0",
      "drop_target.background": "#88C0D033",
      "progress_bar.background": "#88C0D0",
      "skeleton.background": "#3B4252",
      "chart_1": "#88C0D0",
      "chart_2": "#81A1C1",
      "chart_3": "#5E81AC",
      "chart_4": "#A3BE8C",
      "chart_5": "#EBCB8B",
      "chart_bullish": "#A3BE8C",
      "chart_bearish": "#BF616A",
      "description_list_label.background": "#3B4252",
      "description_list_label.foreground": "#D8DEE9"
    }
  }]
}"##;

/// 赛博霓虹：近黑底 + 青/品红高饱和点缀（深色）
const CYBER_NEON_JSON: &str = r##"{
  "name": "iLauncher Skins",
  "themes": [{
    "name": "iLauncher Cyber Neon",
    "mode": "dark",
    "radius": 8,
    "colors": {
      "background": "#0B0B14",
      "foreground": "#D6D6F0",
      "border": "#23233A",
      "caret": "#00F0FF",
      "ring": "#00F0FF",
      "selection.background": "#00F0FF40",
      "muted.background": "#14141F",
      "muted.foreground": "#6E6E94",
      "accent.background": "#1E1E32",
      "accent.foreground": "#00F0FF",
      "accordion.background": "#14141F",
      "popover.background": "#14141F",
      "popover.foreground": "#E8E8FF",
      "sidebar.background": "#0B0B14",
      "sidebar.foreground": "#D6D6F0",
      "sidebar.border": "#1A1A2C",
      "sidebar.accent.background": "#23233A",
      "sidebar.accent.foreground": "#00F0FF",
      "group_box.background": "#12121F",
      "group_box.foreground": "#E8E8FF",
      "primary.background": "#00F0FF",
      "primary.hover.background": "#33F5FF",
      "primary.active.background": "#00C8D6",
      "primary.foreground": "#0B0B14",
      "secondary.background": "#2A2A44",
      "secondary.hover.background": "#343454",
      "secondary.active.background": "#23233A",
      "secondary.foreground": "#E8E8FF",
      "danger.background": "#FF3B5C",
      "danger.hover.background": "#FF5C78",
      "danger.active.background": "#E02E4C",
      "danger.foreground": "#FFFFFF",
      "info.background": "#4D9FFF",
      "info.hover.background": "#6FB2FF",
      "info.active.background": "#3B8DEF",
      "info.foreground": "#0B0B14",
      "success.background": "#00FF9D",
      "success.hover.background": "#33FFB1",
      "success.active.background": "#00E68C",
      "success.foreground": "#0B0B14",
      "warning.background": "#FFB800",
      "warning.hover.background": "#FFC633",
      "warning.active.background": "#E6A500",
      "warning.foreground": "#1A1400",
      "list.background": "#0B0B14",
      "list.active.background": "#00F0FF1A",
      "list.active.border": "#00F0FF",
      "list.hover.background": "#14141F",
      "list.even.background": "#0E0E18",
      "input.border": "#2E2E4A",
      "link.foreground": "#00F0FF",
      "link.hover.foreground": "#33F5FF",
      "link.active.foreground": "#33F5FF",
      "scrollbar.thumb.background": "#00F0FF66",
      "scrollbar.thumb.hover.background": "#00F0FF99",
      "drag_border": "#00F0FF",
      "drop_target.background": "#00F0FF26",
      "progress_bar.background": "#00F0FF",
      "skeleton.background": "#14141F",
      "chart_1": "#00F0FF",
      "chart_2": "#FF2E97",
      "chart_3": "#8B5CFF",
      "chart_4": "#FFB800",
      "chart_5": "#00FF9D",
      "chart_bullish": "#00FF9D",
      "chart_bearish": "#FF3B5C",
      "description_list_label.background": "#14141F",
      "description_list_label.foreground": "#D6D6F0"
    }
  }]
}"##;

/// 猫咖摩卡：Catppuccin Mocha 柔和暖灰紫（深色）
const CATPPUCCIN_JSON: &str = r##"{
  "name": "iLauncher Skins",
  "themes": [{
    "name": "iLauncher Catppuccin Mocha",
    "mode": "dark",
    "radius": 10,
    "colors": {
      "background": "#1E1E2E",
      "foreground": "#CDD6F4",
      "border": "#45475A",
      "caret": "#F5E0DC",
      "ring": "#89B4FA",
      "selection.background": "#89B4FA4D",
      "muted.background": "#313244",
      "muted.foreground": "#6C7086",
      "accent.background": "#45475A",
      "accent.foreground": "#CBA6F7",
      "accordion.background": "#313244",
      "popover.background": "#313244",
      "popover.foreground": "#CDD6F4",
      "sidebar.background": "#181825",
      "sidebar.foreground": "#CDD6F4",
      "sidebar.border": "#313244",
      "sidebar.accent.background": "#45475A",
      "sidebar.accent.foreground": "#CBA6F7",
      "group_box.background": "#262637",
      "group_box.foreground": "#CDD6F4",
      "primary.background": "#89B4FA",
      "primary.hover.background": "#9AB8FA",
      "primary.active.background": "#7AA2F7",
      "primary.foreground": "#1E1E2E",
      "secondary.background": "#585B70",
      "secondary.hover.background": "#676A82",
      "secondary.active.background": "#4B4E63",
      "secondary.foreground": "#CDD6F4",
      "danger.background": "#F38BA8",
      "danger.hover.background": "#F498B8",
      "danger.active.background": "#E5839E",
      "danger.foreground": "#1E1E2E",
      "info.background": "#89DCEB",
      "info.hover.background": "#99E2EF",
      "info.active.background": "#79D4E6",
      "info.foreground": "#1E1E2E",
      "success.background": "#A6E3A1",
      "success.hover.background": "#B5E8B1",
      "success.active.background": "#97DE91",
      "success.foreground": "#1E1E2E",
      "warning.background": "#F9E2AF",
      "warning.hover.background": "#FAE8BF",
      "warning.active.background": "#F6D89E",
      "warning.foreground": "#1E1E2E",
      "list.background": "#1E1E2E",
      "list.active.background": "#CBA6F724",
      "list.active.border": "#CBA6F7",
      "list.hover.background": "#313244",
      "list.even.background": "#232334",
      "input.border": "#585B70",
      "link.foreground": "#89B4FA",
      "link.hover.foreground": "#9AB8FA",
      "link.active.foreground": "#9AB8FA",
      "scrollbar.thumb.background": "#6C708680",
      "scrollbar.thumb.hover.background": "#6C7086",
      "drag_border": "#89B4FA",
      "drop_target.background": "#89B4FA29",
      "progress_bar.background": "#89B4FA",
      "skeleton.background": "#313244",
      "chart_1": "#F38BA8",
      "chart_2": "#FAB387",
      "chart_3": "#F9E2AF",
      "chart_4": "#A6E3A1",
      "chart_5": "#89B4FA",
      "chart_bullish": "#A6E3A1",
      "chart_bearish": "#F38BA8",
      "description_list_label.background": "#313244",
      "description_list_label.foreground": "#CDD6F4"
    }
  }]
}"##;

/// 暖阳米纸：暖白纸面 + 赤陶土点缀（浅色）
const WARM_PAPER_JSON: &str = r##"{
  "name": "iLauncher Skins",
  "themes": [{
    "name": "iLauncher Warm Paper",
    "mode": "light",
    "radius": 8,
    "colors": {
      "background": "#FAF6EE",
      "foreground": "#3B352C",
      "border": "#E4DBC8",
      "caret": "#B85C38",
      "ring": "#B85C38",
      "selection.background": "#E8B48A59",
      "muted.background": "#F1EADB",
      "muted.foreground": "#8C8171",
      "accent.background": "#EFE4CE",
      "accent.foreground": "#B85C38",
      "accordion.background": "#F1EADB",
      "popover.background": "#FFFCF5",
      "popover.foreground": "#3B352C",
      "sidebar.background": "#F5EFE3",
      "sidebar.foreground": "#3B352C",
      "sidebar.border": "#E4DBC8",
      "sidebar.accent.background": "#E9DCC3",
      "sidebar.accent.foreground": "#9C4A2B",
      "group_box.background": "#F3ECDD",
      "group_box.foreground": "#3B352C",
      "primary.background": "#B85C38",
      "primary.hover.background": "#A04E2F",
      "primary.active.background": "#8F4529",
      "primary.foreground": "#FFF8EF",
      "secondary.background": "#E4D8C0",
      "secondary.hover.background": "#DACBAE",
      "secondary.active.background": "#D2C29F",
      "secondary.foreground": "#4A4234",
      "danger.background": "#B3452E",
      "danger.hover.background": "#9E3B27",
      "danger.active.background": "#8D3522",
      "danger.foreground": "#FFF8EF",
      "info.background": "#4E7FA1",
      "info.hover.background": "#426E8D",
      "info.active.background": "#3A6482",
      "info.foreground": "#F5FAFD",
      "success.background": "#6A8E4E",
      "success.hover.background": "#5D7D44",
      "success.active.background": "#52703C",
      "success.foreground": "#F7FBF2",
      "warning.background": "#C9962E",
      "warning.hover.background": "#B78727",
      "warning.active.background": "#A67A22",
      "warning.foreground": "#3B2E10",
      "list.background": "#FAF6EE",
      "list.active.background": "#B85C381F",
      "list.active.border": "#B85C38",
      "list.hover.background": "#F1EADB",
      "list.even.background": "#F7F2E7",
      "input.border": "#D8CCB2",
      "link.foreground": "#B85C38",
      "link.hover.foreground": "#A04E2F",
      "link.active.foreground": "#A04E2F",
      "scrollbar.thumb.background": "#8C817166",
      "scrollbar.thumb.hover.background": "#8C8171",
      "drag_border": "#B85C38",
      "drop_target.background": "#B85C3826",
      "progress_bar.background": "#B85C38",
      "skeleton.background": "#EFE7D6",
      "chart_1": "#B85C38",
      "chart_2": "#C9962E",
      "chart_3": "#6A8E4E",
      "chart_4": "#4E7FA1",
      "chart_5": "#8A6FA8",
      "chart_bullish": "#6A8E4E",
      "chart_bearish": "#B3452E",
      "description_list_label.background": "#F1EADB",
      "description_list_label.foreground": "#6B6252"
    }
  }]
}"##;

/// 薄荷雾：灰绿纸面 + 薄荷绿点缀（浅色）
const MINT_MIST_JSON: &str = r##"{
  "name": "iLauncher Skins",
  "themes": [{
    "name": "iLauncher Mint Mist",
    "mode": "light",
    "radius": 8,
    "colors": {
      "background": "#F4F7F5",
      "foreground": "#24332C",
      "border": "#DCE6E0",
      "caret": "#2F9E77",
      "ring": "#2F9E77",
      "selection.background": "#2F9E7740",
      "muted.background": "#E8EFEB",
      "muted.foreground": "#7C8D84",
      "accent.background": "#DCEBE3",
      "accent.foreground": "#257A5E",
      "accordion.background": "#E8EFEB",
      "popover.background": "#FFFFFF",
      "popover.foreground": "#24332C",
      "sidebar.background": "#EDF3F0",
      "sidebar.foreground": "#24332C",
      "sidebar.border": "#DCE6E0",
      "sidebar.accent.background": "#D8E9E0",
      "sidebar.accent.foreground": "#1E6B51",
      "group_box.background": "#EAF1ED",
      "group_box.foreground": "#24332C",
      "primary.background": "#2F9E77",
      "primary.hover.background": "#2A8D6B",
      "primary.active.background": "#267D5F",
      "primary.foreground": "#FFFFFF",
      "secondary.background": "#D3E2DA",
      "secondary.hover.background": "#C3D6CC",
      "secondary.active.background": "#B5CCBF",
      "secondary.foreground": "#24332C",
      "danger.background": "#C4553B",
      "danger.hover.background": "#AD4A33",
      "danger.active.background": "#9C422E",
      "danger.foreground": "#FFFFFF",
      "info.background": "#3A7CA8",
      "info.hover.background": "#346E96",
      "info.active.background": "#2F6288",
      "info.foreground": "#FFFFFF",
      "success.background": "#2F9E77",
      "success.hover.background": "#2A8D6B",
      "success.active.background": "#267D5F",
      "success.foreground": "#FFFFFF",
      "warning.background": "#C9902D",
      "warning.hover.background": "#B87F24",
      "warning.active.background": "#A8721F",
      "warning.foreground": "#3A2E0D",
      "list.background": "#F4F7F5",
      "list.active.background": "#2F9E771C",
      "list.active.border": "#2F9E77",
      "list.hover.background": "#E8EFEB",
      "list.even.background": "#EFF4F1",
      "input.border": "#CBDAD2",
      "link.foreground": "#257A5E",
      "link.hover.foreground": "#2F9E77",
      "link.active.foreground": "#2F9E77",
      "scrollbar.thumb.background": "#7C8D8466",
      "scrollbar.thumb.hover.background": "#7C8D84",
      "drag_border": "#2F9E77",
      "drop_target.background": "#2F9E7726",
      "progress_bar.background": "#2F9E77",
      "skeleton.background": "#E8EFEB",
      "chart_1": "#2F9E77",
      "chart_2": "#3A7CA8",
      "chart_3": "#C9902D",
      "chart_4": "#C4553B",
      "chart_5": "#7A6FA8",
      "chart_bullish": "#2F9E77",
      "chart_bearish": "#C4553B",
      "description_list_label.background": "#E8EFEB",
      "description_list_label.foreground": "#24332C"
    }
  }]
}"##;

#[cfg(test)]
mod tests {
    use super::*;

    /// 每套皮肤 JSON 必须能解析为 ThemeSet，且主题名互不冲突、颜色表非空
    #[test]
    fn all_skin_jsons_parse_and_have_colors() {
        let mut names = std::collections::HashSet::new();
        for (id, _) in SKINS {
            if *id == DEFAULT_SKIN {
                continue;
            }
            let json = skin_json(id).unwrap_or_else(|| panic!("皮肤 {id} 缺少 JSON"));
            let set: ThemeSet =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("皮肤 {id} JSON 解析失败: {e}"));
            let theme = set.themes.first().unwrap_or_else(|| panic!("皮肤 {id} 没有主题"));
            assert!(!theme.colors.background.is_none(), "皮肤 {id} 缺 background");
            assert!(
                names.insert(theme.name.to_string()),
                "皮肤主题名重复: {}",
                theme.name
            );
        }
    }

    /// 皮肤 id 唯一且 default 在清单内
    #[test]
    fn skin_ids_unique_and_default_present() {
        let mut ids = std::collections::HashSet::new();
        for (id, key) in SKINS {
            assert!(ids.insert(*id), "皮肤 id 重复: {id}");
            assert!(key.starts_with("skin."), "皮肤 {id} 的 i18n 键不规范: {key}");
        }
        assert!(SKINS.iter().any(|(id, _)| *id == DEFAULT_SKIN));
    }

    /// current_skin 对未知注册表值回退 default（不依赖注册表：
    /// 未设置时 load_skin 返回 None → default）
    #[test]
    fn unknown_skin_falls_back_to_default() {
        assert!(SKINS.iter().any(|(id, _)| *id == "nord-night"));
        assert_eq!(skin_json(DEFAULT_SKIN), None);
        assert!(parse_skin("nord-night").is_some());
        assert!(parse_skin("does-not-exist").is_none());
    }
}
