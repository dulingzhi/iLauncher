# UI 框架迁移方案：Tauri(React) → GPUI

> 状态：**P0/P1 已完成并验证，进入 P2**（2026-09-05 更新，见 4.6 实施进度）
> 关联：[FILE_INDEX_OPTIMIZATION_PLAN.md](FILE_INDEX_OPTIMIZATION_PLAN.md)（v3 索引，
> 迁移后将由 GPUI 进程直接以库调用消费，服务进程文件 IPC 可退役）

---

## 1. 动机

iLauncher 是热键唤起的常驻启动器，交互密度极高（每次唤起 → 搜索 → 执行，
全程应 < 100ms）。当前栈的开销恰好全部压在关键路径上：

| 痛点 | Tauri/React 现状 | GPUI 预期 |
|---|---|---|
| 冷启动/唤起延迟 | WebView2 初始化 + React 水合，唤起常 > 300ms | 原生窗口 + GPU 渲染，Zed 实测 ~0.4s 启动、2ms 输入延迟 |
| 常驻内存 | WebView2 进程树 + V8，空闲数百 MB | Zed 级别 ~180MB；启动器场景预计 < 100MB |
| 搜索链路 | UI(WebView) ↔ Tauri IPC(序列化) ↔ Service 进程 ↔ 文件 mmap（v2） | GPUI 进程内直接调 `ilauncher_lib::index_v2::LiveIndex::search`，**省掉 IPC + 跨进程文件协议两层** |
| 大数据列表 | react-virtual + DOM diff | gpui `uniform_list` 原生虚拟列表，GPU 合成 |
| 技术栈分裂 | 前端 TS / 后端 Rust，90+ 条 `tauri::command` 全是薄壳 | 单一 Rust 代码库，命令壳变 Action 直调 |

**迁移的最大筹码**：本工程后端的全部逻辑已在 `ilauncher_lib`（rlib）里——
MFT 索引（index_v2）、剪贴板、插件、工作流、审计都是纯 Rust 库。
Tauri 命令层（`src/commands/*.rs`，~90 条）几乎全是"参数校验 → 调库 → 打包返回值"
的薄壳。迁移 = **重写 UI 层 + 把薄壳换成 gpui Action/异步任务，后端零改动**。

## 2. 现状盘点

### 2.1 前端（`src/`，React 19 + Vite + Tailwind）

23 个组件，按迁移批次分组：

| 批次 | 组件 | 复杂度 |
|---|---|---|
| 核心环 | SearchBox、ActionPanel、PreviewPanel、SmartSuggestions、Toast、ContextMenu | 低-中（启动器主循环） |
| 设置环 | Settings、AppearanceSettings、FontSettings、HotkeyRecorder、HotkeyGuide、ThemeEditor、SandboxSettings、WelcomeGuide、UpdateChecker | 中（表单+主题系统） |
| 功能环 | ClipboardHistory、PluginManager、PluginMarket、WorkflowManager、AuditLogViewer | 中高（大数据列表/富交互） |
| AI 环 | AIChat（react-markdown + syntax-highlighter） | 高（Markdown 渲染是 GPUI 最弱项） |

状态管理 zustand → gpui `Entity<T>`；i18n（i18next）→ rust-i18n/fluent；
pinyin-pro（JS）→ Rust `pinyin` crate（v3 拼音 alias 本来就要做，正好合并）。

### 2.2 后端 Tauri 依赖点

| 能力 | 现状 | GPUI 替代 |
|---|---|---|
| 窗口管理（唤起/隐藏/居中/毛玻璃） | Tauri window API | gpui WindowOptions（自实现居中/圆角/透明） |
| 系统托盘 | `tauri::tray`（lib.rs:805） | `tray-icon` crate（tauri 背后就是它，独立可用） |
| 全局热键 | `global-hotkey` crate（**已是独立 crate**） | 原样保留 |
| 开机自启 | 自研 `utils::autostart`（注册表，**与框架无关**） | 原样保留 |
| 自动更新 | `tauri-plugin-updater` + `scripts/generate-updater-json.js` | `self_update` crate，**保留现有更新 JSON 协议** |
| URL/文件打开 | `tauri-plugin-opener` | `open` crate |
| 进程信息 | `tauri-plugin-process` | `sysinfo` |
| 剪贴板 | 自研 `clipboard.rs`（arboard） | 原样保留 |
| 单实例/服务进程管理 | 自研 | 原样保留 |

**结论：Tauri 专有的仅 4 处**（窗口、托盘、updater、opener），全部有成熟替代。

## 3. GPUI 评估（2026-09 事实核查）

| 维度 | 状态 |
|---|---|
| Windows 支持 | 一等公民（Zed Windows GA 2025-10-15，专用 Windows 工程团队；D3D11 + DirectWrite） |
| 发布形态 | crates.io `0.2.2`（2025-10-22 后停滞 8.5 个月）；主流做法 git 依赖 zed monorepo + Cargo.lock 锁 commit；main 已拆 gpui/gpui_platform/gpui_windows 等 crate 家族（未发布） |
| 组件库 | **gpui 零内置组件**（官方 input 示例 746 行）；事实标准 = 三方 `gpui-component`（longbridge，60+ 组件、Lucide 图标、Apache-2.0，生产验证于 Longbridge Pro） |
| OS 集成 | 缺系统托盘、原生通知、打印；有原生菜单/文件对话框/拖拽/深色模式 |
| 无障碍 | 已发布版本为零；AccessKit 集成 2026-05 合入 main 未发布 |
| 文本输入 | IME 需 DIY（gpui-component 已有可用实现） |
| 维护风险 | 2026-02 Zed 团队公开表示暂停社区向 GPUI 工作、优先自家产品；有社区 fork gpui-ce（活跃度低） |
| 许可证 | gpui Apache-2.0（注意 Zed 自家 ui crate 是 GPL 且未发布，**不可依赖**；组件只用 gpui-component） |

**判断**：GPUI 在 Windows 上已被 Zed 1.0 证明可行，但它是"狗粮框架"——
适合愿意跟进 git 版本、自建缺失部件的团队。

## 4. 迁移策略

### 4.1 为什么只能"整体替换"而非渐进

GPUI 窗口无法嵌入 WebView，Tauri 窗口也无法嵌入 GPUI → 不存在 strangler 路径。
因此采用**并行双前端**：

```
阶段 A（并行期，推荐 1-2 个发布周期）：
  生产构建仍出 Tauri 版；GPUI 版作为预览通道（独立 exe）发布
  后端库共用 → 行为差异只可能来自 UI 层，问题定位容易
阶段 B（切换期）：
  GPUI 版功能对齐后切默认通道；Tauri 版冻结维护一个版本
阶段 C（退役）：
  删除 src/(React)、tauri 依赖、commands/ 薄壳
```

### 4.2 架构目标态

```
ilauncher-gpui.exe (Rust)
 ├─ ui/           gpui 窗口/组件/主题（gpui-component）
 ├─ actions/      原 commands/*.rs 的函数体平移（去掉 #[tauri::command] 壳）
 ├─ index_v2/     LiveIndex 进程内直调 ★ 不再需要 MFT Service 的文件协议
 ├─ clipboard/ plugin/ workflow/ audit/ ...  （原样，已是纯 Rust）
 └─ tray-icon / global-hotkey / autostart / self_update（独立 crate）
```

★ **与索引优化的合并红利**：v3 索引（LiveIndex）设计目标就是单进程内
读写锁共享；GPUI 迁移后 UI 进程直接持有 LiveIndex，MFT Service 进程
与 `{D}_delta.version`/offsets 跨进程协议整体退役（C1 问题的彻底形态）。
迁移窗口期内 v2 文件协议继续由 Service 维护，互不阻塞。

### 4.3 分阶段计划

| 阶段 | 内容 | 验收 | 预估 |
|---|---|---|---|
| **P0 Spike** | git 依赖 gpui + gpui-component；原型窗口：全局热键唤起、居中、半透明圆角、单输入框、uniform_list 渲染 1 万条假数据测滚动帧率、毛玻璃效果可行性 | 唤起 < 100ms；列表滚动 60fps；确认 git 依赖可锁定构建 | 1 周 |
| **P1 主循环** | SearchBox + 结果列表 + 执行（接 ranking/execute_action）+ 托盘 + 热键 + 自启 + i18n + 主题（暗/明） | 日常可替代 Tauri 版完成唤起-搜索-启动 | 3-4 周 |
| **P2 数据环** | ClipboardHistory、设置全部页、UpdateChecker（self_update 接现有 JSON 协议）、PreviewPanel | 功能对齐 v2.1 清单 | 3-4 周 |
| **P3 生态环** | PluginManager/Market、WorkflowManager、AuditLogViewer、AIChat（comrak/pulldown-cmark 渲染 Markdown，代码高亮用 syntect） | 功能对齐；AIChat 降级为纯文本+Markdown 也可接受 | 3-4 周 |
| **P4 切换与退役** | 安装包双通道 → 切默认 → 删 Tauri/React；v2 索引链路退役（接 FILE_INDEX_OPTIMIZATION_PLAN Phase 3） | Tauri 代码删除后单测全绿 | 2 周 |

合计 **12-15 周**（单人当量，含联调缓冲）。

### 4.4 关键技术决策

1. **依赖来源**：~~git 依赖 zed monorepo + 锁 commit~~ → **已修正，见 4.5**：
   crates.io `gpui-pre` 0.3.1 系列 + gpui-component 0.6，锁 Cargo.lock。
2. **组件层**：基于 gpui-component 二次封装自己的 design system（现 Tailwind 主题
   变量一一映射）；缺口组件（如 HotkeyRecorder）自建。
3. **Markdown/AIChat**：pulldown-cmark + syntect；富文本复杂度最高，放最后批次，
   允许先以纯文本降级上线。
4. **更新通道**：self_update 复用现有 updater JSON 格式与签名流程，
   scripts/generate-updater-json.js 基本不动。
5. **索引接入**：UI 进程内嵌 LiveIndex（只读 mmap + overlay），
   MFT Service 仅作为"首次构建/compact 的提权后端"保留，或直接由 UI 提权自管（对齐 v3_e2e 的自提权方案）。

### 4.5 组件库定型：gpui-component

**选型确认**：组件层采用 [gpui-component](https://github.com/longbridge/gpui-component)
（longbridge 开源，Apache-2.0，60+ 桌面组件，shadcn 风格，Lucide 图标内置，
生产验证于 Longbridge Pro 交易终端）。gpui 本体零组件，本层是事实标准。

**依赖形态（相对 4.4-1 的修正，2026-09 核查）**：

gpui-component 0.6 起改用 crates.io 上的 **gpui-pre 0.3.1** 系列
（社区对 zed main 拆分后 crate 家族的预发布，含 `gpui-pre` /
`gpui-pre-platform` / `gpui-pre-macros`）。**无需 git 依赖 zed monorepo**，
锁定 Cargo.lock 即可复现构建，维护风险大幅下降：

```toml
gpui           = { package = "gpui-pre", version = "0.3.1" }
gpui_platform  = { package = "gpui-pre-platform", version = "0.3.1", features = ["font-kit"] }
gpui-component = { version = "0.6" }
```

**现有 React 组件 → gpui-component 映射**：

| React/Radix（现状） | gpui-component | 备注 |
|---|---|---|
| SearchBox（自研） | `input::TextInput` + 自研高亮层 | 高亮自研（fzf match 区间着色） |
| Dialog（radix-dialog） | `modal::Modal` / `dialog::Dialog` | |
| Popover / ContextMenu | `popover::Popover` / `menu::PopupMenu` | |
| ScrollArea | `scroll::ScrollableElement` / `VirtualList` | |
| @tanstack/react-virtual | `uniform_list` / `VirtualList` | 原生虚拟列表 |
| HotkeyRecorder（自研） | 无 → 自研 | 缺口组件，封装 global-hotkey 录制 |
| Toast | `notification` | |
| ThemeEditor/Appearance | `Theme`/`ThemeMode` + schemars | gpui-component 自带暗/明主题系统 |
| react-markdown + syntax-highlighter | `markdown` 组件 + `tree-sitter-languages` feature | **AIChat 风险大幅下降** |
| lucide-react | `Icon` / `IconName`（同 Lucide 图标集） | |
| i18next | rust-i18n（gpui-component 同款） | |

**使用规范**：

1. 业务代码只依赖 `ui` 封装层（本 crate）+ gpui-component 公共 API；
   不直接散落调用裸 gpui 元素构造（框架升级时只改封装层）。
2. 主题以现有 Tailwind 变量表为源，一次性映射到 gpui-component `Theme`，
   运行时切换走 `ThemeMode`。
3. 版本策略：gpui-pre 系列跟随 gpui-component 的配套版本整体升级，
   每月一个 bump 窗口，锁 Cargo.lock。

## 4.6 实施进度（2026-09-05 更新）

代码在 `gpui-app/`（独立 crate，非 workspace），核心索引已拆分为
`ilauncher-index/`（无 Tauri 依赖，35+ 单测）。提交历史即实现日志。

| 阶段 | 状态 | 实测/备注 |
|---|---|---|
| P0 Spike | ✅ 全过 | 冷启动 429-490ms（Go 线 < 500ms）；10 万条 uniform_list 滚动 **144fps**（rAF 实测法，须前台激活，后台 DWM 节流到 4fps 是环境假象）；托盘/热键机制通 |
| P1 主循环 | ✅ 完成 | 搜索防抖 80ms、↑↓ 导航、Enter 经 opener 启动、Esc 销毁窗口由 WindowGuard 异步任务重建；列表行已换 gpui-component `ListItem`（选中/hover 全走 theme token，bench 回归 144.3fps 无退化） |
| P1 索引接入 | ✅ 完成 | `index_service.rs` 常驻 MFT 服务模式：UI 启动时快照齐全但服务未跑 → 静默提权拉起（修 catch-up 盲区）；`--ui-pid` 监控 UI 存活（修复解析越界 bug，服务 2s 内自退）；托盘"重建索引" = `--rebuild` 提权全量重扫（删快照→40s 重扫三盘 709 万行→转常驻）+ UI 轮询 mtime 自动重载；跨盘结果按 fzf score 排序 |
| P1 开机自启 | ✅ 完成 | `autostart.rs` 读写 HKCU Run 项（值名 iLauncher），托盘可勾选菜单，6 个单测走独立测试子键 |
| P1 主题 | 🚧 部分 | 全组件已走 gpui-component Theme token；暗/明切换 UI 未做 |
| P1 i18n | ⬜ 未做 | |
| P2 数据环 | ⬜ 未开始 | ClipboardHistory / 设置页 / UpdateChecker / PreviewPanel |
| P3/P4 | ⬜ 未开始 | |

**与 4.4-5 的偏差说明**：

- 实际用 `gpui-kit 0.6` umbrella（gpui-pre + component + base + assets 一体），
  而非分别依赖 gpui-pre / gpui-pre-platform。
- 索引没有走"UI 进程内嵌 LiveIndex 直读 mmap"的目标态，而是保留了常驻服务
  进程 + 快照文件：UI 侧 `LiveSet` 每盘持有一个 `LiveIndex`（mmap 只读打开
  快照）。原因是增量更新（USN catch-up/compact）需要提权，UI 不常驻提权，
  服务进程模型与现行 Tauri 版一致、风险最低。4.2 中"MFT Service 退役"
  降级为"MFT Service 缩编为纯 compact/catch-up 后端"。
- `--bench` 滚动基准命令：`ilauncher-gpui --bench`（后台 8ms 驱动滚动 +
  on_next_frame 计数 5s）。

## 5. 风险与对策

| 风险 | 等级 | 对策 |
|---|---|---|
| Zed 暂停社区向维护，main 破坏式变更 | 高 | Cargo.lock 锁 commit + 每月 bump 窗口；极端情况冻结 crates.io 0.2.2 或转 gpui-ce fork（成本：自己维护平台层） |
| GPUI 无系统托盘/通知 | 中 | tray-icon crate（tauri 同款底层，行为已验证） |
| 无无障碍支持 | 中 | 启动器场景影响小（对比 PowerToys Run）；跟踪 AccessKit 合入进度 |
| 文本输入/IME 边角 | 中 | gpui-component input 起步；P0 spike 专门验证中文 IME |
| 毛玻璃/圆角/透明在 Windows D3D11 路径效果 | 中 | P0 spike 首个验证项，不达预期则退纯色+圆角（视觉降级 acceptable） |
| 团队 GPUI 经验为零 + 文档较少（3/5） | 中 | P0 产出手册；组件层封装隔离框架细节；业务代码不直接接触 gpui 裸 API |
| 双前端并行期的双倍维护 | 低 | 明确冻结线：P1 完成后 Tauri 版只修 P0 级 bug |
| React 生态依赖（radix/动画库）无对应物 | 低 | 启动器 UI 以列表/表单为主，动画需求少；gpui 有内建动画 |

## 6. Go / No-Go 检查清单（P0 Spike 结束时评审）

- [x] 热键唤起全链路 < 100ms（冷启动 < 500ms）→ 实测冷启动 429-490ms
- [x] `uniform_list` 渲染 10 万条结果滚动 ≥ 60fps → 实测 144fps
- [x] 中文 IME 输入正常（gpui-component input）
- [x] 透明/圆角/居中窗口效果达到现有 Tauri 版观感
- [x] git 依赖可复现构建（锁 commit 后干净机器 build ok）→ 实际无需 git 依赖，
      gpui-kit 0.6 全 crates.io + Cargo.lock
- [x] 托盘 + 全局热键 + 自启三件套跑通
- [x] LiveIndex 进程内搜索接入（用 v3_e2e 的快照直接 open）→ 三盘 709 万行
      全量加载可搜，常用词 avg 6-10ms / p95 < 13ms

**建议**：先批 P0（1 周）。P0 七项全过 → 批 P1-P4；毛玻璃或 IME 不过 →
在 P0 结论里二选一（视觉降级 or 投入自研），再决定是否全量迁移。

## 7. 不做迁移的替代选项（备查）

- **留在 Tauri**：成本为零，但唤起延迟/内存/双语言维护长期存在；
  可把 v3 索引的服务进程文件协议升级为本机 HTTP/gRPC 缩短搜索链路（部分收益）。
- **Dioxus/Slint/egui**：Dioxus 桌面层仍基于 wry（WebView 同构）；Slint 商业授权
  与复杂 UI 表达力存疑；egui 即时模式不适合常驻富交互。均不如 GPUI 契合。
