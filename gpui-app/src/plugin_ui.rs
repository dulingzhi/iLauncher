//! 插件市场 / 已安装管理窗口（Windows）。
//! 市场浏览 + 安装（下载 .ilp → 安装管线）+ 启用/禁用 + 卸载；
//! 已安装插件为 JS/WASM 引擎包（与内置 Rust 插件的 PluginManager 相互独立，
//! 对齐 旧版两套系统并存的设计）。

use std::sync::Arc;

use gpui_kit::component::list::ListItem;
use gpui_kit::component::{button::Button, checkbox::Checkbox, input::{Input, InputEvent, InputState}, *};
use gpui_kit::*;

use crate::plugin::{InstalledPlugin, PluginInstaller, PluginListItem, PluginRegistry, PluginStore, SearchParams};

const PAGE_SIZE: u32 = 20;

/// 市场状态三元组（注册表 + 安装器 + 商店客户端），窗口间共享
pub struct MarketState {
    pub registry: Arc<PluginRegistry>,
    pub installer: Arc<PluginInstaller>,
    pub store: PluginStore,
}

impl MarketState {
    pub fn new(plugins_dir: std::path::PathBuf, cache_dir: std::path::PathBuf) -> Self {
        let registry = Arc::new(PluginRegistry::new(plugins_dir));
        let installer = Arc::new(PluginInstaller::new(registry.clone()));
        let store = PluginStore::new(cache_dir);
        Self { registry, installer, store }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MarketMode {
    Market,
    Installed,
}

pub struct MarketPanel {
    input: Entity<InputState>,
    mode: MarketMode,
    items: Vec<PluginListItem>,
    installed: Vec<InstalledPlugin>,
    busy: bool,
    status: String,
    state: Arc<MarketState>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl MarketPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, state: Arc<MarketState>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索市场插件…"));
        let focus = cx.focus_handle();

        let mut this = Self {
            input: input.clone(),
            mode: MarketMode::Market,
            items: Vec::new(),
            installed: state.registry.list(),
            busy: false,
            status: String::new(),
            state,
            focus,
            _subscriptions: Vec::new(),
        };

        // 市场搜索防抖（词在刷新时从 input 重读）
        this._subscriptions.push(cx.subscribe_in(&input, window, {
            move |this, _, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.search_debounced(cx);
                }
            }
        }));

        this.load_market_popular(cx);
        this
    }

    fn set_status(&mut self, cx: &mut Context<Self>, status: impl Into<String>) {
        self.status = status.into();
        cx.notify();
    }

    /// 初始列表：热门插件
    fn load_market_popular(&mut self, cx: &mut Context<Self>) {
        let Some(client) = crate::http_util::client("iLauncher/plugin-market") else {
            self.set_status(cx, "HTTP 客户端创建失败");
            return;
        };
        let state = self.state.clone();
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = state.store.popular(client.as_ref(), PAGE_SIZE).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(items) => {
                        this.items = items;
                        this.set_status(cx, format!("市场热门 {} 个", this.items.len()));
                    }
                    Err(e) => this.set_status(cx, format!("加载失败: {e:#}")),
                }
            });
        })
        .detach();
    }

    /// 市场搜索（80ms 防抖）
    fn search_debounced(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(80)).await;
            let _ = this.update(cx, |this, cx| this.search_market(cx));
        })
        .detach();
    }

    fn search_market(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().trim().to_string();
        if query.is_empty() {
            self.load_market_popular(cx);
            return;
        }
        let Some(client) = crate::http_util::client("iLauncher/plugin-market") else {
            self.set_status(cx, "HTTP 客户端创建失败");
            return;
        };
        let state = self.state.clone();
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = state
                .store
                .search(client.as_ref(), SearchParams {
                    query: Some(query),
                    page: 1,
                    per_page: PAGE_SIZE,
                    ..Default::default()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(result) => {
                        this.items = result.plugins;
                        this.set_status(cx, format!("{} 条结果", result.total));
                    }
                    Err(e) => this.set_status(cx, format!("搜索失败: {e:#}")),
                }
            });
        })
        .detach();
    }

    /// 安装：下载 .ilp → 安装管线 → 刷新两个列表
    fn install(&mut self, plugin_id: String, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(client) = crate::http_util::client("iLauncher/plugin-market") else {
            self.set_status(cx, "HTTP 客户端创建失败");
            return;
        };
        let state = self.state.clone();
        self.busy = true;
        self.set_status(cx, format!("正在安装 {plugin_id}…"));
        cx.spawn(async move |this, cx| {
            let result = async {
                let ilp = state.store.download(client.as_ref(), &plugin_id, None).await?;
                state.installer.install(&ilp).map(|_| ())
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.installed = this.state.registry.list();
                match result {
                    Ok(()) => this.set_status(cx, format!("✓ {plugin_id} 安装完成")),
                    Err(e) => this.set_status(cx, format!("安装失败: {e:#}")),
                }
            });
        })
        .detach();
    }

    fn uninstall(&mut self, plugin_id: String, cx: &mut Context<Self>) {
        match self.state.installer.uninstall(&plugin_id) {
            Ok(()) => {
                self.installed = self.state.registry.list();
                self.set_status(cx, format!("✓ {plugin_id} 已卸载"));
            }
            Err(e) => self.set_status(cx, format!("卸载失败: {e:#}")),
        }
    }

    fn toggle_enabled(&mut self, plugin_id: String, enabled: bool, cx: &mut Context<Self>) {
        match self.state.registry.set_enabled(&plugin_id, enabled) {
            Ok(()) => {
                self.installed = self.state.registry.list();
                self.set_status(cx, format!("{plugin_id} → {}", if enabled { "启用" } else { "禁用" }));
            }
            Err(e) => self.set_status(cx, format!("切换失败: {e:#}")),
        }
    }

    fn switch_mode(&mut self, mode: MarketMode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.installed = self.state.registry.list();
        cx.notify();
    }
}

impl Render for MarketPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let theme_for_list = theme.clone();
        let items = self.items.clone();
        let installed = self.installed.clone();
        let mode = self.mode;
        let busy = self.busy;
        let status = self.status.clone();

        let mut root = v_flex()
            .id("market-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_this, ev: &KeyDownEvent, window, _cx| {
                if ev.keystroke.key.as_str() == "escape" {
                    window.remove_window();
                }
            }))
            .size_full()
            .p_3()
            .gap_2()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("market-tab")
                                    .small()
                                    .label(if mode == MarketMode::Market { "● 市场" } else { "市场" })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.switch_mode(MarketMode::Market, cx)
                                    })),
                            )
                            .child(
                                Button::new("installed-tab")
                                    .small()
                                    .label(if mode == MarketMode::Installed {
                                        format!("● 已安装（{}）", installed.len())
                                    } else {
                                        format!("已安装（{}）", installed.len())
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.switch_mode(MarketMode::Installed, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(if busy { "处理中…" } else { "" }),
                    ),
            );

        if mode == MarketMode::Market {
            root = root.child(Input::new(&self.input).w_full()).child(
                div()
                    .id("market-results")
                    .flex_1()
                    .child(
                        uniform_list("market-list", items.len(), {
                            let panel = cx.entity();
                            move |visible_range, _window, _cx| {
                                visible_range
                                    .map(|ix| {
                                        let item = &items[ix];
                                        let title = format!("{} v{}", item.name, item.version);
                                        let subtitle = format!(
                                            "{} · {} 下载 · {:.1} 分",
                                            item.description, item.downloads, item.rating
                                        );
                                        let plugin_id = item.id.clone();
                                        ListItem::new(ix)
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .justify_between()
                                                    .gap_2()
                                                    .child(
                                                        v_flex()
                                                            .min_w_0()
                                                            .child(div().text_sm().child(title))
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(theme_for_list.muted_foreground)
                                                                    .truncate()
                                                                    .child(subtitle),
                                                            ),
                                                    )
                                                    .child(
                                                        Button::new(("install", ix))
                                                            .small()
                                                            .label("安装")
                                                            .on_click({
                                                                let panel = panel.clone();
                                                                let plugin_id = plugin_id.clone();
                                                                move |_, _, cx| {
                                                                    panel.update(cx, |this: &mut MarketPanel, cx| {
                                                                        this.install(plugin_id.clone(), cx);
                                                                    });
                                                                }
                                                            }),
                                                    ),
                                            )
                                    })
                                    .collect::<Vec<_>>()
                            }
                        })
                        .size_full(),
                    ),
            );
        } else {
            root = root.child(
                div()
                    .id("installed-results")
                    .flex_1()
                    .child(
                        uniform_list("installed-list", installed.len(), {
                            let panel = cx.entity();
                            move |visible_range, _window, _cx| {
                                visible_range
                                    .map(|ix| {
                                        let plugin = &installed[ix];
                                        let title = format!("{} v{}", plugin.manifest.name, plugin.manifest.version);
                                        let plugin_id = plugin.manifest.id.clone();
                                        let enabled = plugin.enabled;
                                        ListItem::new(ix)
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .justify_between()
                                                    .gap_2()
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .items_center()
                                                            .min_w_0()
                                                            .child(
                                                                Checkbox::new(("enabled", ix))
                                                                    .checked(enabled)
                                                                    .on_click({
                                                                        let panel = panel.clone();
                                                                        let plugin_id = plugin_id.clone();
                                                                        move |checked: &bool, _window, cx: &mut App| {
                                                                            let checked = *checked;
                                                                            panel.update(cx, |this: &mut MarketPanel, cx| {
                                                                                this.toggle_enabled(plugin_id.clone(), checked, cx);
                                                                            });
                                                                        }
                                                                    }),
                                                            )
                                                            .child(
                                                                v_flex()
                                                                    .min_w_0()
                                                                    .child(div().text_sm().child(title))
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(theme_for_list.muted_foreground)
                                                                            .truncate()
                                                                            .child(plugin.manifest.description.clone()),
                                                                    ),
                                                            ),
                                                    )
                                                    .child(
                                                        Button::new(("uninstall", ix))
                                                            .small()
                                                            .label("卸载")
                                                            .on_click({
                                                                let panel = panel.clone();
                                                                let plugin_id = plugin_id.clone();
                                                                move |_, _, cx| {
                                                                    panel.update(cx, |this: &mut MarketPanel, cx| {
                                                                        this.uninstall(plugin_id.clone(), cx);
                                                                    });
                                                                }
                                                            }),
                                                    ),
                                            )
                                    })
                                    .collect::<Vec<_>>()
                            }
                        })
                        .size_full(),
                    ),
            );
        }

        root.child(
            h_flex()
                .w_full()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(if status.is_empty() { "Esc 关闭".to_string() } else { status }),
                )
                .child(
                    Button::new("market-refresh")
                        .small()
                        .label("刷新")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.installed = this.state.registry.list();
                            if this.mode == MarketMode::Market {
                                this.search_market(cx);
                            } else {
                                cx.notify();
                            }
                        })),
                ),
        )
    }
}
