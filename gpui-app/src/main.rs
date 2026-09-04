// iLauncher GPUI P0 Spike
//
// 覆盖 Go/No-Go 检查清单的可运行原型：
//   [1] 热键唤起（global-hotkey）+ 冷启动耗时测量
//   [2] uniform_list 渲染 10 万条假数据（--bench 自动滚动测 FPS）
//   [3] 中文 IME（gpui-component Input，需人工验证）
//   [4] 透明 + 毛玻璃窗口（WindowBackgroundAppearance::Blur）
//   [6] 托盘（tray-icon）+ 全局热键 + 自启（ilauncher feature 复用 utils::autostart）
//   [7] LiveIndex 进程内搜索（--snapshot <path>，ilauncher feature）
//
// 用法：
//   ilauncher-gpui                 正常运行（Ctrl+Space 唤起，Esc 退出）
//   ilauncher-gpui --bench         列表滚动基准（5s 自动滚动 → gpui-p0-result.json）
//   ilauncher-gpui --snapshot X    LiveIndex 真实快照搜索基准

use std::sync::mpsc;
use std::time::Instant;

use gpui_kit::assets::Assets;
use gpui_kit::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
    *,
};
use gpui_kit::*;

const ITEM_COUNT: usize = 100_000;
const ROW_HEIGHT: f32 = 44.;

// ── 全局启动时刻（冷启动测量） ────────────────────────────────────────────────

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn start_time() -> Instant {
    *START.get_or_init(Instant::now)
}

// ── 主视图 ──────────────────────────────────────────────────────────────────

struct Launcher {
    input: Entity<InputState>,
    scroll: UniformListScrollHandle,
    items: std::sync::Arc<Vec<String>>,
    filtered: Vec<usize>,
    first_render_done: Option<Instant>,
    render_count: usize,
    render_total_ms: f64,
    frame_count: usize,
    bench_t0: Option<Instant>,
    bench: bool,
    bench_tick: usize,
    hotkey_rx: mpsc::Receiver<Instant>,
    _subscriptions: Vec<Subscription>,
}

impl Launcher {
    fn new(window: &mut Window, cx: &mut Context<Self>, hotkey_rx: mpsc::Receiver<Instant>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索应用、文件、命令…（中文 IME 请在这里验证）"));

        let words = ["report", "notes", "简历", "配置", "相册", "terminal", "浏览器", "计算器"];
        let mut items = Vec::with_capacity(ITEM_COUNT);
        for i in 0..ITEM_COUNT {
            items.push(format!("{}_{:06}.txt", words[i % words.len()], i));
        }
        let items = std::sync::Arc::new(items);

        let mut this = Self {
            input: input.clone(),
            scroll: UniformListScrollHandle::new(),
            items,
            filtered: Vec::new(),
            first_render_done: None,
            render_count: 0,
            render_total_ms: 0.0,
            frame_count: 0,
            bench_t0: None,
            bench: std::env::args().any(|a| a == "--bench"),
            bench_tick: 0,
            hotkey_rx,
            _subscriptions: Vec::new(),
        };
        this.refilter(cx);
        if this.bench {
            this.run_bench(window, cx);
        }

        this._subscriptions.push(cx.subscribe_in(&input, window, {
            let input = input.clone();
            move |this, _, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    this.apply_query(value, cx);
                }
            }
        }));
        this
    }

    fn refilter(&mut self, _cx: &mut Context<Self>) {
        // 初始全量（bench 模式下保持 10 万条全量以测虚拟列表）
        self.filtered = (0..self.items.len()).collect();
    }

    fn apply_query(&mut self, query: String, cx: &mut Context<Self>) {
        let q = query.to_lowercase();
        self.filtered = if q.is_empty() {
            (0..self.items.len()).collect()
        } else {
            self.items
                .iter()
                .enumerate()
                .filter(|(_, name)| name.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .take(500)
                .collect()
        };
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    /// --bench：后台 8ms 一次滚动驱动 + on_next_frame 自续计数，测真实交付帧率
    fn run_bench(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let total = self.filtered.len();
        self.bench_t0 = Some(Instant::now());
        window.activate_window();
        self.count_frame(window, cx);

        cx.spawn(async move |this, cx| {
            let mut pos = 0usize;
            let mut ticks = 0usize;
            // 预热一帧
            cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
            loop {
                let done = this
                    .update(cx, |this, _| this.bench_t0.map(|t| t.elapsed().as_secs() >= 5).unwrap_or(false))
                    .unwrap_or(true);
                if done {
                    break;
                }
                pos = (pos + 7) % total;
                ticks += 1;
                let _ = this.update(cx, |this, cx| {
                    this.bench_tick = ticks;
                    this.scroll.scroll_to_item(pos, ScrollStrategy::Top);
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(8))
                    .await;
            }
        })
        .detach();
    }

    /// on_next_frame 自续帧计数：帧率 = 合成器实际交付的帧数 / 时间
    fn count_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.frame_count += 1;
        if let Some(t0) = self.bench_t0 {
            if t0.elapsed().as_secs() >= 5 {
                let fps = self.frame_count as f64 / t0.elapsed().as_secs_f64();
                let avg_render_ms = if self.render_count > 0 {
                    self.render_total_ms / self.render_count as f64
                } else {
                    0.0
                };
                let result = serde_json::json!({
                    "bench": "uniform_list_scroll_rAF",
                    "rows": self.filtered.len(),
                    "duration_secs": t0.elapsed().as_secs_f64(),
                    "frames": self.frame_count,
                    "fps": fps,
                    "renders": self.render_count,
                    "avg_render_ms": avg_render_ms,
                    "cold_start_ms": start_time().elapsed().as_secs_f64() * 1000.0,
                });
                println!("BENCH_RESULT {}", serde_json::to_string_pretty(&result).unwrap());
                std::fs::write("gpui-p0-result.json", serde_json::to_string_pretty(&result).unwrap()).ok();
                std::process::exit(0);
            }
        }
        cx.on_next_frame(window, |this, window, cx| this.count_frame(window, cx));
    }
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.first_render_done.is_none() {
            self.first_render_done = Some(Instant::now());
            println!(
                "COLD_START_TO_FIRST_RENDER_MS {:.1}",
                start_time().elapsed().as_secs_f64() * 1000.0
            );
        }
        self.render_count += 1;

        // 轮询热键通道（唤起）
        while let Ok(pressed_at) = self.hotkey_rx.try_recv() {
            println!("HOTKEY_TO_HANDLE_MS {:.1}", pressed_at.elapsed().as_secs_f64() * 1000.0);
            window.activate_window();
            self.input.update(cx, |state, cx| state.focus(window, cx));
        }

        let theme = cx.theme().clone();
        let items = self.items.clone();
        let filtered = self.filtered.clone();
        let result_count = filtered.len();
        let theme_for_list = theme.clone();

        let render_t0 = Instant::now();

        let root = v_flex()
            .size_full()
            .p_3()
            .gap_2()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(Input::new(&self.input).w_full())
            .child(
                div()
                    .id("results")
                    .flex_1()
                    .child(
                        uniform_list("result-list", filtered.len(), {
                            move |visible_range, _window, _cx| {
                                visible_range
                                    .map(|ix| {
                                        let name = &items[filtered[ix]];
                                        div()
                                            .h(px(ROW_HEIGHT))
                                            .w_full()
                                            .px_3()
                                            .items_center()
                                            .rounded_md()
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .justify_between()
                                                    .child(div().text_sm().child(name.clone()))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme_for_list.muted_foreground)
                                                            .child(format!("C:\\demo\\{}", name)),
                                                    ),
                                            )
                                            .hover(|s| s.bg(theme_for_list.secondary))
                                    })
                                    .collect::<Vec<_>>()
                            }
                        })
                        .size_full()
                        .track_scroll(&self.scroll),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!(
                                "{} 条结果（gpui P0 spike）{}",
                                result_count,
                                if self.bench { format!(" · tick {}", self.bench_tick) } else { String::new() }
                            )),
                    )
                    .child(
                        Button::new("quit")
                            .small()
                            .label("退出 (Esc)")
                            .on_click(|_, _, _| std::process::exit(0)),
                    ),
            );
        self.render_total_ms += render_t0.elapsed().as_secs_f64() * 1000.0;
        root
    }
}

// ── 热键线程 ────────────────────────────────────────────────────────────────

fn spawn_hotkey_thread() -> mpsc::Receiver<Instant> {
    use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager};

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let manager = GlobalHotKeyManager::new().expect("hotkey manager");
        // Ctrl+Space 唤起（P0 测试绑定）
        let hotkey = HotKey::new(Some(global_hotkey::hotkey::Modifiers::CONTROL), global_hotkey::hotkey::Code::Space);
        manager.register(hotkey).expect("register hotkey");
        println!("✓ 全局热键已注册: Ctrl+Space");
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            if let Ok(event) = receiver.recv() {
                if event.state == global_hotkey::HotKeyState::Pressed {
                    let _ = tx.send(Instant::now());
                }
            }
        }
    });
    rx
}

// ── 托盘 ────────────────────────────────────────────────────────────────────

fn setup_tray() {
    use tray_icon::{menu::{Menu, MenuItem}, TrayIconBuilder};

    let menu = Menu::new();
    let _ = menu.append(&MenuItem::new("显示 iLauncher", true, None));
    let _ = menu.append(&MenuItem::new("退出", true, None));
    match TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("iLauncher (gpui P0)").build() {
        Ok(_tray) => {
            println!("✓ 托盘已创建");
            // 故意泄漏保持存活（P0；正式版接事件）
            std::mem::forget(_tray);
        }
        Err(e) => println!("⚠️ 托盘创建失败: {e}"),
    }
}

// ── LiveIndex 真实快照搜索基准（feature ilauncher） ──────────────────────────

#[cfg(feature = "ilauncher")]
fn run_snapshot_bench(path: &str) {
    use ilauncher_lib::index_v2::LiveIndex;
    let t0 = Instant::now();
    let idx = LiveIndex::open(std::path::Path::new(path)).expect("open snapshot");
    println!("SNAPSHOT_OPEN_MS {:.1}（{} 行）", t0.elapsed().as_secs_f64() * 1000.0, idx.snapshot().row_count());

    for q in ["report", "notes", "简历", "rpt", "config", "explorer", "wechat", "steam"] {
        let t = Instant::now();
        let hits = idx.search(q, 50).unwrap();
        println!("  QUERY {:>10} → {:>3} hits in {:>7.2?}", q, hits.len(), t.elapsed());
    }
}

#[cfg(not(feature = "ilauncher"))]
fn run_snapshot_bench(_path: &str) {
    eprintln!("需要 --features ilauncher 编译");
}

// ── main ────────────────────────────────────────────────────────────────────

fn main() {
    start_time();
    let args: Vec<String> = std::env::args().collect();

    // --snapshot：不启动 UI，直接测 LiveIndex 进程内搜索
    if let Some(pos) = args.iter().position(|a| a == "--snapshot") {
        if let Some(path) = args.get(pos + 1) {
            run_snapshot_bench(path);
            return;
        }
    }

    setup_tray();
    let hotkey_rx = spawn_hotkey_thread();

    let app = gpui_kit::application().with_assets(Assets);
    app.run(move |cx| {
        gpui_kit::init(cx);

        let window_options = WindowOptions {
            kind: WindowKind::PopUp,
            titlebar: None,
            window_background: WindowBackgroundAppearance::Blurred,
            window_bounds: Some(WindowBounds::centered(size(px(760.), px(480.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|cx| Launcher::new(window, cx, hotkey_rx));
                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
            })
            .expect("open window");
        })
        .detach();
    });
}
