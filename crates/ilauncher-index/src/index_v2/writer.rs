// IndexV2 快照写入器 - 从记录列表构建列式快照文件（temp + rename 原子替换）
//
// 输入为一盘（或一个索引源）的全部记录；写入器完成：
// - 按 id 排序（硬链接同名多行相邻，读取方二分查找依赖此有序性）
// - 唯一名池化 + charmask/ASCII 位图烘焙
// - 父行解析（找不到父 → 孤儿 section，读取方沿 FRN 惰性恢复）
// - 子行 CSR（目录枚举 API 的底座）

use anyhow::{Context, Result};
use rustc_hash::FxHashMap;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use super::format::*;

/// 一条待索引记录
#[derive(Debug, Clone)]
pub struct IndexRecord {
    /// 文件引用号（完整 64 位：序列号 << 48 | 记录号），与 USN journal 一致
    pub id: u64,
    /// 父目录 FRN（根目录为 0；父未被索引时也是 0 → 写入孤儿 section）
    pub parent_id: u64,
    pub name: String,
    pub is_dir: bool,
    pub hidden: bool,
    pub system: bool,
    /// 文件大小（目录为 0）
    pub size: u64,
    /// 创建/修改/访问时间（unix 秒）
    pub created: u32,
    pub modified: u32,
    pub accessed: u32,
}

impl IndexRecord {
    pub fn file(id: u64, parent_id: u64, name: &str, size: u64, modified: u32) -> Self {
        Self {
            id,
            parent_id,
            name: name.to_string(),
            is_dir: false,
            hidden: false,
            system: false,
            size,
            created: 0,
            modified,
            accessed: 0,
        }
    }

    pub fn dir(id: u64, parent_id: u64, name: &str) -> Self {
        Self {
            id,
            parent_id,
            name: name.to_string(),
            is_dir: true,
            hidden: false,
            system: false,
            size: 0,
            created: 0,
            modified: 0,
            accessed: 0,
        }
    }
}

struct Prepared {
    meta: SnapshotMeta,
    ids: Vec<u64>,
    parent_indexes: Vec<i32>,
    name_ids: Vec<u32>,
    flags: Vec<u16>,
    sizes: Vec<u64>,
    creation: Vec<u32>,
    last_write: Vec<u32>,
    last_access: Vec<u32>,
    name_offsets: Vec<u32>,
    name_blob: Vec<u8>,
    uid_starts: Vec<u32>,
    uid_rows: Vec<u32>,
    child_starts: Vec<u32>,
    children: Vec<u32>,
    orphan_rows: Vec<u32>,
    orphan_frns: Vec<u64>,
    unique_masks: Vec<u64>,
    unique_ascii_bits: Vec<u64>,
}

/// 由记录列表构建列式数据（不落盘），供写入器与测试共用
fn prepare(mut records: Vec<IndexRecord>, mut meta: SnapshotMeta) -> Result<Prepared> {
    if records.is_empty() {
        anyhow::bail!("snapshot requires at least one record");
    }

    // 1. 按 id 排序（稳定排序保证同名硬链接行相邻且顺序确定）
    records.sort_by_key(|r| r.id);

    let row_count = records.len();
    let mut ids = Vec::with_capacity(row_count);
    let mut parent_indexes = vec![-1i32; row_count];
    let mut name_ids = vec![0u32; row_count];
    let mut flags = Vec::with_capacity(row_count);
    let mut sizes = Vec::with_capacity(row_count);
    let mut creation = Vec::with_capacity(row_count);
    let mut last_write = Vec::with_capacity(row_count);
    let mut last_access = Vec::with_capacity(row_count);

    // 2. 唯一名池化
    let mut name_to_uid: FxHashMap<&str, u32> = FxHashMap::default();
    let mut names: Vec<&str> = Vec::new();          // uid -> name
    let mut name_offsets = Vec::new();              // uid -> blob offset (+1 收尾)
    let mut name_blob: Vec<u8> = Vec::new();

    // 3. 子行收集：parent_row -> Vec<child_row>
    let mut children_lists: Vec<Vec<u32>> = (0..row_count).map(|_| Vec::new()).collect();

    // id -> row 二分查找表（ids 有序，查询用二分；此处构建一次性 map 加速解析）
    let id_to_row: FxHashMap<u64, u32> = records
        .iter()
        .enumerate()
        .map(|(row, r)| (r.id, row as u32))
        .collect();

    let mut total_files = 0u32;
    let mut total_dirs = 0u32;
    let mut orphan_rows = Vec::new();
    let mut orphan_frns = Vec::new();

    for (row, rec) in records.iter().enumerate() {
        ids.push(rec.id);

        // 唯一名入池
        let uid = *name_to_uid.entry(rec.name.as_str()).or_insert_with(|| {
            names.push(rec.name.as_str());
            names.len() as u32 - 1
        });
        name_ids[row] = uid;

        let mut f = 0u16;
        if rec.is_dir {
            f |= flags::DIRECTORY;
            total_dirs += 1;
        } else {
            total_files += 1;
        }
        if rec.hidden {
            f |= flags::HIDDEN;
        }
        if rec.system {
            f |= flags::SYSTEM;
        }
        flags.push(f);
        sizes.push(rec.size);
        creation.push(rec.created);
        last_write.push(rec.modified);
        last_access.push(rec.accessed);

        // 父行解析
        if rec.parent_id != 0 {
            if let Some(&parent_row) = id_to_row.get(&rec.parent_id) {
                parent_indexes[row] = parent_row as i32;
                children_lists[parent_row as usize].push(row as u32);
            } else {
                orphan_rows.push(row as u32);
                orphan_frns.push(rec.parent_id);
            }
        } else {
            // 根：父为自身，子行收集跳过（根不参与枚举）
            parent_indexes[row] = row as i32;
        }
    }

    // 4. 名字 blob 与偏移
    name_offsets.push(0);
    for name in &names {
        name_blob.extend_from_slice(name.as_bytes());
        name_offsets.push(name_blob.len() as u32);
    }

    // 5. (uid, row) CSR：按 uid 分组的升序行号表
    let unique_count = names.len();
    let mut order: Vec<u32> = (0..row_count as u32).collect();
    order.sort_by_key(|&row| (name_ids[row as usize], row));
    let mut uid_starts = vec![0u32; unique_count + 1];
    for &row in &order {
        uid_starts[name_ids[row as usize] as usize + 1] += 1;
    }
    for i in 0..unique_count {
        uid_starts[i + 1] += uid_starts[i];
    }

    // 6. 子行 CSR（每行子列表按行号升序）
    let mut child_starts = Vec::with_capacity(row_count + 1);
    let mut children = Vec::new();
    child_starts.push(0);
    for list in &mut children_lists {
        list.sort_unstable();
        children.extend_from_slice(list);
        child_starts.push(children.len() as u32);
    }

    // 7. charmask 预过滤位图 + ASCII 位图
    let mut unique_masks = Vec::with_capacity(unique_count);
    let mut unique_ascii_bits = vec![0u64; unique_count.div_ceil(64)];
    for (uid, name) in names.iter().enumerate() {
        unique_masks.push(charmask_of(&name.to_lowercase()));
        if name.is_ascii() {
            unique_ascii_bits[uid / 64] |= 1u64 << (uid % 64);
        }
    }

    meta.row_count = row_count as u32;
    meta.unique_count = unique_count as u32;
    meta.name_blob_len = name_blob.len() as u32;
    meta.children_len = children.len() as u32;
    meta.orphan_count = orphan_rows.len() as u32;
    meta.total_files = total_files;
    meta.total_dirs = total_dirs;
    meta.is_complete = true;

    Ok(Prepared {
        meta,
        ids,
        parent_indexes,
        name_ids,
        flags,
        sizes,
        creation,
        last_write,
        last_access,
        name_offsets,
        name_blob,
        uid_starts,
        uid_rows: order,
        child_starts,
        children,
        orphan_rows,
        orphan_frns,
        unique_masks,
        unique_ascii_bits,
    })
}

fn write_header(w: &mut BufWriter<File>, meta: &SnapshotMeta) -> Result<u64> {
    w.write_all(&MAGIC.to_le_bytes())?;
    w.write_all(&VERSION.to_le_bytes())?;

    w.write_all(&meta.journal_id.to_le_bytes())?;
    w.write_all(&meta.next_usn.to_le_bytes())?;
    w.write_all(&meta.volume_serial.to_le_bytes())?;
    w.write_all(&meta.row_count.to_le_bytes())?;
    w.write_all(&meta.unique_count.to_le_bytes())?;
    w.write_all(&meta.name_blob_len.to_le_bytes())?;
    w.write_all(&meta.children_len.to_le_bytes())?;
    w.write_all(&meta.orphan_count.to_le_bytes())?;
    w.write_all(&meta.total_files.to_le_bytes())?;
    w.write_all(&meta.total_dirs.to_le_bytes())?;
    w.write_all(&[meta.is_complete as u8])?;

    write_len_str(w, &meta.source_key)?;
    write_len_str(w, &meta.source_root)?;

    Ok(align(w.stream_position()?))
}

fn write_len_str(w: &mut BufWriter<File>, s: &str) -> Result<()> {
    w.write_all(&(s.len() as u32).to_le_bytes())?;
    w.write_all(s.as_bytes())?;
    Ok(())
}

macro_rules! write_section {
    ($w:expr, $offsets:expr, $sec:expr, $data:expr) => {{
        let offset = $offsets[$sec as usize];
        if $w.stream_position()? != offset {
            $w.seek(SeekFrom::Start(offset))?;
        }
        for chunk in $data.iter() {
            $w.write_all(&chunk.to_le_bytes())?;
        }
    }};
}

/// 将记录列表写入快照文件（temp + rename 原子替换）
pub fn write_snapshot(path: &Path, records: Vec<IndexRecord>, meta: SnapshotMeta) -> Result<()> {
    let prepared = prepare(records, meta)?;
    let meta = &prepared.meta;

    let tmp_path = path.with_extension("snapshot.tmp");
    let mut w = BufWriter::new(File::create(&tmp_path)?);

    // header（section 基址对齐后由 section_layout 统一计算）
    let header_end = write_header(&mut w, meta)?;
    let (offsets, total_len) = section_layout(meta, header_end);

    // 按 section 写入（write_section 宏内自动 seek 到对齐偏移）
    write_section!(w, offsets, Section::Ids, prepared.ids);
    write_section!(w, offsets, Section::ParentIndexes, prepared.parent_indexes);
    write_section!(w, offsets, Section::NameIds, prepared.name_ids);
    write_section!(w, offsets, Section::Flags, prepared.flags);
    write_section!(w, offsets, Section::Sizes, prepared.sizes);
    write_section!(w, offsets, Section::CreationTimes, prepared.creation);
    write_section!(w, offsets, Section::LastWriteTimes, prepared.last_write);
    write_section!(w, offsets, Section::LastAccessTimes, prepared.last_access);
    write_section!(w, offsets, Section::NameOffsets, prepared.name_offsets);
    // NameBlob：原始字节
    {
        let offset = offsets[Section::NameBlob as usize];
        if w.stream_position()? != offset {
            w.seek(SeekFrom::Start(offset))?;
        }
        w.write_all(&prepared.name_blob)?;
    }
    write_section!(w, offsets, Section::UidStarts, prepared.uid_starts);
    write_section!(w, offsets, Section::UidRows, prepared.uid_rows);
    write_section!(w, offsets, Section::ChildStarts, prepared.child_starts);
    write_section!(w, offsets, Section::Children, prepared.children);
    write_section!(w, offsets, Section::OrphanRows, prepared.orphan_rows);
    write_section!(w, offsets, Section::OrphanFrns, prepared.orphan_frns);
    write_section!(w, offsets, Section::UniqueMasks, prepared.unique_masks);
    write_section!(w, offsets, Section::UniqueAsciiBits, prepared.unique_ascii_bits);

    // 截断到精确长度（复用 tmp 文件时防止尾部垃圾）
    w.flush()?;
    let file = w.get_ref();
    file.set_len(total_len)?;
    drop(w);

    std::fs::rename(&tmp_path, path).with_context(|| format!("原子替换 {:?}", path))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_v2::snapshot::Snapshot;

    fn temp_snapshot(tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir()
            .join(format!("ilauncher_idx2_{}_{}.snapshot", tag, std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// 构造一棵小目录树：
    /// C:\Users\
    ///   alice\ (dir 101, parent 100=Users)
    ///     report.txt  (102)
    ///     简历.txt   (103, 非 ASCII)
    ///   bob\ (dir 104)
    ///     report.txt  (105, 同名文件 → 同一 unique name)
    ///   orphan.dat (106, 父 FRN 999 未索引 → 孤儿)
    fn sample_records() -> Vec<IndexRecord> {
        vec![
            IndexRecord::dir(5, 0, "C:\\"),               // row 0: 根本身（id 5）
            IndexRecord::dir(100, 5, "Users"),
            IndexRecord::dir(101, 100, "alice"),
            IndexRecord::file(102, 101, "report.txt", 512, 1_700_000_000),
            IndexRecord::file(103, 101, "简历.txt", 256, 1_700_000_100),
            IndexRecord::dir(104, 100, "bob"),
            IndexRecord::file(105, 104, "report.txt", 900, 1_700_000_200),
            IndexRecord {
                id: 106,
                parent_id: 999, // 未索引的父 → 孤儿
                name: "orphan.dat".into(),
                is_dir: false,
                hidden: false,
                system: false,
                size: 1,
                created: 0,
                modified: 0,
                accessed: 0,
            },
        ]
    }

    #[test]
    fn test_write_open_roundtrip() {
        let path = temp_snapshot("roundtrip");
        let mut meta = SnapshotMeta::new("C", "C:\\");
        meta.journal_id = 42;
        meta.next_usn = 12345;
        meta.volume_serial = 0xDEADBEEF;

        write_snapshot(&path, sample_records(), meta).unwrap();
        let snap = Snapshot::open(&path).unwrap();

        assert_eq!(snap.row_count(), 8);
        assert_eq!(snap.total_files(), 4);
        assert_eq!(snap.total_dirs(), 4);
        assert_eq!(snap.journal_id(), 42);
        assert_eq!(snap.next_usn(), 12345);
        assert_eq!(snap.volume_serial(), 0xDEADBEEF);
        assert!(snap.is_complete());

        // 路径重建（父引用链）
        let row = snap.first_row_for_id(102).unwrap();
        assert_eq!(snap.get_full_path(row), "C:\\Users\\alice\\report.txt");

        // 非 ASCII 名
        let row = snap.first_row_for_id(103).unwrap();
        assert_eq!(snap.get_full_path(row), "C:\\Users\\alice\\简历.txt");

        // 同名文件共用 unique name，但行不同、路径不同
        let r1 = snap.first_row_for_id(102).unwrap();
        let r2 = snap.first_row_for_id(105).unwrap();
        assert_ne!(r1, r2);
        let uid1 = snap.name_id_of(r1);
        assert_eq!(uid1, snap.name_id_of(r2), "同名必须池化为同一 unique name");
        assert_eq!(snap.rows_for_uid(uid1).len(), 2);

        // 孤儿：父 FRN 未索引 → 路径挂在根下但不被串到错误父
        let row = snap.first_row_for_id(106).unwrap();
        let path_str = snap.get_full_path(row);
        assert!(path_str.starts_with("C:\\"), "孤儿路径应挂在根下: {}", path_str);

        // 子行 CSR：Users 的直接子为 alice + bob（按行号升序）
        let users = snap.first_row_for_id(100).unwrap();
        let child_ids: Vec<u64> = snap
            .children_of(users)
            .iter()
            .map(|&r| snap.id_of(r as usize))
            .collect();
        assert_eq!(child_ids, vec![101, 104]);

        // 元数据列
        let row = snap.first_row_for_id(105).unwrap();
        assert_eq!(snap.size_of(row), 900);
        assert!(!snap.is_dir(row));
        let users = snap.first_row_for_id(100).unwrap();
        assert!(snap.is_dir(users));

        drop(snap);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_atomic_replace() {
        let path = temp_snapshot("replace");
        write_snapshot(
            &path,
            vec![IndexRecord::dir(5, 0, "C:\\"), IndexRecord::file(10, 5, "a.txt", 1, 0)],
            SnapshotMeta::new("C", "C:\\"),
        )
        .unwrap();

        // 第二次写入（不同内容）应原子替换，旧映射失效、新文件可读
        write_snapshot(
            &path,
            vec![
                IndexRecord::dir(5, 0, "C:\\"),
                IndexRecord::file(10, 5, "b.txt", 2, 0),
                IndexRecord::file(11, 5, "c.txt", 3, 0),
            ],
            SnapshotMeta::new("C", "C:\\"),
        )
        .unwrap();

        let snap = Snapshot::open(&path).unwrap();
        assert_eq!(snap.row_count(), 3);
        let row = snap.first_row_for_id(11).unwrap();
        assert_eq!(snap.name_of(row), "c.txt");
        drop(snap);
        let _ = std::fs::remove_file(&path);
    }
}
