// IndexV2 快照读取器 - mmap 零解析打开，typed section 直接切片
//
// 与 Lertaro IndexV2/Snapshot 对齐的核心语义：
// - 打开 O(1)：mmap + 读 header + 计算 section 偏移，常驻内存 = OS 页缓存按需触页
// - 路径沿 ParentIndexes 链即时重建（不物化全路径 → 重命名/移动天然正确）
// - 孤儿行（写入时父未索引）携带真实父 FRN，读取时惰性恢复到活目录行
// - 截断文件（torn write / 崩溃残留）在打开时即报错，而不是查询中途越界

use anyhow::{bail, Context, Result};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

use super::format::*;

pub struct Snapshot {
    mmap: Mmap,
    meta: SnapshotMeta,
    offsets: [u64; SECTION_COUNT],
}

impl Snapshot {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        // FileShare.Delete 语义由 rename 原子替换依赖；memmap2 默认不阻止 rename
        let mmap = unsafe { Mmap::map(&file)? };
        if mmap.len() < 16 {
            bail!("snapshot 文件过小（{} 字节），已损坏", mmap.len());
        }

        let (meta, sections_offset) = read_header(&mmap)?;
        let (offsets, total_len) = section_layout(&meta, sections_offset);

        if (mmap.len() as u64) < total_len {
            bail!(
                "snapshot 文件截断：header 要求 {} 字节，实际 {} 字节",
                total_len,
                mmap.len()
            );
        }

        Ok(Self { mmap, meta, offsets })
    }

    // ── 元信息 ─────────────────────────────────────────────────────────────

    pub fn row_count(&self) -> usize {
        self.meta.row_count as usize
    }
    pub fn unique_count(&self) -> usize {
        self.meta.unique_count as usize
    }
    pub fn total_files(&self) -> u32 {
        self.meta.total_files
    }
    pub fn total_dirs(&self) -> u32 {
        self.meta.total_dirs
    }
    pub fn source_key(&self) -> &str {
        &self.meta.source_key
    }
    pub fn source_root(&self) -> &str {
        &self.meta.source_root
    }
    pub fn journal_id(&self) -> u64 {
        self.meta.journal_id
    }
    pub fn next_usn(&self) -> i64 {
        self.meta.next_usn
    }
    pub fn volume_serial(&self) -> u32 {
        self.meta.volume_serial
    }
    pub fn is_complete(&self) -> bool {
        self.meta.is_complete
    }

    // ── typed section 访问（mmap 页缓存按需触页，零拷贝） ──────────────────

    /// 把某个 section 映射为类型化切片。
    /// 安全性：mmap 基址页对齐 + section 16 字节对齐 ⇒ 任何内置标量类型对齐满足；
    /// 长度在 open() 时已按 header 校验，越界访问不可能发生。
    #[inline]
    fn section<T>(&self, sec: Section, count: usize) -> &[T] {
        let off = self.offsets[sec as usize] as usize;
        let ptr = unsafe { self.mmap.as_ptr().add(off) }.cast::<T>();
        unsafe { std::slice::from_raw_parts(ptr, count) }
    }

    pub fn ids(&self) -> &[u64] {
        self.section(Section::Ids, self.row_count())
    }
    pub fn parent_indexes(&self) -> &[i32] {
        self.section(Section::ParentIndexes, self.row_count())
    }
    pub fn flags(&self) -> &[u16] {
        self.section(Section::Flags, self.row_count())
    }
    pub fn sizes(&self) -> &[u64] {
        self.section(Section::Sizes, self.row_count())
    }
    pub fn last_write_times(&self) -> &[u32] {
        self.section(Section::LastWriteTimes, self.row_count())
    }
    pub fn unique_masks(&self) -> &[u64] {
        self.section(Section::UniqueMasks, self.unique_count())
    }

    #[inline]
    pub fn id_of(&self, row: usize) -> u64 {
        self.ids()[row]
    }
    #[inline]
    pub fn name_id_of(&self, row: usize) -> u32 {
        self.section::<u32>(Section::NameIds, self.row_count())[row]
    }
    #[inline]
    pub fn size_of(&self, row: usize) -> u64 {
        self.sizes()[row]
    }
    #[inline]
    pub fn is_dir(&self, row: usize) -> bool {
        (self.flags()[row] & flags::DIRECTORY) != 0
    }
    #[inline]
    pub fn is_hidden_or_system(&self, row: usize) -> bool {
        (self.flags()[row] & (flags::HIDDEN | flags::SYSTEM)) != 0
    }

    // ── 唯一名访问 ─────────────────────────────────────────────────────────

    /// 唯一名的原始 UTF-8 字节（搜索热路径零解码）
    pub fn unique_name_utf8(&self, uid: u32) -> &[u8] {
        let offsets = self.section::<u32>(Section::NameOffsets, self.unique_count() + 1);
        let start = offsets[uid as usize] as usize;
        let end = offsets[uid as usize + 1] as usize;
        &self.section::<u8>(Section::NameBlob, self.meta.name_blob_len as usize)[start..end]
    }

    pub fn name_of(&self, row: usize) -> &str {
        std::str::from_utf8(self.unique_name_utf8(self.name_id_of(row))).unwrap_or("")
    }

    /// 该唯一名是否是纯 ASCII（搜索可跳过 UTF-8 解码）
    pub fn is_unique_ascii(&self, uid: u32) -> bool {
        let bits = self.section::<u64>(Section::UniqueAsciiBits, (self.unique_count() + 63) / 64);
        (bits[(uid / 64) as usize] & (1u64 << (uid % 64))) != 0
    }

    /// 同名行的 CSR 扇出：所有使用此唯一名的行（升序）
    pub fn rows_for_uid(&self, uid: u32) -> &[u32] {
        let starts = self.section::<u32>(Section::UidStarts, self.unique_count() + 1);
        let base = starts[uid as usize] as usize;
        let len = starts[uid as usize + 1] as usize - base;
        &self.section::<u32>(Section::UidRows, self.row_count())[base..base + len]
    }

    // ── 目录结构 ───────────────────────────────────────────────────────────

    /// 行的已排序子行列表（目录枚举 API 的底座）
    pub fn children_of(&self, row: usize) -> &[u32] {
        let starts = self.section::<u32>(Section::ChildStarts, self.row_count() + 1);
        let base = starts[row] as usize;
        let len = starts[row + 1] as usize - base;
        &self.section::<u32>(Section::Children, self.meta.children_len as usize)[base..base + len]
    }

    /// id 的首行行号（ids 升序，硬链接多行相邻），不存在返回 None
    pub fn first_row_for_id(&self, id: u64) -> Option<usize> {
        let ids = self.ids();
        ids.binary_search(&id).ok().map(|row| {
            // 回到第一个相同 id 的行（硬链接相邻）
            let mut first = row;
            while first > 0 && ids[first - 1] == id {
                first -= 1;
            }
            first
        })
    }

    /// 孤儿行的真实父 FRN（写入时父未被索引），不存在返回 None
    pub fn orphan_parent_frn(&self, row: usize) -> Option<u64> {
        let rows = self.section::<u32>(Section::OrphanRows, self.meta.orphan_count as usize);
        let frns = self.section::<u64>(Section::OrphanFrns, self.meta.orphan_count as usize);
        rows.binary_search(&(row as u32))
            .ok()
            .map(|idx| frns[idx])
    }

    // ── 路径重建 ───────────────────────────────────────────────────────────

    /// 沿父引用链即时重建完整路径。
    ///
    /// 这是 v3 格式相对 v2（物化全路径）的核心正确性改进：
    /// 目录改名/移动只需改一行，子孙路径自动跟随，不存在子树路径失效问题。
    /// 深度上限 512 仅为损坏数据的保险。
    pub fn get_full_path(&self, row: usize) -> String {
        let mut segments: Vec<&str> = Vec::with_capacity(8);
        let mut current = row;

        for _ in 0..512 {
            let parent = self.parent_indexes()[current];

            // 根行（父为自身）：其名字即 source_root，不重复入列
            if parent >= 0 && parent as usize == current {
                break;
            }

            segments.push(self.name_of(current));

            if parent < 0 {
                // 孤儿：沿真实父 FRN 恢复到活目录行（若该目录已被索引）
                match self.orphan_parent_frn(current) {
                    Some(frn) => match self.first_row_for_id(frn) {
                        Some(resolved)
                            if resolved != current && self.is_dir(resolved) =>
                        {
                            current = resolved;
                            continue;
                        }
                        _ => break,
                    },
                    None => break,
                }
            }

            current = parent as usize;
        }

        let mut out = String::with_capacity(
            self.meta.source_root.len() + segments.iter().map(|s| s.len() + 1).sum::<usize>(),
        );
        out.push_str(&self.meta.source_root);
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
}

// ── header 解析 ────────────────────────────────────────────────────────────

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos + n > self.buf.len() {
            bail!("snapshot header 截断");
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into()?))
    }
    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into()?))
    }
    fn len_str(&mut self) -> Result<String> {
        let len = self.u32()? as usize;
        String::from_utf8(self.take(len)?.to_vec()).context("snapshot header 字符串非 UTF-8")
    }
}

fn read_header(buf: &[u8]) -> Result<(SnapshotMeta, u64)> {
    let mut c = Cursor { buf, pos: 0 };

    if c.u64()? != MAGIC {
        bail!("snapshot 魔数不匹配（非 IndexV2 文件）");
    }
    let version = c.u32()?;
    if version != VERSION {
        bail!("snapshot 格式版本不匹配：文件 v{}，程序 v{}", version, VERSION);
    }

    let mut meta = SnapshotMeta::new("", "");
    meta.journal_id = c.u64()?;
    meta.next_usn = c.i64()?;
    meta.volume_serial = c.u32()?;
    meta.row_count = c.u32()?;
    meta.unique_count = c.u32()?;
    meta.name_blob_len = c.u32()?;
    meta.children_len = c.u32()?;
    meta.orphan_count = c.u32()?;
    meta.total_files = c.u32()?;
    meta.total_dirs = c.u32()?;
    meta.is_complete = c.u8()? != 0;
    meta.source_key = c.len_str()?;
    meta.source_root = c.len_str()?;

    Ok((meta, align(c.pos as u64)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_v2::writer::{write_snapshot, IndexRecord};

    fn temp_snapshot(tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir()
            .join(format!("ilauncher_idx2s_{}_{}.snapshot", tag, std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn test_truncated_file_rejected() {
        let path = temp_snapshot("truncated");
        write_snapshot(
            &path,
            vec![IndexRecord::dir(5, 0, "C:\\"), IndexRecord::file(10, 5, "a.txt", 1, 0)],
            SnapshotMeta::new("C", "C:\\"),
        )
        .unwrap();

        // 截断文件应打开失败，而不是查询中途越界
        let full_len = std::fs::metadata(&path).unwrap().len();
        let file = File::options().write(true).open(&path).unwrap();
        file.set_len(full_len - 32).unwrap();
        drop(file);

        assert!(Snapshot::open(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_bad_magic_rejected() {
        let path = temp_snapshot("badmagic");
        std::fs::write(&path, b"NOTASNAPSHOT_FILE_CONTENTS_PADDING________").unwrap();
        assert!(Snapshot::open(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_rename_over_open_snapshot() {
        // Windows 语义验证：rename 原子替换被 mmap 的文件后，
        // 旧 Snapshot 仍服务旧数据（页保持有效），新 open 读到新数据
        let path = temp_snapshot("rename");
        write_snapshot(
            &path,
            vec![IndexRecord::dir(5, 0, "C:\\"), IndexRecord::file(10, 5, "old.txt", 1, 0)],
            SnapshotMeta::new("C", "C:\\"),
        )
        .unwrap();

        let old = Snapshot::open(&path).unwrap();

        write_snapshot(
            &path,
            vec![IndexRecord::dir(5, 0, "C:\\"), IndexRecord::file(10, 5, "new.txt", 1, 0)],
            SnapshotMeta::new("C", "C:\\"),
        )
        .unwrap();

        // 旧映射看到旧数据
        let row = old.first_row_for_id(10).unwrap();
        assert_eq!(old.name_of(row), "old.txt");

        // 新映射看到新数据
        let new = Snapshot::open(&path).unwrap();
        let row = new.first_row_for_id(10).unwrap();
        assert_eq!(new.name_of(row), "new.txt");

        drop(old);
        drop(new);
        let _ = std::fs::remove_file(&path);
    }
}
