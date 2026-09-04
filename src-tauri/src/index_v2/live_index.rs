// IndexV2 实时索引 - Snapshot（不可变 mmap）+ DeltaOverlay（内存增量）+ USN 水位
//
// 生命周期（对齐方案文档 Phase 2 "冷启动 = 打开旧快照 + USN catch-up"）：
//   启动: LiveIndex::open(path) —— mmap 旧快照 O(1)，水位来自 header
//   追赶: catch_up_volume(drive) —— 校验水位 → 从 next_usn 批量 replay journal
//   服务: search / enumerate / counts —— 全链路 overlay 感知
//   折叠: compact(path) —— overlay 折进新快照（header 携带新水位），原子替换
//
// replay 语义（USN reason → overlay 操作）：
//   FILE_CREATE / RENAME_NEW_NAME → upsert（rename 保留 FRN 身份 → C2 天然正确）
//   FILE_DELETE → remove（目录级联）
//   RENAME_OLD_NAME（单独出现）→ 忽略（身份保留，等 NEW_NAME 记录）
//   BASIC_INFO_CHANGE / DATA_* → touch_added（仅刷新 added 行 modified；
//     基线行无 size 来源，USN 不提供，不为其建 override）

use anyhow::Result;
use std::path::Path;

use super::overlay::{DeltaOverlay, EntryRef};
use super::search::{self, SearchHit};
use super::snapshot::Snapshot;
use super::usn_journal::{self, UsnEntry};
use super::writer::{write_snapshot, IndexRecord};
use super::format::SnapshotMeta;

/// catch-up 结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatchUpOutcome {
    /// replay 的增量记录（按 usn 升序）
    CaughtUp(Vec<UsnEntry>),
    /// 快照无水位：以当前 journal 水位为基线（未 replay 历史）
    BaselineSet(i64),
    /// journal 被重建，增量链断裂：该盘需全量重建
    RebuildNeeded,
}

pub struct LiveIndex {
    snapshot: Snapshot,
    overlay: DeltaOverlay,
    journal_id: u64,
    next_usn: i64,
}

impl LiveIndex {
    /// 打开（O(1)：mmap + header，水位来自快照 meta）
    pub fn open(path: &Path) -> Result<Self> {
        let snapshot = Snapshot::open(path)?;
        let journal_id = snapshot.journal_id();
        let next_usn = snapshot.next_usn();
        Ok(Self {
            snapshot,
            overlay: DeltaOverlay::new(),
            journal_id,
            next_usn,
        })
    }

    // ── 只读访问 ─────────────────────────────────────────────────────────

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    pub fn overlay(&self) -> &DeltaOverlay {
        &self.overlay
    }
    pub fn journal_id(&self) -> u64 {
        self.journal_id
    }
    pub fn next_usn(&self) -> i64 {
        self.next_usn
    }

    /// 有效文件/目录总数
    pub fn counts(&self) -> (u64, u64) {
        self.overlay.live_counts(&self.snapshot)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        search::search_with_overlay(&self.snapshot, Some(&self.overlay), query, limit)
    }

    pub fn enumerate(&self, row: usize, limit: usize) -> Vec<SearchHit> {
        search::enumerate_directory_with_overlay(&self.snapshot, Some(&self.overlay), row, limit)
    }

    /// 解析 id → 可枚举的行（基线行或活 added 行）
    pub fn entry_for_id(&self, id: u64) -> Option<EntryRef> {
        if let Some(row) = self.snapshot.first_row_for_id(id) {
            if self.overlay.is_base_row_visible(row as u32) {
                return Some(EntryRef::Base(row));
            }
            // override 行仍可枚举（行身份保留）
            if self.overlay.override_of(row as u32).is_some() {
                return Some(EntryRef::Base(row));
            }
            return None;
        }
        // added 行
        let mut found = None;
        for (idx, rec) in self.overlay.live_added() {
            if rec.id == id {
                found = Some(EntryRef::Added(idx));
                break;
            }
        }
        found
    }

    /// overlay 感知的完整路径
    pub fn full_path(&self, entry: EntryRef) -> String {
        self.overlay.full_path(&self.snapshot, entry)
    }

    // ── 增量应用 ─────────────────────────────────────────────────────────

    /// 设置水位（v3_export 全量扫描落盘 / 测试直接注入）
    pub fn set_water_level(&mut self, journal_id: u64, next_usn: i64) {
        self.journal_id = journal_id;
        self.next_usn = next_usn;
    }

    /// replay 一批 USN 记录（按 usn 升序，journal 天然保证）
    pub fn replay_entries(&mut self, entries: &[UsnEntry]) {
        use usn_journal::{
            REASON_BASIC_INFO_CHANGE, REASON_DATA_EXTEND, REASON_DATA_OVERWRITE,
            REASON_DATA_TRUNCATION, REASON_FILE_CREATE, REASON_FILE_DELETE,
            REASON_RENAME_NEW_NAME,
        };

        for e in entries {
            if e.frn == 5 {
                continue; // 卷根本身不入索引
            }
            let reason = e.reason;
            if reason & REASON_FILE_DELETE != 0 {
                self.overlay.remove(&self.snapshot, e.frn);
            } else if reason & (REASON_FILE_CREATE | REASON_RENAME_NEW_NAME) != 0 {
                self.overlay.upsert(
                    &self.snapshot,
                    e.frn,
                    e.parent_frn,
                    &e.name,
                    e.is_dir(),
                    0, // USN 不提供 size（待 $MFT 自解析补）
                    e.modified_unix(),
                );
            } else if reason
                & (REASON_BASIC_INFO_CHANGE
                    | REASON_DATA_OVERWRITE
                    | REASON_DATA_EXTEND
                    | REASON_DATA_TRUNCATION)
                != 0
            {
                self.overlay.touch_added(e.frn, e.modified_unix());
            }
            // RENAME_OLD_NAME 单独出现：忽略（FRN 身份保留，NEW_NAME 会跟上）
        }
    }

    /// 从卷 journal 追赶增量（Windows-only I/O；水位判定与 replay 均可单测）
    #[cfg(windows)]
    pub fn catch_up_volume(&mut self, drive: char) -> Result<CatchUpOutcome> {
        let handle = usn_journal::open_volume(drive)?;
        let result = (|| -> Result<CatchUpOutcome> {
            let (journal_id, current_next) = usn_journal::query_journal(handle)?;
            match usn_journal::check_water_level(self.journal_id, self.next_usn, journal_id) {
                usn_journal::WaterLevel::JournalRecreated => Ok(CatchUpOutcome::RebuildNeeded),
                usn_journal::WaterLevel::NoWaterLevel => {
                    self.journal_id = journal_id;
                    self.next_usn = current_next;
                    Ok(CatchUpOutcome::BaselineSet(current_next))
                }
                usn_journal::WaterLevel::Valid => {
                    let (entries, next_usn) =
                        usn_journal::read_all_pending(handle, journal_id, self.next_usn)?;
                    self.replay_entries(&entries);
                    self.journal_id = journal_id;
                    self.next_usn = next_usn;
                    Ok(CatchUpOutcome::CaughtUp(entries))
                }
            }
        })();
        usn_journal::close_volume(handle);
        result
    }

    // ── 折叠 ─────────────────────────────────────────────────────────────

    /// Snapshot+Overlay 折叠写新快照（temp + rename 原子替换），
    /// header 携带当前水位；成功后 overlay 清空、快照重新 mmap。
    pub fn compact(&mut self, path: &Path) -> Result<()> {
        let records: Vec<IndexRecord> = DeltaOverlay::compact(&self.snapshot, &self.overlay);
        let mut meta = SnapshotMeta::new(self.snapshot.source_key(), self.snapshot.source_root());
        meta.journal_id = self.journal_id;
        meta.next_usn = self.next_usn;
        write_snapshot(path, records, meta)?;
        self.snapshot = Snapshot::open(path)?;
        self.overlay = DeltaOverlay::new();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_v2::usn_journal::{
        REASON_BASIC_INFO_CHANGE, REASON_FILE_CREATE, REASON_FILE_DELETE,
        REASON_RENAME_NEW_NAME, REASON_RENAME_OLD_NAME,
    };

    fn entry(frn: u64, parent: u64, usn: i64, reason: u32, attrs: u32, name: &str) -> UsnEntry {
        UsnEntry {
            frn,
            parent_frn: parent,
            usn,
            timestamp_filetime: (1_700_000_000 + 11_644_473_600) * 10_000_000,
            reason,
            file_attributes: attrs,
            name: name.to_string(),
        }
    }

    const DIR: u32 = 0x10;

    /// 基线：C:\Users\alice\{report.txt, 简历.txt} + bob\notes.txt
    fn baseline(tag: &str) -> (std::path::PathBuf, SnapshotMeta) {
        let path = std::env::temp_dir()
            .join(format!("ilauncher_li_{}_{}.snapshot", tag, std::process::id()));
        let _ = std::fs::remove_file(&path);
        let records = vec![
            IndexRecord::dir(5, 0, "C:\\"),
            IndexRecord::dir(100, 5, "Users"),
            IndexRecord::dir(101, 100, "alice"),
            IndexRecord::file(102, 101, "report.txt", 512, 1_700_000_000),
            IndexRecord::file(103, 101, "简历.txt", 256, 1_700_000_100),
            IndexRecord::dir(104, 100, "bob"),
            IndexRecord::file(105, 104, "notes.txt", 64, 1_700_000_200),
        ];
        let mut meta = SnapshotMeta::new("C", "C:\\");
        meta.journal_id = 7;
        meta.next_usn = 500;
        write_snapshot(&path, records, meta.clone()).unwrap();
        (path, meta)
    }

    #[test]
    fn test_open_replays_catch_up_semantics() {
        let (path, meta) = baseline("replay");
        let mut idx = LiveIndex::open(&path).unwrap();
        assert_eq!(idx.journal_id(), 7);
        assert_eq!(idx.next_usn(), 500);

        let entries = vec![
            entry(300, 100, 501, REASON_FILE_CREATE, 0, "new.txt"),
            entry(101, 100, 502, REASON_RENAME_NEW_NAME, DIR, "alice2"),
            entry(103, 101, 503, REASON_FILE_DELETE, 0, "简历.txt"),
            entry(102, 104, 504, REASON_RENAME_NEW_NAME, 0, "report.txt"),
        ];
        idx.replay_entries(&entries);

        // 新建文件可搜，路径正确
        let hits = idx.search("new.txt", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "C:\\Users\\new.txt");

        // 目录改名子孙跟随
        let hits = idx.search("report", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "C:\\Users\\bob\\report.txt");

        // 删除消失
        assert!(idx.search("简历", 10).unwrap().is_empty());

        // 计数：3 文件 -1 删除 +1 新建 = 3；4 目录不变（根/Users/alice2/bob）
        assert_eq!(idx.counts(), (3, 4));

        // 水位推进后 compact → 重开验证持久化
        idx.set_water_level(7, 505);
        idx.compact(&path).unwrap();
        let idx2 = LiveIndex::open(&path).unwrap();
        assert_eq!(idx2.journal_id(), 7);
        assert_eq!(idx2.next_usn(), 505);
        let hits = idx2.search("new.txt", 10).unwrap();
        assert_eq!(hits[0].path, "C:\\Users\\new.txt");
        assert!(idx2.search("简历", 10).unwrap().is_empty());

        drop(idx);
        drop(idx2);
        let _ = std::fs::remove_file(&path);
        let _ = meta;
    }

    #[test]
    fn test_rename_old_name_alone_is_noop() {
        let (path, _meta) = baseline("oldname");
        let mut idx = LiveIndex::open(&path).unwrap();
        idx.replay_entries(&[entry(102, 101, 501, REASON_RENAME_OLD_NAME, 0, "report.txt")]);

        // 无 NEW_NAME 时身份保留，状态零变化
        assert_eq!(idx.counts(), (3, 4));
        let hits = idx.search("report", 10).unwrap();
        assert_eq!(hits[0].path, "C:\\Users\\alice\\report.txt");

        drop(idx);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_metadata_change_only_touches_added() {
        let (path, _meta) = baseline("meta");
        let mut idx = LiveIndex::open(&path).unwrap();
        let entries = vec![
            entry(300, 104, 501, REASON_FILE_CREATE, 0, "fresh.txt"),
            entry(300, 104, 502, REASON_BASIC_INFO_CHANGE, 0, "fresh.txt"),
            // 基线行的元数据刷新：无 size 来源，不产生 override
            entry(102, 101, 503, REASON_BASIC_INFO_CHANGE, 0, "report.txt"),
        ];
        idx.replay_entries(&entries);

        assert_eq!(idx.overlay().overrides_iter().count(), 0);
        let hits = idx.search("fresh", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].modified, 1_700_000_000);

        drop(idx);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_entry_for_id_and_enumerate() {
        let (path, _meta) = baseline("enum");
        let mut idx = LiveIndex::open(&path).unwrap();
        idx.replay_entries(&[
            entry(300, 100, 501, REASON_FILE_CREATE, DIR, "temp"),
            entry(301, 300, 502, REASON_FILE_CREATE, 0, "inner.txt"),
        ]);

        // added 目录可经 entry_for_id 解析并枚举其子行
        let users = idx.entry_for_id(100).unwrap();
        let children = idx.enumerate(match users {
            EntryRef::Base(r) => r,
            _ => panic!("users must be base"),
        }, 100);
        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"alice") && names.contains(&"bob") && names.contains(&"temp"));

        let temp = idx.entry_for_id(300).expect("added dir resolvable");
        let hits = idx.search("inner", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "C:\\Users\\temp\\inner.txt");
        let _ = temp;

        drop(idx);
        let _ = std::fs::remove_file(&path);
    }
}
