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
    pub autostart: bool,
    pub clipboard_capacity: usize,
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
        autostart: crate::autostart::is_enabled(),
        clipboard_capacity: clipboard_capacity_or_default(),
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

fn model(cx: &App) -> Entity<SettingsModel> {
    cx.global::<SettingsModelGlobal>().0.clone()
}

/// 设置页标题表（单测直接验证它；build_pages 末尾 debug_assert 对账防脱节）
pub(crate) fn page_titles() -> Vec<&'static str> {
    let mut v = vec!["通用", "外观"];
    #[cfg(feature = "clipboard")]
    v.push("剪贴板");
    v.extend(["索引", "关于"]);
    v
}

/// 组装五个设置页（不依赖 Window，纯装配逻辑；值闭包的数据源是全局 model）
pub(crate) fn build_pages(
    index_set: &LiveSet,
    tx: &mpsc::Sender<AppSignal>,
    #[cfg(feature = "clipboard")] store: &Arc<Mutex<ClipboardStore>>,
) -> Vec<(&'static str, SettingPage)> {
    // ── 通用：自启动（真实写入注册表 Run 项）、热键（暂只读展示） ──
    let general = SettingPage::new("通用")
        .icon(IconName::Settings2)
        .group(
            SettingGroup::new().title("启动").item(
                SettingItem::new(
                    "开机自启动",
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
                .description("登录 Windows 后自动在后台运行 iLauncher"),
            ).item(
                // 热键注册在全局热键线程里，自定义绑定留待后续版本
                SettingItem::new(
                    "唤起热键",
                    SettingField::input(|_: &App| "Ctrl+Space".into(), |_, _| {}),
                )
                .disabled(true)
                .description("当前绑定；自定义热键将在后续版本开放"),
            ),
        );

    // ── 外观：深色模式（注册表 + 全局主题切换，与托盘菜单等价） ──
    let appearance = SettingPage::new("外观")
        .icon(IconName::Palette)
        .group(
            SettingGroup::new().title("主题").item(
                SettingItem::new(
                    "深色模式",
                    SettingField::switch(
                        |cx: &App| model(cx).read(cx).theme_dark,
                        |dark, cx: &mut App| {
                            let _ = crate::settings::save_theme_dark(dark);
                            model(cx).update(cx, |m, _| m.theme_dark = dark);
                            use gpui_kit::component::theme::ThemeMode;
                            gpui_kit::component::theme::Theme::change(
                                if dark { ThemeMode::Dark } else { ThemeMode::Light },
                                None,
                                cx,
                            );
                            cx.refresh_windows();
                        },
                    ),
                )
                .description("关闭时跟随 Windows 系统外观"),
            ),
        );

    let mut pages = vec![("通用", general), ("外观", appearance)];

    // ── 剪贴板：容量（注册表 + 运行时 set_capacity）、清空历史 ──
    #[cfg(feature = "clipboard")]
    {
        let store = store.clone();
        let clipboard = SettingPage::new("剪贴板")
            .icon(IconName::Copy)
            .group(
                SettingGroup::new().title("历史记录").item(
                    SettingItem::new(
                        "历史容量",
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
                    .description("保存的剪贴板条目上限，超出后按最旧截断（图片文件一并清理）"),
                ).item(
                    SettingItem::new(
                        "清空历史",
                        SettingField::render({
                            let store = store.clone();
                            move |_, _, _| {
                                let store = store.clone();
                                Button::new("clear-clipboard-history")
                                    .label("清空全部历史")
                                    .on_click(move |_, _, _| {
                                        store.lock().clear();
                                    })
                            }
                        }),
                    )
                    .description("删除全部文本与图片记录，图片文件一并删除，不可恢复"),
                ),
            );
        pages.push(("剪贴板", clipboard));
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
                SharedString::from("演示数据（未启用 ilauncher feature）")
            }
        }
    };
    let rebuild_tx = tx.clone();
    let index = SettingPage::new("索引")
        .icon(IconName::HardDrive)
        .group(
            SettingGroup::new().title("文件索引").item(
                SettingItem::new(
                    "索引状态",
                    SettingField::input(index_status, |_, _| {}),
                )
                .disabled(true)
                .description("常驻 MFT 服务维护；搜索窗口状态栏同源自适应"),
            ).item(
                SettingItem::new(
                    "重建索引",
                    SettingField::render(move |_, _, _| {
                        let tx = rebuild_tx.clone();
                        Button::new("rebuild-index")
                            .label("立即重建")
                            .on_click(move |_, _, _| {
                                let _ = tx.send(AppSignal::RebuildIndex);
                            })
                    }),
                )
                .description("提权全量重扫（约 40 秒），期间搜索结果可能不全"),
            ),
        );
    pages.push(("索引", index));

    // ── 关于：版本 / 数据目录 / 更新检查（占位） ──
    let about = SettingPage::new("关于")
        .icon(IconName::Info)
        .group(
            SettingGroup::new().title("应用").item(
                SettingItem::new(
                    "版本",
                    SettingField::input(
                        |_: &App| format!("{}（GPUI 预览版）", env!("CARGO_PKG_VERSION")).into(),
                        |_, _| {},
                    ),
                )
                .disabled(true),
            ).item(
                SettingItem::new(
                    "数据目录",
                    SettingField::input(
                        |_: &App| data_dir().to_string_lossy().into_owned().into(),
                        |_, _| {},
                    ),
                )
                .disabled(true)
                .description("剪贴板历史、图片与索引快照存放位置"),
            ).item(
                SettingItem::new(
                    "检查更新",
                    SettingField::render(move |_, _, cx: &mut App| {
                        let state = model(cx).read(cx).update_state.clone();
                        Button::new("check-update")
                            .label(state.button_label())
                            .disabled(!state.can_click())
                            .on_click(move |_, _, cx| dispatch_update_action(cx))
                    }),
                )
                .description(
                    "对接 GitHub releases latest.json（Tauri 同款协议）；点击按钮开始检查",
                ),
            ).item(
                // 动态状态行：进度 / 新版本号 / 失败原因都在这里显示
                SettingItem::new(
                    "更新状态",
                    SettingField::input(
                        |cx: &App| model(cx).read(cx).update_state.status_text().into(),
                        |_, _| {},
                    ),
                )
                .disabled(true),
            ),
        );
    pages.push(("关于", about));

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
            .child(
                Settings::new("app-settings")
                    .sidebar_width(px(200.))
                    .pages(pages.into_iter().map(|(_, page)| page)),
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
        let mut expected = vec!["通用", "外观"];
        #[cfg(feature = "clipboard")]
        expected.push("剪贴板");
        expected.extend(["索引", "关于"]);
        assert_eq!(page_titles(), expected);
    }
}
