# iLauncher 文件索引模块优化方案（参考 Lertaro）

> 日期：2026-09-05
> 范围：`src-tauri/src/mft_scanner/`（9 个文件，~3.2k 行）+ `src-tauri/src/plugin/file_search.rs`
> 参考：`D:\Projects\Lertaro`（.NET 10，Core/IndexV2 + Core/Indexer，~16k 行索引相关代码）

---

## 1. 两边架构现状

### 1.1 iLauncher 现状

```
UI 主进程 (Tauri)                MFT Service 子进程 (--mft-service)
├─ file_search 插件               ├─ MultiDriveScanner（启动全量重建）
│   └─ mft_cache: per-drive       │   └─ StreamingBuilder: FSCTL_ENUM_USN_DATA
│      (IndexQuery + PathReader)  │      两阶段（FRN map → BFS 路径物化）
│      └─ 3-gram FST 交集查询      │   └─ IndexBuilder: rayon 并行 3-gram
│         → bitmap → paths.dat    │      → FST + RoaringBitmap 落盘
│            → SkimMatcher 重排    ├─ UsnIncrementalUpdater（100ms 轮询 USN）
└─ 版本轮询 needs_reload()         │   → delta 追加 + deleted bitmap
   → 全量重新 open                 ├─ DeltaMerger（5min / 50MB 阈值合并重建 FST）
                                   └─ PID 监控，UI 退出即自毁
```

每盘文件：`{D}_paths.dat`（变长全路径记录）、`{D}_offsets.dat`、`{D}_index.fst`（3-gram→offset）、`{D}_bitmaps.dat`、`{D}_index_delta.dat`、`{D}_deleted.dat`、`{D}_index.version`、`.ready`。

### 1.2 Lertaro 现状（IndexV2 引擎）

```
Lertaro.Service (SYSTEM 服务，常驻)
├─ UsnIndexer：每盘一个 LiveIndex
│   ├─ Snapshot：单一 mmap 文件，列式 sections（NameIds/Flags/ParentIndexes/
│   │   Ids/Sizes/时间戳/唯一名池/uid→rows CSR/alias/orphan/ASCII 位图），
│   │   打开 O(1)，无解析、无对象分配
│   └─ DeltaOverlay：tombstone（删）+ override（改名/移动，保留行身份）
│       + Added（新行）+ RenamedAway + MetadataOverrides
│       ReaderWriterLockSlim：读锁并发搜索 / 写锁原子批量 USN 应用 / Compact 全折叠
├─ UsnMonitor：每盘一个，游标持久化；游标越界（< LowestValidUsn 或
│   JournalId 变化）→ 自动停止监控，触发该盘重索引
├─ MftIndexScanner：直接解析 $MFT 原始数据运行（MftDataRunParser，fixup 修复，
│   8MB 顺序块读，解析全部 $FILE_NAME 属性 → 硬链接一行一条），
│   不用 FSCTL_ENUM_USN_DATA
└─ Compact：idle/阈值触发，Snapshot+Delta 折叠 → temp + File.Replace 原子替换
   查询：charmask(u64) 位图预过滤（AVX2 向量化扫 unique 名）→ fzf 打分
        → FzfTopN 堆 → 头部精化（HighlightMask 权重）→ 流式输出，
        CancellationToken 取消被新键击取代的旧扫描；worker/slab/hitlist 全部池化
```

关键设计思想（IndexV2 的核心收益来源）：

1. **不物化完整路径**：每行只存 `name + parent_ref`，路径在使用时沿 ParentIndexes 链即时重建。重命名/移动目录天然正确，索引体积大幅缩小。
2. **Snapshot 不可变 + Overlay 小而可折叠**：mmap 快照只读，热更新全部落在内存 overlay；周期性 Compact 折叠回磁盘，原子替换。
3. **冷启动 = 打开旧快照 + USN catch-up**：持久化 `JournalId/NextUsn`，启动先 mmap 旧索引（O(1)），USN 水位有效则直接 replay 增量；只有水位失效才全量重建。
4. **唯一名字典**：匹配只扫 ~1-2M unique name（而非 2-3M 行），charmask 预过滤 + AVX2，模糊打分后再通过 uid→rows CSR 扇出到行。

---

## 2. 差距与问题清单（按严重程度排序）

### P0 — 正确性缺陷

| # | 问题 | 证据 | Lertaro 对照 |
|---|------|------|--------------|
| C1 | **UI 与 Service 之间的增量同步链路是断的**。`UsnIncrementalUpdater::attach_index()` 在整个代码库中从未被调用（只有定义和文档注释）；version 号只在 DeltaMerger 合并时才递增。后果：小于 50MB 的增量（正常使用几乎永远达不到）UI 进程**完全看不到**，新文件要等服务重启或偶发合并后才能搜到 | `grep attach_index` 仅命中 index_builder.rs 注释与 usn_incremental_updater.rs 定义；lib.rs 服务流程只调用 `initialize()` + `start_monitoring()` | LiveIndex 单进程内读写锁共享 Snapshot+Overlay，天然一致；跨进程则靠"游标持久化 + 启动 catch-up"，不存在中间态可见性问题 |
| C2 | **目录重命名导致整棵子树路径全部失效**。paths.dat 物化全路径；USN 对 RENAME_NEW_NAME 只处理该目录本身（旧 id tombstone + 新路径追加），子孙行的路径仍指向旧目录名 → 搜出的全是死路径 | usn_incremental_updater.rs `handle_usn_record` 只处理 FILE_CREATE/FILE_DELETE/RENAME_NEW_NAME | Lertaro 存 parent 引用，路径动态重建，目录改名/移动零成本、天然正确 |
| C3 | **FRN 缓存未命中时路径构建静默失败**。`query_frn_from_mft()` 是 stub，直接返回 `Ok(None)`（代码里自带 TODO）；新建深层文件（父目录不在增量缓存中）会索引成残缺路径或丢路径 | usn_incremental_updater.rs:597-637 | Lertaro 从根 FRN=5 全量建立 parent 结构，overlay 用 `ParentBaseRow/ParentFrn` 惰性解析 + orphan 恢复，不需要运行时查 MFT |
| C4 | **每次启动全量删库重建**。`check_and_cleanup_old_data()` 无条件删除全部 `.dat/.fst` 并从头扫 MFT，开机到可用索引需要完整扫描时间 | multi_drive_scanner.rs:222-233（注释明说"每次都清理，因为会全量重建"） | Lertaro 启动 = mmap 旧快照 + USN replay 追赶；仅水位失效才重建该盘 |
| C5 | 断电/崩溃无恢复保障：delta 文件追加写无 truncate 检测、无临时文件清理；主索引合并用 rename 覆盖被 mmap 的文件依赖 Windows 特性 | index_builder.rs / delta_merger.rs | Lertaro Snapshot 打开时校验文件长度 ≥ header 计算长度（truncated 检测）；SnapshotWriter temp-then-replace；Compact 失败可重新打开旧快照恢复 |

### P1 — 性能差距

| # | 问题 | Lertaro 对照 |
|---|------|--------------|
| P1 | **查询结果无相关性排序**：3-gram 交集后按 file_id 序取前 N（`take(limit)`），再用 SkimMatcherV2 重排但只在这 N 个里排；短查询（1-2 字符）走 FST 前缀 union，可能先物化几万 bitmap 再取前 20，排序依据的是 id 而非分数 | FzfTopN 堆直接按 fzf 分数保留全局 TopN；两阶段：粗排扫全量 unique，精化（昂贵的高亮权重）只对头部 ×5 headroom 做 |
| P2 | **每盘 offsets 索引常驻内存** Vec<usize>（2.2M × 8B ≈ 17MB/盘，无界增长）；paths.dat 全路径重复存储父路径前缀，体积大 | 列式 CSR + 唯一名池；打开 O(1)，常驻内存 = OS 页缓存按需触页 |
| P3 | 全量扫描用 FSCTL_ENUM_USN_DATA：每记录仅主文件名、变长记录串行解析、无法获得 size/mtime/硬链接 | MftIndexScanner 直接读 $MFT 数据运行：8MB 顺序块、fixup、解析所有 $FILE_NAME、扩展记录（$ATTRIBUTE_LIST）合并、顺带拿到 flags/size/三个时间戳 |
| P4 | 查询无取消机制：快速输入时多个过期扫描互相竞争 CPU | CancellationToken 按 chunk 取消；worker 池化零分配 |
| P5 | 每次 mmap 全量预热（含 50MB 采样 touch）拖慢首次查询路径 | 不需要预热——查询按需触页 |

### P2 — 功能差距

- 无拼音/别名索引（Lertaro AliasProvider 体系：拼音、西班牙语等，构建时烘焙进快照，带 provider fingerprint 失效检测）
- 无目录限定搜索 / 目录枚举 API（Lertaro DirectoryFilterResolver、EnumerateDirectory）
- 无网络盘/非 NTFS 卷监控（Lertaro FolderDriveMonitor + ReFsScanner + NetworkDrive 缓存，可后置）
- USN 只监听部分 reason（缺 RENAME_OLD_NAME、DATA_OVERWRITE/BASIC_INFO_CHANGE 元数据刷新）

---

## 3. 优化方案（分四个阶段）

### Phase 0：止血修复（1-2 天，不动数据格式）✅ 已完成（2026-09-05）

实施内容（详见提交记录）：
- delta 可见性协议落地：Service 每次 flush 后递增 `{D}_delta.version`；
  UI 查询前比对版本号，变化则 `hot_reload_delta()` + `PathReader::reload_paths()`，
  新文件/删除/改名即时可见（此前只有 DeltaMerger 合并时才可见）
- 新增 `{D}_offsets_delta.dat`：USN 追加路径的偏移量列表（file_id 从 offsets.dat
  条目数起连续编号），UI 据此扩展偏移索引读取新增路径
- 移除死代码：`attach_index` / `delta_state_handle`（跨进程共享 Arc 方案从未接线）
- 定期 flush：gram > 1000 立即刷；否则每 30s 刷一次；退出前 `finalize()` 落盘尾量
  （此前 < 1000 条的尾量直接丢失）
- 修复 `IndexQuery::search()` 双读锁不一致窗口（改为单次读锁）
- 全量重建完成后递增主索引版本号（同会话 Service 重启重建可被 UI 感知）
- 新增 `INDEX_IO_LOCK`：flush 与 merge 互斥（修复合并吞掉并发追加偏移量的竞态）
- DeltaMerger 合并时重建 offsets.dat（折入追加条目）并清理 offsets_delta.dat

遗留（随 Phase 2 处理）：启动全量重建守卫（需先有 USN 水位追赶，否则跳过重建
会导致索引丢失关闭期间的变更）。

### Phase 1：索引结构 v3 —— 列式快照 + parent 引用（2-3 周，核心阶段）

目标：解决 C2/C3/C5 与 P2，存储与查询性能对齐 IndexV2。

**新磁盘格式（每盘一个文件 `{D}.snapshot`，参考 SnapshotFormat）**

```
Header: magic | format_version | row_count | unique_count | journal_id |
        next_usn | volume_serial | is_complete | exclusion_fingerprint |
        source_root | sections_offset
Sections(16B 对齐, 即运行时布局, mmap 后直接当 typed span 用):
  Ids[u64] ParentRefs[u64] NameIds[u32] Flags[u16] Sizes[u64]
  CreationTimes[u32] LastWriteTimes[u32] LastAccessTimes[u32]
  UniqueMasks[u64 × unique_count]      ← charmask 预过滤位图，构建时烘焙
  NameOffsets[u32 × unique+1] NameBlob[u8]        ← 唯一名字符串池(UTF-8)
  UidStarts[u32 × unique+1] UidRows[u32 × row]    ← 同名行 CSR
  UniqueAsciiBits[u64 bitmap]                     ← ASCII 名零解码匹配
  (预留 AliasStarts/AliasBlob/Orphan sections)
```

关键决策（与 Lertaro 对齐）：

- **存 parent 引用而不是全路径**：行内只有 name 引用 + parent 行引用/FRN；`get_full_path(row)` 沿父链重建（深度上限 512 防损坏死循环）。重命名/移动只改一行。C2 根除。
- **唯一名字典化**：搜索 Phase A 只匹配 unique name（数量约为行数的 1/2 ~ 1/3），命中后经 UidRows 扇出行。charmask 预过滤用 AVX2（没有 AVX2 时标量，Rust 可用 `std::arch` 或先标量后加 `is_x86_feature_detected`）。
- **打开 O(1)**：mmap + 读 header + 计算 section offsets，不预热（P5）。
- **原子写与恢复**：写 `.{D}.snapshot.tmp` → flush → rename 覆盖；打开时校验文件长度 ≥ header 计算长度，损坏则降级重建该盘。
- FRN/Parent 解析在全量扫描时一次完成（BFS 自根向下），不存在 C3 的运行时查询。

**全量扫描改造（对齐 MftIndexScanner）**

- 用 `FSCTL_GET_NTFS_VOLUME_DATA` 拿 `mft_start_lcn/record_size/mft_valid_len`，解析 `$MFT` 自身记录的 `$DATA` data run（含 `$ATTRIBUTE_LIST` 扩展记录合并）。
- 8MB 块顺序 `ReadFile` + fixup 修复，跳过非 "FILE"  magic / 未使用记录；解析所有 `$FILE_NAME` 属性（硬链接一行一条）与 `$STANDARD_INFORMATION`（flags + 三个时间戳）。
- 收集 (frn, parent_frn, name, flags, size, times) 后：排序 → 唯一名池化 → 写列式快照。Rust 侧可用 mft crate 或自写 parser（自写约 600 行，参考 Lertaro MftParser/MftDataRunParser）。

**查询路径改造（对齐 NameSearch/SearchMatcher）**

```
query → charmask 预过滤(AVX2 扫 UniqueMasks) → 候选 unique name fzf/skim 打分
      → FzfTopN(容量 = limit × headroom) 堆维护 → 头部精化(高亮权重/连续度)
      → UidRows 扇出行 → 路径沿父链重建 → 流式回调输出
```

- 保留 3-gram FST 作为**可选的子串精确匹配后端**（它对 "包含某连续子串" 的查询比 fzf 更快更准），两种模式：默认 fzf 模糊；引号或前缀修饰符切换子串模式。也可在 Phase 1 先只上 fzf，FST 改造延后。
- 新增 CancellationToken 语义：tokio oneshot/Notify，新查询取消旧查询的扫描循环。

**✅ 已完成（2026-09-05，commit `overlay` 批次）**

- 列式快照底座 `index_v2/`：format（18 section 布局 + charmask）/ writer（唯一名池化、
  父引用解析、孤儿 section、temp+rename 原子替换）/ snapshot（mmap O(1) 打开、
  typed section 切片、路径沿父链重建、截断文件打开即报错）。
- 搜索 `search.rs`：charmask 预过滤 + fzf 打分 + rayon 并行 + `enumerate_directory`。
  性能冒烟（release，52.5 万行）：open 157µs，查询 ~5ms。
- **DeltaOverlay**（`overlay.rs`，C2 根除 + USN 增量语义）：
  tombstone（基线行墓碑）/ override（改名·移动·元数据，**保留行身份 → 目录 rename
  后子孙路径经父链自动跟随**）/ added（新行 + 墓碑位）三层；
  目录删除级联（基线 BFS 子行 + added parent_frn 链，已 override 移出的行不杀）；
  id（FRN）复用不复活墓碑；`compact()` 折叠为 IndexRecord 直接喂 write_snapshot；
  `search_with_overlay` / `enumerate_directory_with_overlay` 全链路 overlay 感知。
- 单元测试 18 个全绿，覆盖：目录 rename 子孙跟随、删除级联（含移出存活）、
  added 目录级联、id 复用、upsert 幂等、compact 往返、跨目录移动 compact。
- 死代码清理：`index_builder::build_from_paths`、streaming_builder 遗留 v1
  `{D}_index.dat` 写入链路、`quick_scan_mft_for_frn_map`（800MB 内存预载）、
  `query_frn_from_mft` stub（C3 的静默失败源）——全部删除，`cargo check` 零警告。
- **MFT 扫描输出接入 v3**（`v3_export.rs`）：`FrnMap → IndexRecord` 纯转换
  （BFS 自根 + 排除子树标记 + 孤儿保留真实 parent_frn），
  `StreamingBuilder::scan_mft_streaming_v3()` 复用阶段 1 FrnMap 直出
  `{D}.snapshot`。`ParentInfo` 补充 `is_dir`（USN record 属性位）。
  限制：USN ENUM 不提供 size/mtime，导出为 0（待 $MFT 自解析补）。

遗留（Phase 2）：USN 事件源接入 DeltaOverlay；水位持久化 + 启动 catch-up。

### Phase 2：冷启动 + USN catch-up（1 周）

目标：解决 C4，开机即可用。

1. **水位持久化**：快照 header 存 `journal_id + next_usn`（Phase 1 已预留）；运行时水位存内存 + 每次 delta flush 写 sidecar（`{D}.usn.cursor`）。
2. **启动流程**：
   ```
   打开快照(O(1)) → 查询 USN journal →
   ├─ journal_id 不同 / volume_serial 不同 / next_usn < lowest_valid_usn
   │   → 该盘全量重建（直接 $MFT 扫描）
   └─ 水位有效 → FSCTL_READ_USN_JOURNAL 从 cursor 批量读到 next_usn，
       replay 进 DeltaOverlay（可一次读多批）
   ```
3. **Overlay 语义升级**（对齐 DeltaOverlay）：现在 DeltaState 只有 gram→bitmap 与 deleted bitmap；升级为 tombstone / override / added 三层（overlay 存行结构而非 gram 结构），删除目录时级联 tombstone 子孙行（或在路径重建时检查祖先存活）。USN reason 补全：RENAME_OLD_NAME（tombstone 旧身份）、DATA_OVERWRITE/BASIC_INFO_CHANGE（re-stat 元数据）。
4. **Compact**：idle（IdleTrimGate：检测系统空闲/无查询 N 分钟）或 delta 超过阈值时，将 Snapshot+Overlay 折叠写新快照，原子替换，bump version；UI 侧版本轮询保持不变（已是成熟机制）。

**✅ 已完成（2026-09-05，commit `live_index` 批次）**

- `usn_journal.rs`：UsnEntry 纯数据 + `parse_usn_buffer` 安全解析（逐字段
  from_le_bytes，零 unsafe——DeviceIoControl 缓冲区不保证对齐，裸指针强转是
  未对齐读 UB）+ `check_water_level` 水位判定（Valid / NoWaterLevel /
  JournalRecreated）+ Windows-only 卷 I/O（open_volume / query_journal /
  read_all_pending 非阻塞批量读）。
- `live_index.rs` LiveIndex = Snapshot（mmap O(1) 打开）+ DeltaOverlay +
  水位：启动 open → catch_up_volume 校验水位并 replay → search/enumerate
  全链路 overlay 感知 → compact 折叠新快照（header 携带新水位）原子替换。
- replay 语义：CREATE / RENAME_NEW_NAME → upsert（rename 保留 FRN 身份，
  C2 天然正确）；DELETE → remove（目录级联）；RENAME_OLD_NAME 单独出现
  忽略；BASIC_INFO_CHANGE / DATA_* → touch_added（仅刷新 added 行 modified，
  基线行无 USN size 来源不建 override）。
- v3_export 快照 header 现在携带 journal_id + next_usn（扫描时从 USN journal
  查询写入），与 LiveIndex 水位衔接。
- 测试 25 个全绿（journal 解析往返/损坏拒绝/水位判定、replay 建改删移动/
  OLD_NAME 幂等/元数据刷新/compact 水位持久化重开、entry_for_id + 枚举）。

**✅ Phase 2 收尾（2026-09-05，commit `v3_service` 批次）**

- `v3_service.rs` V3DriveService（每盘一线程，lib.rs 监控阶段与 v2 并行启动）：
  - 启动守卫 `decide_startup`：快照存在且水位有效 → open + catch-up（秒开）；
    快照缺失/打开失败/journal 重建 → 才 `scan_mft_streaming_v3` 全量重建
    （C4 根除：不再每次启动无条件删库重扫）
  - 运行：每 2s `catch_up_volume`；`CompactPolicy`（pending ≥ 10 万且距上次
    ≥ 30min → compact）；运行中 journal 重建 → 自动全量重建恢复
  - 退出：有 pending 变更 → 最终 compact，水位随 header 持久化，
    下次启动从断点 catch-up
- DeltaOverlay 补 `pending_len()`（compact 策略输入）。
- UI 查询仍在 v2 链路（Phase 0 的增量同步），v3 并行维护直到 Phase 3
  查询切换；届时 v2（UsnIncrementalUpdater/DeltaMerger/paths.dat）退役。

Phase 2 完成。剩余工作均属 Phase 3：UI 查询切换 v3（含拼音 alias、目录
限定搜索、子串模式）、$MFT 自解析扫描器（补 size/mtime）、v2 链路退役清理。

### Phase 3：体验增强（按需，可并行）

- 拼音 Alias：独立 crate 生成拼音 alias，构建时烘焙进快照 Alias sections；provider fingerprint 变化触发强制重压缩（对齐 Lertaro AliasProviderRegistry）。
- 目录限定搜索：DirectoryFilterResolver（路径前缀 → 根行解析 → 行祖先成员缓存）。
- 目录枚举 API：`enumerate_directory(path, recursive, patterns)`（Lertaro ChildrenOf CSR 直接支持）。
- 服务常驻化（可选演进）：目前 Service 随 UI 启停，冷启动 = UI 启动即扫描；若改为常驻服务（Lertaro 模型），开机即完成 catch-up，UI 秒开。涉及安装器/权限，建议作为独立里程碑。
- 工程保障：为 overlay 合并、路径重建、快照读写写 Rust 单元测试 + 随机 fuzz（对照 Lertaro Tests 的 20-seed 随机 USN 流验证）；CI 加 format_version 兼容性测试。

### 明确不照搬的部分

- Lertaro 的 3 进程隔离（SYSTEM 服务 + UI + hook）：iLauncher 的 UI 子进程模型更简单，现阶段收益/成本不成比例。
- 网络盘 / WSL / 文件夹监控：需求出现时再引入（Lertaro 的 NetworkDrive 子系统约 2k 行，耦合较深）。
- C# 特有的 ReaderWriterLockSlim/GC 手段：Rust 用 `parking_lot::RwLock` + epoch/arc-swap 达到同等并发模型（读多写少，`arc_swap` 替换 snapshot 引用可做到读无锁）。

---

## 4. 迁移与兼容性

| 事项 | 方案 |
|------|------|
| 旧格式（v2 FST+paths.dat） | format_version 检测：v2 存在则按 Phase 2 启动流程先用旧索引服务，后台线程构建 v3 完成后原子切换；或直接删除重建（首次发布可接受，后续版本必须无缝） |
| 双进程协议 | 保持"文件 + version 号"协议不变：主索引 version（Compact 递增）与 delta version（flush 递增）分离；UI 每查询前 O(1) 读两个 version 文件 |
| USN cursor | sidecar 文件，与快照同目录同事务更新；cursor 丢失视为水位失效 → 重建 |
| 回滚 | v3 快照与 v2 文件并存于不同文件名，旧代码可继续读 v2 直到 v3 稳定 |

---

## 5. 风险与验证

- **$MFT 自解析风险**：需要管理员权限读卷（现有 FSCTL 路径同样要求）；fixup/扩展记录处理必须 fuzz（构造损坏记录不 panic、不越界）。缓解：保留 FSCTL_ENUM_USN_DATA 作为 fallback 扫描器。
- **overlay 复杂度**：tombstone/override 三层语义是正确性核心，参照 DeltaOverlay 的不变量（"每个 FRN 至多一条 live 目录行"）写 property 测试。
- **性能验收指标**（对照 Lertaro 量级）：
  - 冷启动到可查询：< 500ms（现：全量扫描数十秒 ~ 数分钟）
  - 单键击查询 P95：< 30ms（现：短查询 FST 前缀 union 可达百 ms）
  - 单盘索引内存：≤ 页缓存（现：17MB offsets + 数十 MB 结构/盘）
  - USN 变更可见延迟：< 2s（现：基本不可见，重启才见）
