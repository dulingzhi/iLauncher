// MFT 扫描结果 → IndexV2 (v3) 快照导出
//
// 把 StreamingBuilder 阶段 1 产出的 FrnMap（FRN → ParentInfo）转换成
// index_v2 的 IndexRecord 列表并写列式快照。纯数据转换，不碰卷句柄，
// 可单元测试（合成 FrnMap 即可，无需真实磁盘）。
//
// 限制（与方案文档一致）：FSCTL_ENUM_USN_DATA 不提供 size/mtime，
// 导出记录这两项为 0，待 $MFT 自解析扫描器（P3）落地后补。

use anyhow::Result;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::index_v2::{write_snapshot, IndexRecord, SnapshotMeta};

use super::types::FrnMap;

/// 与 StreamingBuilder::should_ignore 相同的排除规则（路径子串）
fn should_ignore(path_lower: &str) -> bool {
    path_lower.contains("$recycle.bin")
        || path_lower.contains("system volume information")
        || path_lower.contains("\\winsxs\\")
        || path_lower.contains("\\temp\\")
}

/// FrnMap → IndexRecord 列表（纯函数）。
///
/// - 根 FRN=5 → 根记录（parent_id = 0，名字即盘符，source_root 用）
/// - BFS 自根向下，排除规则命中整棵子树跳过
/// - 孤儿（BFS 未访问）保留真实 parent_frn → 写入器落 orphan section，
///   读取时惰性恢复
/// - size/mtime 为 0（USN ENUM 不提供，见模块注释）
pub fn frn_map_to_records(frn_map: &FrnMap, drive_letter: char) -> Vec<IndexRecord> {
    let root_frn = 5u64;
    let mut records = Vec::with_capacity(frn_map.len() + 1);
    records.push(IndexRecord::dir(root_frn, 0, &format!("{}:", drive_letter)));

    // parent_frn → children
    let mut children: FxHashMap<u64, Vec<u64>> =
        FxHashMap::with_capacity_and_hasher(frn_map.len() / 2 + 1, Default::default());
    for (frn, info) in frn_map.iter() {
        if *frn == root_frn {
            continue;
        }
        children.entry(info.parent_frn).or_default().push(*frn);
    }

    let mut visited: FxHashSet<u64> =
        FxHashSet::with_capacity_and_hasher(frn_map.len() + 1, Default::default());
    visited.insert(root_frn);

    // BFS：path_lower 用于排除判断（与 v2 流的行为对齐）
    let mut queue: VecDeque<(u64, String)> = VecDeque::new();
    queue.push_back((root_frn, format!("{}:", drive_letter).to_lowercase()));

    while let Some((parent_frn, parent_path_lower)) = queue.pop_front() {
        let Some(child_frns) = children.get(&parent_frn) else {
            continue;
        };
        for &child_frn in child_frns {
            if !visited.insert(child_frn) {
                continue;
            }
            let Some(info) = frn_map.get(&child_frn) else {
                continue;
            };
            let child_path_lower = format!("{}\\{}", parent_path_lower, info.filename.to_lowercase());
            if should_ignore(&child_path_lower) {
                // 整棵子树标记为已访问（不进 records，也不进孤儿兜底）
                let mut stack: Vec<u64> = children
                    .get(&child_frn)
                    .map(|v| v.clone())
                    .unwrap_or_default();
                while let Some(f) = stack.pop() {
                    if visited.contains(&f) {
                        continue;
                    }
                    visited.insert(f);
                    if let Some(grandchildren) = children.get(&f) {
                        stack.extend_from_slice(grandchildren);
                    }
                }
                continue;
            }

            records.push(IndexRecord {
                id: child_frn,
                parent_id: parent_frn,
                name: info.filename.clone(),
                is_dir: info.is_dir,
                hidden: false,
                system: false,
                size: 0,
                created: 0,
                modified: 0,
                accessed: 0,
            });

            if info.is_dir && children.contains_key(&child_frn) {
                queue.push_back((child_frn, child_path_lower));
            }
        }
    }

    // 孤儿：BFS 未访问（父链断在排除目录或未索引节点），保留真实 parent_frn
    for (frn, info) in frn_map.iter() {
        if visited.contains(frn) || *frn == root_frn {
            continue;
        }
        records.push(IndexRecord {
            id: *frn,
            parent_id: info.parent_frn,
            name: info.filename.clone(),
            is_dir: info.is_dir,
            hidden: false,
            system: false,
            size: 0,
            created: 0,
            modified: 0,
            accessed: 0,
        });
    }

    records
}

/// FrnMap → v3 快照文件（`{output_dir}\{drive}.snapshot`，temp + rename 原子替换）
///
/// journal_id / next_usn 写入 header（USN 水位，启动 catch-up 的基准）；
/// 无水位来源时传 (0, 0)，LiveIndex 首次 catch-up 会以当前 journal 为基线。
pub fn write_v3_snapshot(
    frn_map: &FrnMap,
    drive_letter: char,
    output_dir: &str,
    journal_id: u64,
    next_usn: i64,
) -> Result<PathBuf> {
    let records = frn_map_to_records(frn_map, drive_letter);
    let path = Path::new(output_dir).join(format!("{}.snapshot", drive_letter));
    let mut meta = SnapshotMeta::new(
        &drive_letter.to_string(),
        &format!("{}:\\", drive_letter),
    );
    meta.journal_id = journal_id;
    meta.next_usn = next_usn;
    write_snapshot(&path, records, meta)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_v2::snapshot::Snapshot;
    use crate::index_v2::search::search;

    fn pinfo(parent: u64, name: &str, is_dir: bool) -> super::super::types::ParentInfo {
        super::super::types::ParentInfo {
            parent_frn: parent,
            filename: name.to_string(),
            is_dir,
        }
    }

    /// 合成树：
    /// 5 C:\ → 100 Users → 101 alice {102 report.txt, 103 简历.txt}
    ///              └→ 104 bob {105 notes.txt}
    /// 200 stray.dat（父 FRN 999 未索引 → 孤儿）
    /// 300 $RECYCLE.BIN\junk（应被排除）
    /// 301 C:\Temp\scratch（应被排除，子树 302 连带排除）
    fn sample_frn_map() -> FrnMap {
        let mut m = FrnMap::default();
        m.insert(100, pinfo(5, "Users", true));
        m.insert(101, pinfo(100, "alice", true));
        m.insert(102, pinfo(101, "report.txt", false));
        m.insert(103, pinfo(101, "简历.txt", false));
        m.insert(104, pinfo(100, "bob", true));
        m.insert(105, pinfo(104, "notes.txt", false));
        m.insert(200, pinfo(999, "stray.dat", false));
        m.insert(300, pinfo(5, "$RECYCLE.BIN", true));
        m.insert(301, pinfo(5, "Temp", true));
        m.insert(302, pinfo(301, "scratch.tmp", false));
        m
    }

    fn temp_snapshot(tag: &str) -> PathBuf {
        let path = std::env::temp_dir()
            .join(format!("ilauncher_v3_{}_{}.snapshot", tag, std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn test_frn_map_to_records_tree_and_exclusions() {
        let records = frn_map_to_records(&sample_frn_map(), 'C');
        let ids: Vec<u64> = records.iter().map(|r| r.id).collect();

        // 根 + 7 个正常节点（含 Temp 目录本身，与 v2 行为一致：
        // "\temp\" 规则只排除其内部路径）+ 1 孤儿 = 9；
        // $RECYCLE.BIN 整树、Temp 的子树不进
        assert_eq!(records.len(), 9, "records: {:?}", ids);
        assert!(ids.contains(&5), "根记录必须存在");

        // 排除规则
        assert!(!ids.contains(&300), "$RECYCLE.BIN 应被排除");
        assert!(ids.contains(&301), "Temp 目录本身与 v2 一致保留");
        assert!(!ids.contains(&302), "Temp 子树应连带排除");

        // 根 parent_id = 0
        let root = records.iter().find(|r| r.id == 5).unwrap();
        assert_eq!(root.parent_id, 0);
        assert!(root.is_dir);

        // 孤儿保留真实 parent_frn
        let stray = records.iter().find(|r| r.id == 200).unwrap();
        assert_eq!(stray.parent_id, 999);
        assert_eq!(stray.name, "stray.dat");

        // is_dir 传递正确
        let alice = records.iter().find(|r| r.id == 101).unwrap();
        assert!(alice.is_dir);
        let report = records.iter().find(|r| r.id == 102).unwrap();
        assert!(!report.is_dir);
        assert_eq!(report.parent_id, 101);
    }

    #[test]
    fn test_write_v3_snapshot_roundtrip() {
        let path = temp_snapshot("roundtrip");
        let records = frn_map_to_records(&sample_frn_map(), 'C');
        let meta = SnapshotMeta::new("C", "C:\\");
        write_snapshot(&path, records, meta).unwrap();

        let snap = Snapshot::open(&path).unwrap();
        assert_eq!(snap.row_count(), 9);
        assert_eq!(snap.source_root(), "C:\\");

        // 路径沿父链重建
        let report = snap.first_row_for_id(102).unwrap();
        assert_eq!(snap.get_full_path(report), "C:\\Users\\alice\\report.txt");

        // 非 ASCII
        let resume = snap.first_row_for_id(103).unwrap();
        assert_eq!(snap.get_full_path(resume), "C:\\Users\\alice\\简历.txt");

        // 孤儿挂根下不越界
        let stray = snap.first_row_for_id(200).unwrap();
        let p = snap.get_full_path(stray);
        assert!(p.starts_with("C:\\"), "孤儿路径: {}", p);

        // 目录枚举
        let users = snap.first_row_for_id(100).unwrap();
        let child_ids: Vec<u64> = snap
            .children_of(users)
            .iter()
            .map(|&r| snap.id_of(r as usize))
            .collect();
        assert_eq!(child_ids, vec![101, 104]);

        // 搜索可用且排除目录搜不到
        let hits = search(&snap, "notes", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "C:\\Users\\bob\\notes.txt");
        assert!(search(&snap, "scratch", 10).unwrap().is_empty());

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }

    /// 排除目录的子树里即使有节点父链指向排除目录外，也不应漏入
    /// （BFS 不进入排除目录 → 其子不会出现；孤儿兜底只收父链断的节点）
    #[test]
    fn test_excluded_subtree_no_leak_via_orphans() {
        let mut m = sample_frn_map();
        // 父在 $RECYCLE.BIN 里的文件，名字不含排除词
        m.insert(400, pinfo(300, "innocent.txt", false));
        let records = frn_map_to_records(&m, 'C');
        assert!(
            !records.iter().any(|r| r.id == 400),
            "排除目录内的节点不得经孤儿通道漏入"
        );
    }
}
