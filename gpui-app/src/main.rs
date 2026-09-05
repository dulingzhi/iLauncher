// iLauncher GPUI P1 主循环原型
//
// 架构：Watcher 实体统一持有窗口生命周期
//   - Esc → window.remove_window() 销毁窗口（隐藏）
//   - Ctrl+Space 热键 / 托盘"显示" → Watcher 轮询通道，前台激活或重建窗口
//   - 输入防抖 80ms 后搜索（单字符查询 33ms 不掉帧）
//   - ↑↓ 选择、Enter 启动（opener）、托盘"退出"
//
// 用法：
//   ilauncher-gpui                      Demo 数据运行（不依赖 src-tauri）
//   ILAUNCHER_SNAPSHOT=<path> ilauncher-gpui   真实快照搜索（需 --features ilauncher 构建）
//   ilauncher-gpui --bench              列表滚动帧率基准
//   ilauncher-gpui --snapshot <path>    LiveIndex 进程内搜索基准（需 feature ilauncher）

mod search;

use std::sync::mpsc;
use std::time::Instant;

use gpui_kit::assets::Assets;
use gpui_kit::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
    *,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use search::{Entry, SearchSource};

const DEMO_COUNT: usize = 100_000;
const PAGE_LIMIT: usize = 50;
const ROW_HEIGHT: f32 = 44.;
/// 输入防抖：暂停输入这么久后才真正执行搜索
const DEBOUNCE_MS: u64 = 80;
/// 唤起信号轮询周期（热键 → 窗口激活的附加延迟 ≤ 该值）
const SIGNAL_POLL_MS: u64 = 16;

// ── 全局启动时刻（冷启动测量） ────────────────────────────────────────────────

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn start_time() -> Instant {
    *START.get_or_init(Instant::now)
}

// ── 唤起信号（热键 / 托盘菜单共用） ──────────────────────────────────────────

enum AppSignal {
    /// 唤起窗口（Instant 为按下时刻，用于测量唤起延迟）
    Show(Instant),
}

// ── 主视图 ──────────────────────────────────────────────────────────────────

struct Launcher {
    input: Entity<InputState>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    source: SearchSource,
    entries: std::rc::Rc<Vec<Entry>>,
    selected: usize,
    query_gen: usize,
    first_render_done: Option<Instant>,
    render_count: usize,
    render_total_ms: f64,
    frame_count: usize,
    bench_t0: Option<Instant>,
    bench: bool,
    bench_tick: usize,
    _subscriptions: Vec<Subscription>,
}

impl Launcher {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索应用、文件、命令…（中文 IME 请在这里验证）"));
        let focus = cx.focus_handle();

        let words = ["report", "notes", "简历", "配置", "相册", "terminal", "浏览器", "计算器"];
        let demo: Vec<Entry> = (0..DEMO_COUNT)
            .map(|i| Entry::new(format!("{}_{:06}.txt", words[i % words.len()], i), format!("C:\\demo\\{}_{:06}.txt", words[i % words.len()], i)))
            .collect();

        let bench = std::env::args().any(|a| a == "--bench");
        let source = SearchSource::from_env_or_demo(demo.clone());
        // bench 模式保持 10 万条全量以测虚拟列表；正常运行空查询显示空（启动器惯例）
        let entries = if bench { std::rc::Rc::new(demo) } else { std::rc::Rc::new(Vec::new()) };

        let mut this = Self {
            input: input.clone(),
            scroll: UniformListScrollHandle::new(),
            focus,
            source,
            entries,
            selected: 0,
            query_gen: 0,
            first_render_done: None,
            render_count: 0,
            render_total_ms: 0.0,
            frame_count: 0,
            bench_t0: None,
            bench,
            bench_tick: 0,
            _subscriptions: Vec::new(),
        };
        if this.bench {
            this.run_bench(window, cx);
        }

        this._subscriptions.push(cx.subscribe_in(&input, window, {
            let input = input.clone();
            move |this, _, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    this.apply_query_debounced(value, cx);
                }
            }
        }));
        this
    }

    /// 输入防抖：只执行最新一代查询，暂停输入 DEBOUNCE_MS 后真正搜索
    fn apply_query_debounced(&mut self, query: String, cx: &mut Context<Self>) {
        self.query_gen = self.query_gen.wrapping_add(1);
        let gen_id = self.query_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(DEBOUNCE_MS))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.query_gen == gen_id {
                    this.apply_query(query, cx);
                }
            });
        })
        .detach();
    }

    /// 防抖后到期的真正搜索：空查询清空结果，非空最多 PAGE_LIMIT 条
    fn apply_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.entries = std::rc::Rc::new(self.source.search(&query, PAGE_LIMIT));
        self.selected = 0;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    /// 移动选择（↑↓ 键）
    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        self.selected = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        self.scroll.scroll_to_item(self.selected, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// 启动当前选中项
    fn launch_selected(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get(self.selected) else { return };
        let path = entry.path.clone();
        println!("LAUNCH {}", path);
        match opener::open(path) {
            Ok(_) => cx.notify(),
            Err(e) => eprintln!("⚠ 打开失败: {e}"),
        }
    }

    /// 唤起时聚焦输入框（由 Watcher 调）
    fn focus_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| state.focus(window, cx));
    }

    /// --bench：后台 8ms 一次滚动驱动 + on_next_frame 自续计数，测真实交付帧率
    fn run_bench(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let total = self.entries.len();
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
                    "rows": self.entries.len(),
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.first_render_done.is_none() {
            self.first_render_done = Some(Instant::now());
            println!(
                "COLD_START_TO_FIRST_RENDER_MS {:.1}",
                start_time().elapsed().as_secs_f64() * 1000.0
            );
        }
        self.render_count += 1;
        let render_t0 = Instant::now();

        let theme = cx.theme().clone();
        let entries = self.entries.clone();
        let selected = self.selected;
        let result_count = entries.len();
        let theme_for_list = theme.clone();

        let root = v_flex()
            .id("root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                match ev.keystroke.key.as_str() {
                    "up" => this.move_selection(-1, cx),
                    "down" => this.move_selection(1, cx),
                    "enter" => this.launch_selected(cx),
                    // gpui-pre 无 hide API：销毁窗口，唤起时由 Watcher 重建
                    "escape" => window.remove_window(),
                    _ => {}
                }
            }))
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
                        uniform_list("result-list", entries.len(), {
                            move |visible_range, _window, _cx| {
                                visible_range
                                    .map(|ix| {
                                        let entry = &entries[ix];
                                        let is_selected = ix == selected;
                                        div()
                                            .h(px(ROW_HEIGHT))
                                            .w_full()
                                            .px_3()
                                            .items_center()
                                            .rounded_md()
                                            .cursor_pointer()
                                            .when(is_selected, |s| s.bg(theme_for_list.secondary))
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .justify_between()
                                                    .gap_2()
                                                    .child(div().text_sm().child(entry.name.clone()))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme_for_list.muted_foreground)
                                                            .truncate()
                                                            .child(entry.path.clone()),
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
                                "{} 条结果 · ↑↓ 选择 · Enter 打开 · Esc 隐藏{}",
                                result_count,
                                if self.bench { format!(" · tick {}", self.bench_tick) } else { String::new() }
                            )),
                    )
                    .child(
                        Button::new("quit")
                            .small()
                            .label("退出")
                            .on_click(|_, _, _| std::process::exit(0)),
                    ),
            );
        self.render_total_ms += render_t0.elapsed().as_secs_f64() * 1000.0;
        root
    }
}

// ── 窗口生命周期守护：隐藏后唤起重建 + 信号轮询 ─────────────────────────────
//
// 不用 GPUI 实体承载（实体在 app 初始化闭包返回后会被释放），
// 改为纯异步任务直接持有状态（rx + 窗口句柄），随任务存活。

struct WindowGuard {
    rx: mpsc::Receiver<AppSignal>,
    window: Option<(WindowHandle<Root>, Entity<Launcher>)>,
}

impl WindowGuard {
    fn new(rx: mpsc::Receiver<AppSignal>) -> Self {
        Self { rx, window: None }
    }

    /// 唤起：窗口还在就前台激活 + 聚焦输入；已被 Esc 销毁则重建
    fn summon(&mut self, pressed_at: Instant, cx: &mut AsyncApp) {
        if let Some((handle, launcher)) = &self.window {
            let activated = handle
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    launcher.update(cx, |l, cx| l.focus_input(window, cx));
                })
                .is_ok();
            if activated {
                println!("HOTKEY_TO_HANDLE_MS {:.1}", pressed_at.elapsed().as_secs_f64() * 1000.0);
                return;
            }
            self.window = None;
        }

        // 重建窗口（Esc 销毁后首次唤起 / 初始唤起）
        let options = make_window_options();
        let mut launcher_slot: Option<Entity<Launcher>> = None;
        let result = cx.open_window(options, |window, cx| {
            let launcher = cx.new(|cx| Launcher::new(window, cx));
            launcher_slot = Some(launcher.clone());
            cx.new(|cx| Root::new(launcher, window, cx).bg(cx.theme().background))
        });
        match result {
            Ok(handle) => {
                println!("SUMMON_REOPEN_MS {:.1}", pressed_at.elapsed().as_secs_f64() * 1000.0);
                self.window = Some((handle, launcher_slot.expect("launcher entity")));
            }
            Err(e) => eprintln!("⚠ 重建窗口失败: {e:#}"),
        }
    }
}

fn make_window_options() -> WindowOptions {
    WindowOptions {
        kind: WindowKind::PopUp,
        titlebar: None,
        window_background: WindowBackgroundAppearance::Blurred,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Point { x: px(200.), y: px(200.) },
            size: size(px(760.), px(480.)),
        })),
        ..Default::default()
    }
}

// ── 热键线程 ────────────────────────────────────────────────────────────────

fn spawn_hotkey_thread(tx: mpsc::Sender<AppSignal>) {
    use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager};

    std::thread::spawn(move || {
        let manager = GlobalHotKeyManager::new().expect("hotkey manager");
        // Ctrl+Space 唤起（P0/P1 测试绑定）
        let hotkey = HotKey::new(Some(global_hotkey::hotkey::Modifiers::CONTROL), global_hotkey::hotkey::Code::Space);
        manager.register(hotkey).expect("register hotkey");
        println!("✓ 全局热键已注册: Ctrl+Space");
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            if let Ok(event) = receiver.recv() {
                if event.state == global_hotkey::HotKeyState::Pressed {
                    let _ = tx.send(AppSignal::Show(Instant::now()));
                }
            }
        }
    });
}

// ── 托盘（菜单事件：显示 / 退出） ────────────────────────────────────────────

fn setup_tray(tx: mpsc::Sender<AppSignal>) {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::TrayIconBuilder;

    let menu = Menu::new();
    let _ = menu.append(&MenuItem::with_id("show", "显示 iLauncher", true, None));
    let _ = menu.append(&MenuItem::with_id("quit", "退出", true, None));
    match TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("iLauncher (gpui P1)").build() {
        Ok(_tray) => {
            println!("✓ 托盘已创建");
            // 故意泄漏保持存活（正式版接事件）
            std::mem::forget(_tray);
        }
        Err(e) => println!("⚠️ 托盘创建失败: {e}"),
    }

    // 菜单点击事件线程（muda）
    std::thread::spawn(move || {
        let receiver = MenuEvent::receiver();
        loop {
            if let Ok(event) = receiver.recv() {
                match event.id.0.as_ref() {
                    "show" => {
                        let _ = tx.send(AppSignal::Show(Instant::now()));
                    }
                    "quit" => std::process::exit(0),
                    _ => {}
                }
            }
        }
    });
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

    let (tx, rx) = mpsc::channel();
    setup_tray(tx.clone());
    spawn_hotkey_thread(tx.clone());
    // 启动即显示主窗口（含 bench 模式）；之后 Esc 隐藏、热键/托盘唤起
    let _ = tx.send(AppSignal::Show(Instant::now()));

    let app = gpui_kit::application().with_assets(Assets);
    app.run(move |cx| {
        gpui_kit::init(cx);
        let mut guard = WindowGuard::new(rx);
        cx.spawn(move |cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
            println!("✓ 窗口守护轮询已启动");
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(SIGNAL_POLL_MS))
                    .await;

                let mut latest: Option<Instant> = None;
                while let Ok(sig) = guard.rx.try_recv() {
                    match sig {
                        AppSignal::Show(t) => latest = Some(t),
                    }
                }
                if let Some(t) = latest {
                    guard.summon(t, &mut cx);
                }
            }
            }
        })
        .detach();
    });
}
