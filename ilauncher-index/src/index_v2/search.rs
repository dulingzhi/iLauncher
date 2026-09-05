// IndexV2 搜索 - charmask 预过滤 + fzf 风格模糊打分 + TopN
//
// 流程（对齐 Lertaro NameSearch 的 Phase A/B）：
//   Phase A: 扫 unique 名表 —— AVX2/标量 charmask 预过滤（大部分候选被位图拒绝，
//            零解码零分配）→ 幸存者对 lowercased 名做子序列模糊打分
//   Phase B: 命中的 unique 按分数排序后经 UidRows CSR 扇出到行，
//            沿父链重建路径，产出 limit 条结果
//
// 相对 v2（FST 3-gram 交集按 file_id 取前 N）的改进：
// 结果按相关性分数取 TopN，短查询不再物化海量 bitmap。

use anyhow::Result;
use rayon::prelude::*;
use std::cmp::Ordering as CmpOrdering;

use super::format::required_mask_of;
use super::overlay::{DeltaOverlay, EntryRef};
use super::snapshot::Snapshot;

/// 一条搜索结果
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub row: usize,
    pub score: i64,
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u32,
}

/// fzf 风格的子序列模糊打分（不分配内存）。
///
/// 打分规则（保持简单可解释）：
/// - 每个匹配字符基础分 +1
/// - 边界奖励 +16：词首（pos 0）或前一字符是分隔符（/ \ _ - . 空格）
/// - 连续奖励 +8：与上一个匹配位置相邻
/// - 前缀惩罚：首个匹配位置越靠后减分越多（-1/字符），偏好靠前的命中
///
/// pattern 必须按序是 name 的子序列，否则返回 None。匹配前调用方需把
/// pattern 与 name 都转小写（大小写不敏感）。
pub fn fuzzy_score(pattern_lower: &[char], name_lower: &[char]) -> Option<i64> {
    if pattern_lower.is_empty() || pattern_lower.len() > name_lower.len() {
        return None;
    }

    let mut score = 0i64;
    let mut p_idx = 0usize;
    let mut prev_match: Option<usize> = None;

    for (i, &ch) in name_lower.iter().enumerate() {
        if ch == pattern_lower[p_idx] {
            let mut s = 1i64;
            match prev_match {
                None => {
                    // 边界奖励
                    if i == 0 || is_separator(name_lower[i - 1]) {
                        s += 16;
                    }
                    // 前缀惩罚
                    s -= i as i64;
                }
                Some(prev) => {
                    if i == prev + 1 {
                        s += 8;
                    }
                    if is_separator(name_lower[i - 1]) {
                        s += 16;
                    }
                }
            }
            score += s;
            prev_match = Some(i);
            p_idx += 1;
            if p_idx == pattern_lower.len() {
                // 尾部未匹配字符轻微惩罚，偏好长度接近的词
                score -= (name_lower.len() - i - 1) as i64 / 4;
                return Some(score);
            }
        }
    }
    None
}

#[inline]
fn is_separator(c: char) -> bool {
    matches!(c, '/' | '\\' | '_' | '-' | '.' | ' ')
}

/// 对 unique 名打分的并行 worker 输出
struct UniqueHit {
    uid: u32,
    score: i64,
}

/// 在快照上执行模糊搜索，返回按分数降序的最多 limit 条结果。
///
/// 实现：rayon 并行扫 unique 名表（charmask 预过滤在标量循环内完成，
/// 命中列表合并后排序，再扇出行）。行级分数 = unique 分数 - 行号微扰
/// （保持扇出顺序稳定）。
pub fn search(snapshot: &Snapshot, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    search_with_overlay(snapshot, None, query, limit)
}

/// overlay 感知的搜索：基线 unique 扇出时跳过墓碑/override 行，
/// override 与活 added 记录单独打分后按分数合并进结果。
///
/// 无 overlay 时与 `search` 完全等价（None 分支零开销判断）。
pub fn search_with_overlay(
    snapshot: &Snapshot,
    overlay: Option<&DeltaOverlay>,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let query_lower = query.to_lowercase();
    let pattern: Vec<char> = query_lower.chars().collect();
    let (required_mask, can_filter) = required_mask_of(&query_lower);
    let unique_count = snapshot.unique_count();

    // Phase A：并行扫 unique 名（分块，合并各块命中）
    let chunk_size = 8192.max(unique_count / (rayon::current_num_threads().max(1) * 4));
    let hits: Vec<UniqueHit> = (0..unique_count)
        .into_par_iter()
        .step_by(chunk_size)
        .flat_map(|start| {
            let end = (start + chunk_size).min(unique_count);
            let masks = snapshot.unique_masks();
            let mut local: Vec<UniqueHit> = Vec::new();

            for uid in start..end {
                // charmask 预过滤：名字必须覆盖查询的全部 ASCII 字符
                if can_filter && (masks[uid] & required_mask) != required_mask {
                    continue;
                }

                let name_bytes = snapshot.unique_name_utf8(uid as u32);
                let name_lower = std::str::from_utf8(name_bytes)
                    .unwrap_or("")
                    .to_lowercase();
                let name_chars: Vec<char> = name_lower.chars().collect();

                if let Some(score) = fuzzy_score(&pattern, &name_chars) {
                    local.push(UniqueHit { uid: uid as u32, score });
                }
            }
            local
        })
        .collect();

    // Phase A.5：overlay 候选（override 新名 + 活 added 名）直接打分。
    // 量小（USN 增量规模），顺序执行即可。
    let mut overlay_hits: Vec<(i64, &str, EntryRef)> = Vec::new();
    if let Some(ov) = overlay {
        for (row, o) in ov.overrides_iter() {
            let name_lower = o.name.to_lowercase();
            let name_chars: Vec<char> = name_lower.chars().collect();
            if let Some(score) = fuzzy_score(&pattern, &name_chars) {
                overlay_hits.push((score, o.name.as_str(), EntryRef::Base(row as usize)));
            }
        }
        for (idx, rec) in ov.live_added() {
            let name_lower = rec.name.to_lowercase();
            let name_chars: Vec<char> = name_lower.chars().collect();
            if let Some(score) = fuzzy_score(&pattern, &name_chars) {
                overlay_hits.push((score, rec.name.as_str(), EntryRef::Added(idx)));
            }
        }
    }

    // 合并候选：score 降序，同分按名字、来源升序（确定性）
    let mut overlay_hits = overlay_hits;
    overlay_hits.sort_by(|a, b| match b.0.cmp(&a.0) {
        CmpOrdering::Equal => a.1.cmp(b.1).then(a.2.ord_key().cmp(&b.2.ord_key())),
        other => other,
    });

    let mut results = Vec::with_capacity(limit);

    // Phase B-1：overlay 候选（按分数与基线交错合并：双指针）
    let mut base_i = 0usize;
    let mut ov_i = 0usize;
    let mut hits_sorted = hits;
    hits_sorted.sort_by(|a, b| match b.score.cmp(&a.score) {
        CmpOrdering::Equal => a.uid.cmp(&b.uid),
        other => other,
    });

    let push_overlay_hit =
        |ov: &DeltaOverlay, score: i64, entry: EntryRef, results: &mut Vec<SearchHit>| {
            let (name, is_dir, size, modified) = match entry {
                EntryRef::Base(row) => {
                    let o = ov.override_of(row as u32).expect("overlay hit must be override");
                    (o.name.as_str(), o.is_dir, o.size, o.modified)
                }
                EntryRef::Added(idx) => {
                    let rec = ov.added_get(idx).expect("overlay hit must be live added");
                    (rec.name.as_str(), rec.is_dir, rec.size, rec.modified)
                }
            };
            results.push(SearchHit {
                row: usize::MAX, // overlay 行无基线行号
                score,
                name: name.to_string(),
                path: ov.full_path(snapshot, entry),
                is_dir,
                size,
                modified,
            });
        };

    while results.len() < limit && (base_i < hits_sorted.len() || ov_i < overlay_hits.len()) {
        let take_overlay = if base_i >= hits_sorted.len() {
            true
        } else if ov_i >= overlay_hits.len() {
            false
        } else {
            // 同分时基线优先（与既有行为一致）
            overlay_hits[ov_i].0 > hits_sorted[base_i].score
        };
        if take_overlay {
            let (score, _, entry) = overlay_hits[ov_i];
            ov_i += 1;
            if let Some(ov) = overlay {
                push_overlay_hit(ov, score, entry, &mut results);
            }
        } else {
            let hit = &hits_sorted[base_i];
            base_i += 1;
            for &row in snapshot.rows_for_uid(hit.uid) {
                if results.len() >= limit {
                    break;
                }
                if let Some(ov) = overlay {
                    // 墓碑/override 行经 overlay 通道出结果，跳过
                    if !ov.is_base_row_visible(row) {
                        continue;
                    }
                }
                let row = row as usize;
                results.push(SearchHit {
                    row,
                    score: hit.score,
                    name: snapshot.name_of(row).to_string(),
                    path: overlay
                        .map(|ov| ov.full_path(snapshot, EntryRef::Base(row)))
                        .unwrap_or_else(|| snapshot.get_full_path(row)),
                    is_dir: snapshot.is_dir(row),
                    size: snapshot.size_of(row),
                    modified: snapshot.last_write_times()[row],
                });
            }
        }
    }

    Ok(results)
}

/// 目录枚举：列出某行的全部（直接）子行，按名字升序。
/// 供 "浏览目录" / 目录限定搜索的底座（对齐 Lertaro EnumerateDirectory）。
pub fn enumerate_directory(snapshot: &Snapshot, row: usize, limit: usize) -> Vec<SearchHit> {
    enumerate_directory_with_overlay(snapshot, None, row, limit)
}

/// overlay 感知的目录枚举：基线子行过滤墓碑/移出的 override，
/// 并补充挂在该目录下的活 added 子行。
pub fn enumerate_directory_with_overlay(
    snapshot: &Snapshot,
    overlay: Option<&DeltaOverlay>,
    row: usize,
    limit: usize,
) -> Vec<SearchHit> {
    let dir_id = snapshot.id_of(row);
    let mut results: Vec<SearchHit> = Vec::new();

    for &child in snapshot.children_of(row) {
        if results.len() >= limit {
            break;
        }
        let child = child as usize;
        let hit = match overlay {
            Some(ov) => {
                if !ov.is_base_row_visible(child as u32) {
                    // 墓碑必跳过；override 行看 parent 是否仍指向本目录
                    match ov.override_of(child as u32) {
                        Some(o) if o.parent_row == row as i32 => Some(make_hit(ov, snapshot, EntryRef::Base(child))),
                        _ => None,
                    }
                } else {
                    Some(make_hit(ov, snapshot, EntryRef::Base(child)))
                }
            }
            None => Some(SearchHit {
                row: child,
                score: 0,
                name: snapshot.name_of(child).to_string(),
                path: snapshot.get_full_path(child),
                is_dir: snapshot.is_dir(child),
                size: snapshot.size_of(child),
                modified: snapshot.last_write_times()[child],
            }),
        };
        if let Some(h) = hit {
            results.push(h);
        }
    }

    // added 子行（parent_frn == 本目录 id）
    if let Some(ov) = overlay {
        for (idx, rec) in ov.live_added() {
            if results.len() >= limit {
                break;
            }
            if rec.parent_frn == dir_id {
                results.push(make_hit(ov, snapshot, EntryRef::Added(idx)));
            }
        }
    }

    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    results
}

/// 构造一条 overlay 感知的 SearchHit（row = usize::MAX 仅用于 override/added 行）
fn make_hit(ov: &DeltaOverlay, snapshot: &Snapshot, entry: EntryRef) -> SearchHit {
    match entry {
        EntryRef::Base(row) => match ov.override_of(row as u32) {
            Some(o) => SearchHit {
                row: usize::MAX,
                score: 0,
                name: o.name.clone(),
                path: ov.full_path(snapshot, entry),
                is_dir: o.is_dir,
                size: o.size,
                modified: o.modified,
            },
            None => SearchHit {
                row,
                score: 0,
                name: snapshot.name_of(row).to_string(),
                path: ov.full_path(snapshot, entry),
                is_dir: snapshot.is_dir(row),
                size: snapshot.size_of(row),
                modified: snapshot.last_write_times()[row],
            },
        },
        EntryRef::Added(idx) => {
            let rec = ov.added_get(idx).expect("make_hit(Added) requires a live record");
            SearchHit {
                row: usize::MAX,
                score: 0,
                name: rec.name.clone(),
                path: ov.full_path(snapshot, entry),
                is_dir: rec.is_dir,
                size: rec.size,
                modified: rec.modified,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_v2::format::SnapshotMeta;
    use crate::index_v2::writer::{write_snapshot, IndexRecord};

    fn temp_snapshot(tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir()
            .join(format!("ilauncher_srch_{}_{}.snapshot", tag, std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn sample_snapshot(tag: &str) -> std::path::PathBuf {
        let path = temp_snapshot(tag);
        let records = vec![
            IndexRecord::dir(5, 0, "C:\\"),
            IndexRecord::dir(100, 5, "Users"),
            IndexRecord::dir(101, 100, "alice"),
            IndexRecord::file(102, 101, "report_final.txt", 512, 1_700_000_000),
            IndexRecord::file(103, 101, "resume.docx", 256, 1_700_000_100),
            IndexRecord::file(104, 101, "readme.md", 128, 1_700_000_200),
            IndexRecord::dir(105, 100, "bob"),
            IndexRecord::file(106, 105, "report_draft.txt", 64, 1_700_000_300),
            IndexRecord::file(107, 105, "简历.pdf", 32, 1_700_000_400),
        ];
        write_snapshot(&path, records, SnapshotMeta::new("C", "C:\\")).unwrap();
        path
    }

    #[test]
    fn test_fuzzy_score_rules() {
        let chars = |s: &str| s.chars().collect::<Vec<_>>();

        // 子序列命中：词首奖励 + 连续奖励
        let exact = fuzzy_score(&chars("report"), &chars("report.txt")).unwrap();
        let subseq = fuzzy_score(&chars("rpt"), &chars("report.txt")).unwrap();
        assert!(exact > subseq, "连续命中应优于跳字: {} vs {}", exact, subseq);

        // 非子序列返回 None
        assert!(fuzzy_score(&chars("xyz"), &chars("report.txt")).is_none());
        assert!(fuzzy_score(&chars("report"), &chars("rpt")).is_none());
        assert!(fuzzy_score(&chars(""), &chars("report")).is_none());

        // 前缀惩罚：靠后的命中分数更低
        let early = fuzzy_score(&chars("rep"), &chars("rep_xyz")).unwrap();
        let late = fuzzy_score(&chars("rep"), &chars("xyz_rep")).unwrap();
        assert!(early > late, "前缀命中应优于后缀: {} vs {}", early, late);
    }

    #[test]
    fn test_search_exact_and_fuzzy() {
        let path = sample_snapshot("basic");
        let snap = Snapshot::open(&path).unwrap();

        // 精确子串式命中
        let hits = search(&snap, "report", 10).unwrap();
        assert_eq!(hits.len(), 2, "两个 report 文件都应命中");
        assert!(hits.iter().all(|h| h.name.contains("report")));
        // 分数降序
        for w in hits.windows(2) {
            assert!(w[0].score >= w[1].score);
        }

        // 模糊缩写
        let hits = search(&snap, "rft", 10).unwrap();
        assert!(!hits.is_empty(), "rft 应模糊命中 report_final");
        assert_eq!(hits[0].name, "report_final.txt");

        // 不命中
        assert!(search(&snap, "zzzzz", 10).unwrap().is_empty());

        // 路径正确
        let hits = search(&snap, "resume", 10).unwrap();
        assert_eq!(hits[0].path, "C:\\Users\\alice\\resume.docx");
        assert!(!hits[0].is_dir);

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_search_limit_and_prefilter_safety() {
        let path = sample_snapshot("limit");
        let snap = Snapshot::open(&path).unwrap();

        // limit 生效
        let hits = search(&snap, "report", 1).unwrap();
        assert_eq!(hits.len(), 1);

        // 预过滤安全性：含非 ASCII 字符的查询仍可命中 CJK 文件名
        let hits = search(&snap, "简历", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "简历.pdf");

        // 混合查询（ASCII + CJK）
        let hits = search(&snap, "简历pdf", 10).unwrap();
        assert_eq!(hits.len(), 1, "混合查询应命中 CJK 文件名: {:?}", hits);

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_enumerate_directory() {
        let path = sample_snapshot("enum");
        let snap = Snapshot::open(&path).unwrap();

        let users = snap.first_row_for_id(100).unwrap();
        let children = enumerate_directory(&snap, users, 100);
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].name, "alice");
        assert_eq!(children[1].name, "bob");
        assert!(children.iter().all(|c| c.is_dir));

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }
}


// ── 性能冒烟（release 模式手动跑：cargo test --release --lib -- --ignored bench） ──

#[cfg(test)]
mod bench {
    use super::*;
    use crate::index_v2::format::SnapshotMeta;
    use crate::index_v2::snapshot::Snapshot;
    use crate::index_v2::writer::{write_snapshot, IndexRecord};

    fn big_snapshot(tag: &str, files: usize) -> std::path::PathBuf {
        let path = std::env::temp_dir()
            .join(format!("ilauncher_bench_{}_{}_{}.snapshot", tag, files, std::process::id()));
        if path.exists() {
            return path;
        }
        let mut records = vec![IndexRecord::dir(5, 0, "C:\\")];
        // 模拟 10 个顶层目录 × N 个子目录，每目录放若干文件
        let mut id = 100u64;
        let mut rng: u64 = 0x12345678;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let words = ["report", "analysis", "notes", "summary", "backup", "config", "archive"];
        let exts = ["txt", "docx", "pdf", "md", "xlsx"];
        for top in 0..10 {
            let top_id = id;
            id += 1;
            records.push(IndexRecord::dir(top_id, 5, &format!("dir{:02}", top)));
            for sub in 0..files / 10 / 20 {
                let sub_id = id;
                id += 1;
                records.push(IndexRecord::dir(sub_id, top_id, &format!("sub{:03}", sub)));
                for f in 0..20 {
                    let w = words[(next() % words.len() as u64) as usize];
                    let e = exts[(next() % exts.len() as u64) as usize];
                    records.push(IndexRecord::file(id, sub_id, &format!("{}_{}_{}.{}", w, sub, f, e), 100, 1_700_000_000));
                    id += 1;
                }
            }
        }
        write_snapshot(&path, records, SnapshotMeta::new("C", "C:\\")).unwrap();
        path
    }

    #[test]
    #[ignore]
    fn bench_search_latency() {
        let path = big_snapshot("lat", 500_000);
        let snap = Snapshot::open(&path).unwrap();
        println!("rows={}, uniques={}", snap.row_count(), snap.unique_count());

        for q in ["report", "rpt", "notes", "xyz", "analysis"] {
            let start = std::time::Instant::now();
            let hits = search(&snap, q, 50).unwrap();
            println!("query {:?}: {} hits in {:?}", q, hits.len(), start.elapsed());
        }

        // 打开耗时（冷启动路径）
        let start = std::time::Instant::now();
        let s2 = Snapshot::open(&path).unwrap();
        println!("open: {:?}", start.elapsed());
        drop(s2);
        drop(snap);
        let _ = std::fs::remove_file(&path);
    }
}
