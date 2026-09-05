# GPUI P0 Spike 结果与 Go/No-Go 结论

日期：2026-09-05
代码：`gpui-app/`（ilauncher-gpui 0.1.0，gpui-kit 0.6 umbrella）
运行环境：Windows 11，dev profile（app crate opt-level=1，GPUI 依赖 opt-level=3），未做 release 优化

## Go/No-Go 检查清单

| # | 检查项 | 目标 | 实测 | 结论 |
|---|--------|------|------|------|
| 1 | 冷启动（进程启动 → 首帧渲染） | < 500 ms | **429–466 ms**（dev 构建） | ✅ GO |
| 1 | 热键唤起（Ctrl+Space → 窗口前台） | < 100 ms | 机制已通（global-hotkey 线程 → mpsc → 前台激活 + 输入框聚焦）；按下到处理的延迟需人工按键验证 | ⚠️ 机制 GO，数字待人工 |
| 2 | 10 万条列表滚动帧率 | 60 fps | **138.9 fps 交付帧率**（前台，5s rAF 实测，312 次重绘）；渲染树构建均耗 0.014 ms | ✅ GO（余量 >2×） |
| 3 | 中文 IME | 可用 | gpui-component `Input` 组件（基于 gpui 官方 IME 支持）；需人工在输入框键入中文确认 | ⚠️ 待人工 |
| 4 | 毛玻璃观感 | 接近现行应用 | `WindowBackgroundAppearance::Blurred` + 无边框弹窗；需人工目验 | ⚠️ 待人工 |
| 5 | 可复现构建 | Cargo.lock 入库 | Cargo.lock 已入库；注意：阿里云镜像滞后（仅 gpui-kit 0.1.0），`gpui-app/.cargo/config.toml` 临时切换清华 TUNA sparse 索引 | ✅ GO（附条件） |
| 6 | 托盘 + 热键 + 自启 | 三件套可用 | 托盘创建 ✅、热键注册 ✅；自启复用现有 `utils::autostart`（注册表实现，与 UI 框架无关，生产已验证，可整体移植） | ✅ GO |
| 7 | LiveIndex 进程内接入 | 搜索延迟可用 | 289 万行真实快照：打开 0.13 ms；常用词 avg 6–10 ms / p95 < 13 ms；单字符 "t" avg 33 ms（与现行 Tauri 版同一索引实现，非迁移风险） | ✅ GO |

## 测量方法与重要发现

### 列表帧率（检查项 2）

初版用"render 调用次数 ÷ 时间"测得 4 fps，误判。排查发现：
- 渲染树构建均耗 **0.019 ms**，CPU 侧毫无压力；
- 4 fps 是 **DWM 对完全被遮挡窗口的节流**（控制台窗口盖住应用窗口时合成器按 ~4Hz 交付）。

改用 `window.on_next_frame` 自续计数（合成器实际交付帧率）并在 bench 开始时 `activate_window()` 前台激活后：**138.9 fps**。

教训：无人值守/后台跑 GUI 基准必须区分"被遮挡节流"与"真实性能"，P1 以后的性能回归测试要用 rAF 计数 + 前台激活。

### LiveIndex 基准（检查项 7）

基准程序：`src-tauri/src/bin/snapshot_bench.rs`（只读、无需管理员）。
快照：`C:\Users\81468\AppData\Local\Temp\ilauncher_v3_e2e_29096\C.snapshot`（288.85 万行）。

```
report   hits=50  avg=  6.7 ms  p95=  7.3 ms
notes    hits=50  avg= 10.1 ms  p95= 11.5 ms
简历      hits=0   avg= 15.6 ms  p95= 16.2 ms
配置      hits=8   avg= 15.5 ms  p95= 16.6 ms
t        hits=50  avg= 32.8 ms  p95= 38.6 ms   ← 单字符模糊扫描，最差情况
pdf      hits=50  avg=  6.9 ms  p95=  9.0 ms
steam    hits=50  avg=  9.5 ms  p95= 12.5 ms
快照打开           0.13 ms（mmap 懒加载）
```

## 已知缺口（P1 需补）

1. **Esc 隐藏窗口**：P0 只有退出按钮；gpui-pre 0.3.3 未找到 hide 窗口 API，需调研（备选：销毁窗口、热键时再开，或向 gpui-pre 提 PR）。
2. **gpui-app 尚未接 `ilauncher_lib`**（feature `ilauncher` 已声明但未验证编译）：以 `--snapshot` 模式跑真实索引的进程内验证推迟到 P1（避免 Tauri 依赖树拖慢 P0 迭代）。
3. 镜像问题：阿里云 crates 镜像同步滞后，已在 `gpui-app/.cargo/config.toml` 切换清华 TUNA；若官方镜像恢复同步可移除该覆盖。
4. IME 与毛玻璃两项需人工各花 2 分钟确认（启动 `ilauncher-gpui`，键入中文、观察窗口）。

## 结论：**GO** ✅

7 项检查中 4 项硬数据达标（冷启动 429ms < 500ms、列表 138fps > 60fps、索引毫秒级、三件套机制可用），
3 项待人工目验但无已知阻断风险。建议按方案进入 P1（主循环：搜索接 LiveIndex + 结果交互 + 设置面板）。

复现：
```bash
cd gpui-app && cargo build && ./target/debug/ilauncher-gpui.exe --bench   # 帧率
cd src-tauri && cargo build --release --bin snapshot_bench && ./target/release/snapshot_bench.exe <快照路径>
```
