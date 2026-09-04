// IndexV2 增量覆盖层（DeltaOverlay） - 基线快照之上的 USN 增量状态
//
// 设计对齐 Lertaro IndexV2/DeltaOverlay，按本工程的简化模型实现：
// - 基线 Snapshot 不可变（mmap），所有增量只进 overlay
// - 三类状态：
//     deleted_base: 基线行墓碑（行号集合）
//     overrides:    基线行的改名/移动/元数据修正（保留行身份 → 子行 parent 链不断）
//     added:        基线中不存在的新行（含墓碑位 added_removed）
// - 目录删除级联：基线目录 → BFS 子行；added 目录 → parent_frn 链
// - compact() 把 基线行(应用 override、跳过墓碑) + 活 added 折叠成 IndexRecord 列表，
//   可直接喂给 writer::write_snapshot 生成新基线
//
// 正确性要点（相对旧 v2 物化全路径的修复）：
// - 目录 rename 只产生一条 override，子孙路径经父链自动跟随（无需重写子树）
// - id（FRN）复用：delete 后同 FRN 重建 → 新增 added 记录，绝不复活墓碑

use rustc_hash::{FxHashMap, FxHashSet};

use super::snapshot::Snapshot;
use super::writer::IndexRecord;

/// 覆盖层中一条记录的引用（搜索/枚举结果的来源标识）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRef {
    /// 基线快照行号
    Base(usize),
    /// added 列表下标
    Added(usize),
}

impl EntryRef {
    /// 确定性排序键（搜索合并打平同分顺序用）
    #[inline]
    pub fn ord_key(&self) -> (u8, usize) {
        match self {
            EntryRef::Base(r) => (0, *r),
            EntryRef::Added(i) => (1, *i),
        }
    }
}

/// 基线行的覆盖记录：改名/移动/元数据更新（行身份保留）
#[derive(Debug, Clone)]
pub struct OverrideRecord {
    pub name: String,
    /// 覆盖后的父行（基线行号）；父不在基线（added 父或未索引）时为 -1，用 parent_frn
    pub parent_row: i32,
    /// parent_row == -1 时的父 FRN（compact 时按 id 解析）
    pub parent_frn: u64,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u32,
}

/// 基线中不存在的新行
#[derive(Debug, Clone)]
pub struct AddedRecord {
    pub id: u64,
    /// 父目录 FRN（可为 added 目录的 id —— 按 id 解析不限于基线）
    pub parent_frn: u64,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u32,
}

/// 路径重建时的父链接（内部）
enum ParentLink {
    Base(usize),
    Frn(u64),
}

#[derive(Debug, Default)]
pub struct DeltaOverlay {
    /// 被删基线行号
    deleted_base: FxHashSet<u32>,
    /// 基线行覆盖（key = 基线行号）
    overrides: FxHashMap<u32, OverrideRecord>,
    /// 新增行
    added: Vec<AddedRecord>,
    /// added 行墓碑（与 added 同索引）
    added_removed: Vec<bool>,
    /// frn -> added 下标（含已删的，remove 后查得到墓碑）
    added_by_id: FxHashMap<u64, u32>,
    /// 相对基线冻结计数的增量
    file_count_delta: i64,
    dir_count_delta: i64,
}

impl DeltaOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    // ── 增量应用 ─────────────────────────────────────────────────────────

    /// 应用一条 USN 创建/重命名/元数据更新。
    ///
    /// 语义：同一 id 重复 upsert 幂等更新既有记录（override 或 added），
    /// 不产生重复行。rename = 对同一 id upsert（行身份保留 → 目录改名
    /// 后子孙 parent 链不断，路径自动跟随）。
    ///
    /// 注意：硬链接（同 id 多行）只覆盖首行，这是 USN 增量模型的固有近似。
    pub fn upsert(
        &mut self,
        snap: &Snapshot,
        id: u64,
        parent_frn: u64,
        name: &str,
        is_dir: bool,
        size: u64,
        modified: u32,
    ) {
        // 1. 活 added 记录 → 原地更新
        if let Some(&idx) = self.added_by_id.get(&id) {
            if !self.added_removed[idx as usize] {
                let rec = &mut self.added[idx as usize];
                if rec.is_dir != is_dir {
                    if is_dir {
                        self.dir_count_delta += 1;
                        self.file_count_delta -= 1;
                    } else {
                        self.dir_count_delta -= 1;
                        self.file_count_delta += 1;
                    }
                }
                rec.parent_frn = parent_frn;
                rec.name.clear();
                rec.name.push_str(name);
                rec.is_dir = is_dir;
                rec.size = size;
                rec.modified = modified;
                return;
            }
        }

        // 2. 基线行存在：
        if let Some(row) = snap.first_row_for_id(id) {
            let row32 = row as u32;
            if self.deleted_base.contains(&row32) {
                // id 复用（删除后重建）→ 走新增，不复活墓碑
            } else if let Some(o) = self.overrides.get_mut(&row32) {
                // 已 override → 原地更新
                o.parent_row = snap.first_row_for_id(parent_frn).map(|r| r as i32).unwrap_or(-1);
                o.parent_frn = parent_frn;
                o.name.clear();
                o.name.push_str(name);
                o.is_dir = is_dir;
                o.size = size;
                o.modified = modified;
                return;
            } else {
                // 活基线行 → 覆盖（保留行身份）
                let parent_row = snap.first_row_for_id(parent_frn).map(|r| r as i32).unwrap_or(-1);
                self.overrides.insert(
                    row32,
                    OverrideRecord {
                        name: name.to_string(),
                        parent_row,
                        parent_frn,
                        is_dir,
                        size,
                        modified,
                    },
                );
                return;
            }
        }

        // 3. 新增行
        let idx = self.added.len() as u32;
        self.added.push(AddedRecord {
            id,
            parent_frn,
            name: name.to_string(),
            is_dir,
            size,
            modified,
        });
        self.added_removed.push(false);
        self.added_by_id.insert(id, idx);
        if is_dir {
            self.dir_count_delta += 1;
        } else {
            self.file_count_delta += 1;
        }
    }

    /// 删除一个文件/目录（目录级联删除子孙）。返回是否有状态变化。
    pub fn remove(&mut self, snap: &Snapshot, id: u64) -> bool {
        let mut changed = false;

        // added 记录（含其 added 子孙）
        if let Some(&idx) = self.added_by_id.get(&id) {
            if !self.added_removed[idx as usize] {
                self.tombstone_added(idx as usize);
                changed = true;
            }
        }

        // 基线行
        if let Some(row) = snap.first_row_for_id(id) {
            let row32 = row as u32;
            if self.deleted_base.contains(&row32) {
                return changed;
            }
            let is_dir = self
                .overrides
                .get(&row32)
                .map(|o| o.is_dir)
                .unwrap_or_else(|| snap.is_dir(row));
            self.deleted_base.insert(row32);
            self.overrides.remove(&row32);
            if is_dir {
                self.dir_count_delta -= 1;
            } else {
                self.file_count_delta -= 1;
            }
            changed = true;

            if is_dir {
                self.cascade_remove_base_dir(snap, row32);
            }
        }

        changed
    }

    /// 级联删除基线目录的子树（BFS）。
    /// 已 override 且 parent_row 不再指向被删目录的行 = 已移出，不杀。
    fn cascade_remove_base_dir(&mut self, snap: &Snapshot, dir_row: u32) {
        let dir_id = snap.id_of(dir_row as usize);
        let mut stack: Vec<u32> = snap.children_of(dir_row as usize).to_vec();
        let mut visited: FxHashSet<u32> = FxHashSet::default();

        while let Some(child) = stack.pop() {
            if !visited.insert(child) {
                continue;
            }
            // 已移出该目录的 override 行不杀
            let moved_out = match self.overrides.get(&child) {
                Some(o) => o.parent_row != dir_row as i32,
                None => false,
            };
            if moved_out || self.deleted_base.contains(&child) {
                continue;
            }
            let is_dir = self
                .overrides
                .get(&child)
                .map(|o| o.is_dir)
                .unwrap_or_else(|| snap.is_dir(child as usize));
            self.deleted_base.insert(child);
            self.overrides.remove(&child);
            if is_dir {
                self.dir_count_delta -= 1;
                for &grandchild in snap.children_of(child as usize) {
                    stack.push(grandchild);
                }
            } else {
                self.file_count_delta -= 1;
            }
        }

        // 基线目录的 added 子孙（按 parent_frn 链，tombstone_added 内部递归）
        let added_idx: Vec<usize> = (0..self.added.len())
            .filter(|&i| !self.added_removed[i] && self.added[i].parent_frn == dir_id)
            .collect();
        for i in added_idx {
            self.tombstone_added(i);
        }
    }

    /// 墓碑一条 added 记录并级联其 added 子孙
    fn tombstone_added(&mut self, idx: usize) {
        if self.added_removed[idx] {
            return;
        }
        self.added_removed[idx] = true;
        if self.added[idx].is_dir {
            self.dir_count_delta -= 1;
        } else {
            self.file_count_delta -= 1;
        }
        let child_id = self.added[idx].id;
        let children: Vec<usize> = (0..self.added.len())
            .filter(|&i| !self.added_removed[i] && self.added[i].parent_frn == child_id)
            .collect();
        for i in children {
            self.tombstone_added(i);
        }
    }

    // ── 查询 ─────────────────────────────────────────────────────────────

    /// 基线行当前是否可见（未删、未 override —— override 行经独立通道出结果）
    #[inline]
    pub fn is_base_row_visible(&self, row: u32) -> bool {
        !self.deleted_base.contains(&row) && !self.overrides.contains_key(&row)
    }

    /// 有效文件/目录总数（基线冻结值 + 增量）
    pub fn live_counts(&self, snap: &Snapshot) -> (u64, u64) {
        (
            (snap.total_files() as i64 + self.file_count_delta).max(0) as u64,
            (snap.total_dirs() as i64 + self.dir_count_delta).max(0) as u64,
        )
    }

    /// 遍历活 added 记录（搜索/枚举/compact 共用）
    pub fn live_added(&self) -> impl Iterator<Item = (usize, &AddedRecord)> {
        self.added
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.added_removed[*i])
    }

    /// 遍历全部 override（搜索打分用）
    pub fn overrides_iter(&self) -> impl Iterator<Item = (u32, &OverrideRecord)> {
        self.overrides.iter().map(|(&row, o)| (row, o))
    }

    /// 取一条 override（按基线行号）
    pub fn override_of(&self, row: u32) -> Option<&OverrideRecord> {
        self.overrides.get(&row)
    }

    /// added 行是否存活
    pub fn is_added_live(&self, idx: usize) -> bool {
        self.added_removed.get(idx).copied() == Some(false)
    }

    /// 按下标取活 added 记录（已删返回 None）
    pub fn added_get(&self, idx: usize) -> Option<&AddedRecord> {
        if self.is_added_live(idx) {
            Some(&self.added[idx])
        } else {
            None
        }
    }

    // ── 路径重建（overlay 感知） ──────────────────────────────────────────

    /// 沿父引用链即时重建完整路径（overlay 版）。
    ///
    /// 与 Snapshot::get_full_path 同构，但父/名字可能来自 override 或 added；
    /// override 保留行身份 → 目录改名后子孙路径自动跟随。
    pub fn full_path(&self, snap: &Snapshot, entry: EntryRef) -> String {
        let mut segments: Vec<&str> = Vec::with_capacity(8);
        let mut current = entry;

        for _ in 0..512 {
            let (name, parent): (&str, ParentLink) = match current {
                EntryRef::Base(row) => {
                    let pi = snap.parent_indexes()[row];
                    // 根行（父为自身）：名字即 source_root
                    if pi >= 0 && pi as usize == row {
                        break;
                    }
                    match self.overrides.get(&(row as u32)) {
                        Some(o) => (
                            &o.name,
                            if o.parent_row >= 0 {
                                ParentLink::Base(o.parent_row as usize)
                            } else {
                                ParentLink::Frn(o.parent_frn)
                            },
                        ),
                        None => {
                            if pi < 0 {
                                // 孤儿：沿真实父 FRN 恢复
                                (
                                    snap.name_of(row),
                                    ParentLink::Frn(snap.orphan_parent_frn(row).unwrap_or(0)),
                                )
                            } else {
                                (snap.name_of(row), ParentLink::Base(pi as usize))
                            }
                        }
                    }
                }
                EntryRef::Added(idx) => {
                    let rec = &self.added[idx];
                    (&rec.name, ParentLink::Frn(rec.parent_frn))
                }
            };

            if !name.is_empty() {
                segments.push(name);
            }

            match parent {
                ParentLink::Base(row) => current = EntryRef::Base(row),
                ParentLink::Frn(0) => break,
                ParentLink::Frn(frn) => {
                    // added 行优先（其 id 不在基线 ids 中）
                    if let Some(&idx) = self.added_by_id.get(&frn) {
                        if !self.added_removed[idx as usize] {
                            current = EntryRef::Added(idx as usize);
                            continue;
                        }
                    }
                    match snap.first_row_for_id(frn) {
                        Some(row) if current != EntryRef::Base(row) => {
                            current = EntryRef::Base(row);
                        }
                        _ => break,
                    }
                }
            }
        }

        let mut out = String::with_capacity(
            snap.source_root().len() + segments.iter().map(|s| s.len() + 1).sum::<usize>(),
        );
        out.push_str(snap.source_root());
        for seg in segments.iter().rev() {
            if seg.is_empty() {
                continue;
            }
            if !out.ends_with('\\') {
                out.push('\\');
            }
            out.push_str(seg);
        }
        out
    }

    // ── 折叠（compact） ───────────────────────────────────────────────────

    /// 把 基线行（跳过墓碑、应用 override）+ 活 added 折叠为记录列表。
    ///
    /// 输出可直接喂给 writer::write_snapshot 生成新基线。
    /// parent_id 一律写 FRN：override 的 parent_row 转 id、其余保留基线父
    /// （根为 0、孤儿为真实父 FRN），写入器按 id 重新解析父行 ——
    /// 父是 added 行时天然解析成功（added 行的 id 在 records 中）。
    pub fn compact(snap: &Snapshot, overlay: &DeltaOverlay) -> Vec<IndexRecord> {
        let mut records = Vec::with_capacity(snap.row_count());

        for row in 0..snap.row_count() {
            let row32 = row as u32;
            if overlay.deleted_base.contains(&row32) {
                continue;
            }
            let id = snap.id_of(row);
            let rec = match overlay.overrides.get(&row32) {
                Some(o) => IndexRecord {
                    id,
                    parent_id: if o.parent_row >= 0 {
                        snap.id_of(o.parent_row as usize)
                    } else {
                        o.parent_frn
                    },
                    name: o.name.clone(),
                    is_dir: o.is_dir,
                    hidden: snap.is_hidden(row),
                    system: snap.is_system(row),
                    size: o.size,
                    created: 0,
                    modified: o.modified,
                    accessed: 0,
                },
                None => {
                    let pi = snap.parent_indexes()[row];
                    let parent_id = if pi < 0 {
                        // 孤儿：保留真实父 FRN（可能是 added 行，写入器按 id 解析）
                        snap.orphan_parent_frn(row).unwrap_or(0)
                    } else if pi as usize == row {
                        0 // 根
                    } else {
                        snap.id_of(pi as usize)
                    };
                    IndexRecord {
                        id,
                        parent_id,
                        name: snap.name_of(row).to_string(),
                        is_dir: snap.is_dir(row),
                        hidden: snap.is_hidden(row),
                        system: snap.is_system(row),
                        size: snap.size_of(row),
                        created: 0,
                        modified: snap.last_write_times()[row],
                        accessed: 0,
                    }
                }
            };
            records.push(rec);
        }

        for (_, rec) in overlay.live_added() {
            records.push(IndexRecord {
                id: rec.id,
                parent_id: rec.parent_frn,
                name: rec.name.clone(),
                is_dir: rec.is_dir,
                hidden: false,
                system: false,
                size: rec.size,
                created: 0,
                modified: rec.modified,
                accessed: 0,
            });
        }

        records
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_v2::format::SnapshotMeta;
    use crate::index_v2::search::search_with_overlay;
    use crate::index_v2::writer::write_snapshot;

    fn temp_snapshot(tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir()
            .join(format!("ilauncher_ov_{}_{}.snapshot", tag, std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// 基线树：
    /// C:\Users\
    ///   alice\  (101)  report.txt (102), 简历.txt (103)
    ///   bob\    (104)  notes.txt  (105)
    fn sample_snapshot(tag: &str) -> (std::path::PathBuf, SnapshotMeta) {
        let path = temp_snapshot(tag);
        let records = vec![
            IndexRecord::dir(5, 0, "C:\\"),
            IndexRecord::dir(100, 5, "Users"),
            IndexRecord::dir(101, 100, "alice"),
            IndexRecord::file(102, 101, "report.txt", 512, 1_700_000_000),
            IndexRecord::file(103, 101, "简历.txt", 256, 1_700_000_100),
            IndexRecord::dir(104, 100, "bob"),
            IndexRecord::file(105, 104, "notes.txt", 64, 1_700_000_200),
        ];
        let meta = SnapshotMeta::new("C", "C:\\");
        write_snapshot(&path, records, meta.clone()).unwrap();
        (path, meta)
    }

    /// C2 修复场景：目录 rename 后子孙路径自动跟随
    #[test]
    fn test_dir_rename_children_follow() {
        let (path, _meta) = sample_snapshot("rename");
        let snap = Snapshot::open(&path).unwrap();

        let mut ov = DeltaOverlay::new();
        // alice → alice2（rename = 对同一 id upsert）
        ov.upsert(&snap, 101, 100, "alice2", true, 0, 1_700_000_300);

        // 子孙行未动，但路径经父链自动跟随新名字
        let report = snap.first_row_for_id(102).unwrap();
        assert_eq!(ov.full_path(&snap, EntryRef::Base(report)), "C:\\Users\\alice2\\report.txt");
        let resume = snap.first_row_for_id(103).unwrap();
        assert_eq!(ov.full_path(&snap, EntryRef::Base(resume)), "C:\\Users\\alice2\\简历.txt");

        // 目录自身路径
        let alice = snap.first_row_for_id(101).unwrap();
        assert_eq!(ov.full_path(&snap, EntryRef::Base(alice)), "C:\\Users\\alice2");

        // 不受影响的分支路径不变
        let notes = snap.first_row_for_id(105).unwrap();
        assert_eq!(ov.full_path(&snap, EntryRef::Base(notes)), "C:\\Users\\bob\\notes.txt");

        // 搜索结果路径同样是新路径
        let hits = search_with_overlay(&snap, Some(&ov), "report", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "C:\\Users\\alice2\\report.txt");

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }

    /// 目录删除级联：子文件消失；已 override 移出的文件存活
    #[test]
    fn test_dir_delete_cascade() {
        let (path, _meta) = sample_snapshot("cascade");
        let snap = Snapshot::open(&path).unwrap();

        let mut ov = DeltaOverlay::new();
        // report.txt 从 alice 移到 bob（override 换父）
        ov.upsert(&snap, 102, 104, "report.txt", false, 512, 1_700_000_000);
        // 删除 alice 目录 → 级联杀 简历.txt，但不杀已移出的 report.txt
        assert!(ov.remove(&snap, 101));

        let hits = search_with_overlay(&snap, Some(&ov), "report", 10).unwrap();
        assert_eq!(hits.len(), 1, "移出的 report.txt 应存活");
        assert_eq!(hits[0].path, "C:\\Users\\bob\\report.txt");

        assert!(
            search_with_overlay(&snap, Some(&ov), "简历", 10).unwrap().is_empty(),
            "alice 内的 简历.txt 应被级联删除"
        );

        // 计数：-1 目录、-1 文件（103 被杀；102 活着；105 活着）
        let (files, dirs) = ov.live_counts(&snap);
        assert_eq!(files, 2);
        assert_eq!(dirs, 3);

        // 级联幂等：重复 remove 同目录无变化
        assert!(!ov.remove(&snap, 101));

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }

    /// added 目录级联：删除 added 目录杀掉其 added 子孙
    #[test]
    fn test_added_dir_cascade() {
        let (path, _meta) = sample_snapshot("addedcascade");
        let snap = Snapshot::open(&path).unwrap();

        let mut ov = DeltaOverlay::new();
        // 新建 temp 目录（added），其下新建两个文件（added）
        ov.upsert(&snap, 900, 100, "temp", true, 0, 1_700_000_300);
        ov.upsert(&snap, 901, 900, "a.txt", false, 1, 1_700_000_301);
        ov.upsert(&snap, 902, 900, "b.txt", false, 2, 1_700_000_302);

        let hits = search_with_overlay(&snap, Some(&ov), "a.txt", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "C:\\Users\\temp\\a.txt");

        // 删除 added 目录 → 两个 added 文件级联消失
        assert!(ov.remove(&snap, 900));
        assert!(search_with_overlay(&snap, Some(&ov), "a.txt", 10).unwrap().is_empty());
        assert!(search_with_overlay(&snap, Some(&ov), "b.txt", 10).unwrap().is_empty());

        let (files, dirs) = ov.live_counts(&snap);
        assert_eq!(files, 3, "基线 3 文件全部保留");
        assert_eq!(dirs, 4, "基线 4 目录全部保留");

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }

    /// id 复用：delete 后同 FRN 重建 → added 记录，不复活墓碑
    #[test]
    fn test_id_reuse_after_delete() {
        let (path, _meta) = sample_snapshot("reuse");
        let snap = Snapshot::open(&path).unwrap();

        let mut ov = DeltaOverlay::new();
        // 新建 id 200
        ov.upsert(&snap, 200, 104, "new.txt", false, 10, 1_700_000_400);
        // 删除
        assert!(ov.remove(&snap, 200));
        // 同 FRN 重建（NTFS 序列号变化前可能复用记录号）
        ov.upsert(&snap, 200, 104, "new2.txt", false, 20, 1_700_000_500);

        // 应恰好一条活 added 记录，名字是新名字
        let live: Vec<_> = ov.live_added().collect();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].1.name, "new2.txt");

        let hits = search_with_overlay(&snap, Some(&ov), "new2", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "C:\\Users\\bob\\new2.txt");
        // 墓碑的 old 记录不得出现（"new.txt" 是 "new2.txt" 的子序列，
        // 模糊命中 new2.txt 属正常，但绝不允许 resurrect 出 new.txt 本身）
        let hits = search_with_overlay(&snap, Some(&ov), "new.txt", 10).unwrap();
        assert!(
            hits.iter().all(|h| h.name != "new.txt"),
            "墓碑记录 new.txt 不得复活: {:?}",
            hits.iter().map(|h| &h.name).collect::<Vec<_>>()
        );

        let (files, _dirs) = ov.live_counts(&snap);
        assert_eq!(files, 4, "基线 3 + 复用后的 1 条 added");

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }

    /// upsert 幂等：同参数重复 upsert 不产生重复状态
    #[test]
    fn test_upsert_idempotent() {
        let (path, _meta) = sample_snapshot("idempotent");
        let snap = Snapshot::open(&path).unwrap();

        let mut ov = DeltaOverlay::new();
        for _ in 0..3 {
            ov.upsert(&snap, 102, 101, "report.txt", false, 512, 1_700_000_000);
        }
        assert_eq!(ov.overrides_iter().count(), 1);
        assert_eq!(ov.live_added().count(), 0);

        // 基线行被 override 后再 upsert → 原地更新，不新增 added
        ov.upsert(&snap, 102, 101, "report_final.txt", false, 600, 1_700_000_999);
        assert_eq!(ov.overrides_iter().count(), 1);
        assert_eq!(ov.live_added().count(), 0);
        let report = snap.first_row_for_id(102).unwrap();
        assert_eq!(
            ov.full_path(&snap, EntryRef::Base(report)),
            "C:\\Users\\alice\\report_final.txt"
        );

        // 计数零增量
        let (files, dirs) = ov.live_counts(&snap);
        assert_eq!(files, 3);
        assert_eq!(dirs, 4);

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }

    /// compact 往返：overlay 操作 → compact → 写新快照 → 重开验证
    #[test]
    fn test_compact_roundtrip() {
        let (path, meta) = sample_snapshot("compact");
        let snap = Snapshot::open(&path).unwrap();

        let mut ov = DeltaOverlay::new();
        // alice → alice2
        ov.upsert(&snap, 101, 100, "alice2", true, 0, 1_700_000_300);
        // 新文件进 alice2（added，父 FRN = 101）
        ov.upsert(&snap, 300, 101, "fresh.txt", false, 42, 1_700_000_500);
        // notes.txt 改名
        ov.upsert(&snap, 105, 104, "notes2.txt", false, 64, 1_700_000_200);
        // 删除 简历.txt
        assert!(ov.remove(&snap, 103));

        let records = DeltaOverlay::compact(&snap, &ov);
        let new_path = temp_snapshot("compact_out");
        write_snapshot(&new_path, records, meta).unwrap();
        let snap2 = Snapshot::open(&new_path).unwrap();

        // 行数：基线 7 - 1 删除 + 1 added = 7
        assert_eq!(snap2.row_count(), 7);
        let (files, dirs) = ov.live_counts(&snap);
        assert_eq!(snap2.total_files(), files as u32);
        assert_eq!(snap2.total_dirs(), dirs as u32);

        // 子孙跟随 + added 挂进改名后的目录
        let fresh = snap2.first_row_for_id(300).unwrap();
        assert_eq!(snap2.get_full_path(fresh), "C:\\Users\\alice2\\fresh.txt");
        let report = snap2.first_row_for_id(102).unwrap();
        assert_eq!(snap2.get_full_path(report), "C:\\Users\\alice2\\report.txt");

        // 改名/删除生效
        let notes = snap2.first_row_for_id(105).unwrap();
        assert_eq!(snap2.name_of(notes), "notes2.txt");
        assert!(snap2.first_row_for_id(103).is_none());

        // 新快照上搜索（无 overlay）路径全部正确
        let hits = crate::index_v2::search::search(&snap2, "report", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "C:\\Users\\alice2\\report.txt");

        drop(snap);
        drop(snap2);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&new_path);
    }

    /// override 行换父后 compact：parent_id 解析正确
    #[test]
    fn test_compact_move_to_other_dir() {
        let (path, meta) = sample_snapshot("compactmove");
        let snap = Snapshot::open(&path).unwrap();

        let mut ov = DeltaOverlay::new();
        ov.upsert(&snap, 102, 104, "report.txt", false, 512, 1_700_000_000);

        let records = DeltaOverlay::compact(&snap, &ov);
        let new_path = temp_snapshot("compactmove_out");
        write_snapshot(&new_path, records, meta).unwrap();
        let snap2 = Snapshot::open(&new_path).unwrap();

        let report = snap2.first_row_for_id(102).unwrap();
        assert_eq!(snap2.get_full_path(report), "C:\\Users\\bob\\report.txt");

        drop(snap);
        drop(snap2);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&new_path);
    }
}
