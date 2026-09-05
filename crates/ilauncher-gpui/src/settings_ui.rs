//! 设置窗口（gpui-component 现成 Settings 组件：可拖拽侧栏 + 搜索 + 页/组/项/字段）。
//!
//! 架构分层：
//! - 可测逻辑全部下沉：注册表读写（settings.rs）、容量截断（ilauncher-clipboard）
//! - 本模块是纯 UI 装配层：SettingsModel 是 App 全局实体（字段值闭包的数据源），
//!   set 闭包负责「写真实来源（注册表/服务）→ 刷新 model → 副作用（主题/截断）」
//! - build_pages 不依赖 Window（SettingPage 是纯数据结构），可单测页清单

use std::sync::mpsc;

use gpui_kit::component::button::Button;
#[cfg(feature = "clipboard")]
use gpui_kit::component::setting::NumberFieldOptions;
use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings};
use gpui_kit::component::*;
use gpui_kit::*;

use crate::i18n::{self, t};
use crate::search::LiveSet;
use crate::updater;
use crate::AppSignal;

#[cfg(feature = "clipboard")]
use ilauncher_clipboard::ClipboardStore;
#[cfg(feature = "clipboard")]
use parking_lot::Mutex;
#[cfg(feature = "clipboard")]
use std::sync::Arc;

/// 设置快照（App 全局实体）。真实来源是注册表与各服务，这里只做会话内显示缓存。
pub struct SettingsModel {
    pub theme_dark: bool,
    /// 当前皮肤 id（skins.rs SKINS 清单；"default" = 内置明暗主题）
    pub skin: String,
    pub autostart: bool,
    pub clipboard_capacity: usize,
    /// 界面语言（注册表原始值："zh-CN" / "en" / "system"；未设置按跟随系统处理）
    pub language: String,
    /// 检查更新状态机（updater::UpdateState）
    pub update_state: updater::UpdateState,
}

/// Global trait 需要显式实现，包一层新类型
#[derive(Clone)]
struct SettingsModelGlobal(Entity<SettingsModel>);
impl Global for SettingsModelGlobal {}

/// 应用启动时初始化全局 model（读注册表当前值）
pub fn init_model(cx: &mut App) {
    let model = SettingsModel {
        theme_dark: crate::settings::load_theme_dark()
            .unwrap_or_else(crate::settings::system_prefers_dark),
        skin: crate::skins::current_skin(),
        autostart: crate::autostart::is_enabled(),
        clipboard_capacity: clipboard_capacity_or_default(),
        language: crate::settings::load_language()
            .unwrap_or_else(|| i18n::FOLLOW_SYSTEM.to_string()),
        update_state: updater::UpdateState::Idle,
    };
    let entity = cx.new(|_| model);
    let capacity = entity.read(cx).clipboard_capacity;
    cx.set_global(SettingsModelGlobal(entity));
    println!("✓ 设置 model 已加载（剪贴板容量 {capacity}）");
}

#[cfg(feature = "clipboard")]
fn clipboard_capacity_or_default() -> usize {
    crate::settings::load_clipboard_capacity().unwrap_or(ilauncher_clipboard::DEFAULT_CAPACITY)
}

#[cfg(not(feature = "clipboard"))]
fn clipboard_capacity_or_default() -> usize {
    crate::settings::load_clipboard_capacity().unwrap_or(500)
}

/// 托盘切换主题后同步 model（设置页开关显示与托盘一致）
pub fn sync_theme_model(cx: &mut App, dark: bool) {
    model(cx).update(cx, |m, _| m.theme_dark = dark);
}

/// 托盘/其他入口切换皮肤后同步 model（设置页下拉框显示一致）
pub fn sync_skin_model(cx: &mut App, skin: &str) {
    model(cx).update(cx, |m, _| m.skin = skin.to_string());
}

fn model(cx: &App) -> Entity<SettingsModel> {
    cx.global::<SettingsModelGlobal>().0.clone()
}

/// 设置页标题表（单测直接验证它；build_pages 末尾 debug_assert 对账防脱节）
pub(crate) fn page_titles() -> Vec<String> {
    let mut v = vec![t!("settings.page_general").to_string(), t!("settings.page_appearance").to_string()];
    #[cfg(feature = "clipboard")]
    v.push(t!("settings.page_clipboard").to_string());
    v.extend([
        t!("settings.page_index").to_string(),
        t!("settings.page_about").to_string(),
    ]);
    v
}

/// 组装五个设置页（不依赖 Window，纯装配逻辑；值闭包的数据源是全局 model）
pub(crate) fn build_pages(
    index_set: &LiveSet,
    tx: &mpsc::Sender<AppSignal>,
    #[cfg(feature = "clipboard")] store: &Arc<Mutex<ClipboardStore>>,
) -> Vec<(String, SettingPage)> {
    // ── 通用：自启动（真实写入注册表 Run 项）、热键（暂只读展示）、界面语言 ──
    let general = SettingPage::new(t!("settings.page_general").to_string())
        .icon(IconName::Settings2)
        .group(
            SettingGroup::new().title(t!("settings.general_group_start").to_string()).item(
                SettingItem::new(
                    t!("settings.general_autostart").to_string(),
                    SettingField::switch(
                        |cx: &App| model(cx).read(cx).autostart,
                        |on, cx: &mut App| {
                            let res = if on {
                                crate::autostart::enable()
                            } else {
                                crate::autostart::disable()
                            };
                            if res.is_ok() {
                                model(cx).update(cx, |m, _| m.autostart = on);
                            }
                        },
                    ),
                )
                .description(t!("settings.general_autostart_desc").to_string()),
            ).item(
                // 热键注册在全局热键线程里，自定义绑定留待后续版本
                SettingItem::new(
                    t!("settings.general_hotkey").to_string(),
                    SettingField::input(|_: &App| "Ctrl+Space".into(), |_, _| {}),
                )
                .disabled(true)
                .description(t!("settings.general_hotkey_desc").to_string()),
            ).item(
                SettingItem::new(
                    t!("settings.general_language").to_string(),
                    SettingField::dropdown(
                        vec![
                            (i18n::FOLLOW_SYSTEM.into(), t!("settings.lang_system").to_string().into()),
                            (i18n::ZH_CN.into(), "简体中文".into()),
                            (i18n::EN.into(), "English".into()),
                        ],
                        |cx: &App| model(cx).read(cx).language.clone().into(),
                        |value, cx: &mut App| {
                            let lang = value.to_string();
                            if i18n::switch_to(&lang).is_ok() {
                                model(cx).update(cx, |m, _| m.language = lang);
                            }
                            cx.refresh_windows();
                        },
                    ),
                )
                .description(t!("settings.general_language_desc").to_string()),
            ),
        );

    // ── 外观：皮肤选择 + 深色模式（注册表 + 全局主题切换，与托盘菜单等价） ──
    let skin_options: Vec<(SharedString, SharedString)> = crate::skins::SKINS
        .iter()
        .map(|(id, key)| ((*id).into(), t!(*key).to_string().into()))
        .collect();
    let appearance = SettingPage::new(t!("settings.page_appearance").to_string())
        .icon(IconName::Palette)
        .group(
            SettingGroup::new().title(t!("settings.appearance_group_theme").to_string()).item(
                SettingItem::new(
                    t!("settings.appearance_skin").to_string(),
                    SettingField::dropdown(
                        skin_options,
                        |cx: &App| model(cx).read(cx).skin.clone().into(),
                        |value, cx: &mut App| {
                            let id = value.to_string();
                            crate::skins::select_skin(&id, cx);
                        },
                    ),
                )
                .description(t!("settings.appearance_skin_desc").to_string()),
            ).item(
                SettingItem::new(
                    t!("settings.appearance_dark").to_string(),
                    SettingField::switch(
                        |cx: &App| model(cx).read(cx).theme_dark,
                        |dark, cx: &mut App| {
                            model(cx).update(cx, |m, _| m.theme_dark = dark);
                            crate::skins::set_dark_mode(dark, cx);
                        },
                    ),
                )
                .description(t!("settings.appearance_dark_desc").to_string()),
            ),
        );

    let mut pages = vec![
        (t!("settings.page_general").to_string(), general),
        (t!("settings.page_appearance").to_string(), appearance),
    ];

    // ── 剪贴板：容量（注册表 + 运行时 set_capacity）、清空历史 ──
    #[cfg(feature = "clipboard")]
    {
        let store = store.clone();
        let clipboard = SettingPage::new(t!("settings.page_clipboard").to_string())
            .icon(IconName::Copy)
            .group(
                SettingGroup::new().title(t!("settings.clipboard_group_history").to_string()).item(
                    SettingItem::new(
                        t!("settings.clipboard_capacity").to_string(),
                        SettingField::number_input(
                            NumberFieldOptions {
                                min: crate::settings::CLIPBOARD_CAPACITY_MIN as f64,
                                max: crate::settings::CLIPBOARD_CAPACITY_MAX as f64,
                                step: 10.0,
                            },
                            |cx: &App| model(cx).read(cx).clipboard_capacity as f64,
                            {
                                let store = store.clone();
                                move |v, cx: &mut App| {
                                    let cap = v as usize;
                                    let _ = crate::settings::save_clipboard_capacity(cap);
                                    store.lock().set_capacity(cap);
                                    model(cx).update(cx, |m, _| m.clipboard_capacity = cap);
                                }
                            },
                        )
                        .default_value(ilauncher_clipboard::DEFAULT_CAPACITY as f64),
                    )
                    .description(t!("settings.clipboard_capacity_desc").to_string()),
                ).item(
                    SettingItem::new(
                        t!("settings.clipboard_clear").to_string(),
                        SettingField::render({
                            let store = store.clone();
                            move |_, _, _| {
                                let store = store.clone();
                                Button::new("clear-clipboard-history")
                                    .label(t!("settings.clipboard_clear_button").to_string())
                                    .on_click(move |_, _, _| {
                                        store.lock().clear();
                                    })
                            }
                        }),
                    )
                    .description(t!("settings.clipboard_clear_desc").to_string()),
                ),
            );
        pages.push((t!("settings.page_clipboard").to_string(), clipboard));
    }

    // ── 索引：状态展示 + 重建（复用托盘同一信号通道） ──
    let index_status = {
        let index_set = index_set.clone();
        move |_: &App| {
            #[cfg(feature = "ilauncher")]
            {
                crate::search::SearchSource::Live(index_set.clone())
                    .status_text()
                    .into()
            }
            #[cfg(not(feature = "ilauncher"))]
            {
                let _ = &index_set;
                SharedString::from(t!("settings.index_demo").to_string())
            }
        }
    };
    let rebuild_tx = tx.clone();
    let index = SettingPage::new(t!("settings.page_index").to_string())
        .icon(IconName::HardDrive)
        .group(
            SettingGroup::new().title(t!("settings.index_group_files").to_string()).item(
                SettingItem::new(
                    t!("settings.index_status").to_string(),
                    SettingField::input(index_status, |_, _| {}),
                )
                .disabled(true)
                .description(t!("settings.index_status_desc").to_string()),
            ).item(
                SettingItem::new(
                    t!("settings.index_rebuild").to_string(),
                    SettingField::render(move |_, _, _| {
                        let tx = rebuild_tx.clone();
                        Button::new("rebuild-index")
                            .label(t!("settings.index_rebuild_button").to_string())
                            .on_click(move |_, _, _| {
                                let _ = tx.send(AppSignal::RebuildIndex);
                            })
                    }),
                )
                .description(t!("settings.index_rebuild_desc").to_string()),
            ),
        );
    pages.push((t!("settings.page_index").to_string(), index));

    // ── 关于：版本 / 数据目录 / 更新检查（占位） ──
    let about = SettingPage::new(t!("settings.page_about").to_string())
        .icon(IconName::Info)
        .group(
            SettingGroup::new().title(t!("settings.about_group_app").to_string()).item(
                SettingItem::new(
                    t!("settings.about_version").to_string(),
                    SettingField::input(
                        |_: &App| {
                            t!("settings.about_version_value", version = env!("CARGO_PKG_VERSION"))
                                .to_string()
                                .into()
                        },
                        |_, _| {},
                    ),
                )
                .disabled(true),
            ).item(
                SettingItem::new(
                    t!("settings.about_datadir").to_string(),
                    SettingField::input(
                        |_: &App| data_dir().to_string_lossy().into_owned().into(),
                        |_, _| {},
                    ),
                )
                .disabled(true)
                .description(t!("settings.about_datadir_desc").to_string()),
            ).item(
                SettingItem::new(
                    t!("settings.about_update").to_string(),
                    SettingField::render(move |_, _, cx: &mut App| {
                        let state = model(cx).read(cx).update_state.clone();
                        Button::new("check-update")
                            .label(state.button_label())
                            .disabled(!state.can_click())
                            .on_click(move |_, _, cx| dispatch_update_action(cx))
                    }),
                )
                .description(t!("settings.about_update_desc").to_string()),
            ).item(
                // 动态状态行：进度 / 新版本号 / 失败原因都在这里显示
                SettingItem::new(
                    t!("settings.about_update_status").to_string(),
                    SettingField::input(
                        |cx: &App| model(cx).read(cx).update_state.status_text().into(),
                        |_, _| {},
                    ),
                )
                .disabled(true),
            ),
        );
    pages.push((t!("settings.page_about").to_string(), about));

    debug_assert_eq!(
        pages.len(),
        page_titles().len(),
        "设置页清单与标题表脱节：build_pages 增删页时同步改 page_titles"
    );
    pages
}

/// 用户数据目录（剪贴板历史/图片、索引快照同目录）
fn data_dir() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(|d| std::path::PathBuf::from(d).join("iLauncher"))
        .unwrap_or_else(|| std::path::PathBuf::from("iLauncher"))
}

/// 「检查更新」按钮点击分派：按状态机推进
/// Idle/Failed → 发起检查；Available → 下载；Ready → 启动安装程序并退出
fn dispatch_update_action(cx: &mut App) {
    let m = model(cx);
    let state = m.read(cx).update_state.clone();
    match state {
        updater::UpdateState::Idle | updater::UpdateState::Failed(_) => {
            m.update(cx, |m, cx| {
                m.update_state = updater::UpdateState::Checking;
                cx.spawn(async move |m: WeakEntity<SettingsModel>, cx| {
                    let state = run_check().await;
                    let _ = m.update(cx, |m, _| m.update_state = state);
                })
                .detach();
            });
        }
        updater::UpdateState::Available(info) => {
            m.update(cx, |m, cx| {
                m.update_state = updater::UpdateState::Downloading(info.version.clone());
                cx.spawn(async move |m: WeakEntity<SettingsModel>, cx| {
                    let state = run_download(info).await;
                    let _ = m.update(cx, |m, _| m.update_state = state);
                })
                .detach();
            });
        }
        updater::UpdateState::Ready { path, .. } => match updater::launch_installer(&path) {
            Ok(()) => {
                println!("✓ 安装程序已启动，退出以释放文件锁");
                std::process::exit(0);
            }
            Err(e) => {
                m.update(cx, |m, _| {
                    m.update_state =
                        updater::UpdateState::Failed(format!("启动安装程序失败: {e:#}"));
                });
            }
        },
        // Checking / Downloading / UpToDate：按钮已禁用，防御性忽略
        _ => {}
    }
}

/// 更新检查/下载共用的 HTTP 客户端（构造收敛在 http_util）
fn http_client() -> Option<std::sync::Arc<dyn gpui_kit::http_client::HttpClient>> {
    crate::http_util::client(&format!("iLauncher/{}", updater::CURRENT_VERSION))
}

async fn run_check() -> updater::UpdateState {
    let Some(client) = http_client() else {
        return updater::UpdateState::Failed("HTTP 客户端创建失败".into());
    };
    updater::check(client.as_ref(), updater::CURRENT_VERSION)
        .await
        .unwrap_or_else(|e| updater::UpdateState::Failed(format!("{e:#}")))
}

async fn run_download(info: updater::UpdateInfo) -> updater::UpdateState {
    let Some(client) = http_client() else {
        return updater::UpdateState::Failed("HTTP 客户端创建失败".into());
    };
    match updater::download(client.as_ref(), &info).await {
        Ok(path) => updater::UpdateState::Ready {
            version: info.version.clone(),
            path,
        },
        Err(e) => updater::UpdateState::Failed(format!("{e:#}")),
    }
}

pub struct SettingsView {
    focus: FocusHandle,
    index_set: LiveSet,
    tx: mpsc::Sender<AppSignal>,
    _model_subscription: Subscription,
    #[cfg(feature = "clipboard")]
    clipboard_store: Arc<Mutex<ClipboardStore>>,
}

impl SettingsView {
    pub fn new(
        cx: &mut Context<Self>,
        index_set: LiveSet,
        tx: mpsc::Sender<AppSignal>,
        #[cfg(feature = "clipboard")] clipboard_store: Arc<Mutex<ClipboardStore>>,
    ) -> Self {
        // model 变化（主题/更新状态机）→ 通知视图重渲染
        let m = model(cx);
        let _model_subscription = cx.observe(&m, |_, _, cx| cx.notify());
        Self {
            focus: cx.focus_handle(),
            index_set,
            tx,
            _model_subscription,
            #[cfg(feature = "clipboard")]
            clipboard_store,
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let pages = build_pages(
            &self.index_set,
            &self.tx,
            #[cfg(feature = "clipboard")]
            &self.clipboard_store,
        );

        v_flex()
            .id("settings-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key.as_str() == "escape" {
                    window.remove_window();
                }
                cx.notify();
            }))
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(crate::window_drag::drag_strip(
                format!("iLauncher · {}", crate::i18n::t!("tray.settings")),
                &theme,
            ))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .child(
                        Settings::new("app-settings")
                            .sidebar_width(px(200.))
                            .pages(pages.into_iter().map(|(_, page)| page)),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    // 只导入被测项：glob 导入会把 gpui-component 的深嵌套类型拉进测试模块，
    // 触发 rustc 在 #[test] 展开期递归爆栈
    use super::page_titles;

    #[test]
    fn page_titles_cover_core_sections() {
        // 默认 locale zh-CN：标题表即中文文案（i18n 默认语言）
        crate::i18n::test_use_zh();
        let mut expected = vec!["通用".to_string(), "外观".to_string()];
        #[cfg(feature = "clipboard")]
        expected.push("剪贴板".to_string());
        expected.extend(["索引".to_string(), "关于".to_string()]);
        assert_eq!(page_titles(), expected);
    }
}
