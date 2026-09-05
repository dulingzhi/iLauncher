// IndexV2 列式快照格式 - 参考 Lertaro IndexV2/SnapshotFormat
//
// 设计目标：
// - 单一 mmap 文件，列式 sections 即运行时布局，打开 O(1) 无解析
// - 存 parent 行引用而非物化完整路径（重命名/移动天然正确，体积更小）
// - 唯一名字典化：NameIds + UidStarts/UidRows CSR，搜索只扫 unique 名
// - 构建时烘焙 charmask 预过滤位图（UniqueMasks）与 ASCII 位图（UniqueAsciiBits）
// - header 持久化 USN 水位（journal_id / next_usn）与卷序列号，供 Phase 2 冷启动追赶

/// 快照魔数："SLICK2\0\0"（little-endian u64）
pub const MAGIC: u64 = 0x0000_324B_4349_4C53;
/// 格式版本（不兼容变更时递增，读取方校验）
pub const VERSION: u32 = 1;
/// section 基址对齐（保证 u64/16 字节列可按自然对齐直接切片）
pub const SECTION_ALIGNMENT: u64 = 16;

pub mod flags {
    pub const DIRECTORY: u16 = 0x01;
    pub const HIDDEN: u16 = 0x02;
    pub const SYSTEM: u16 = 0x04;
}

/// 快照元信息（header 内容）
#[derive(Debug, Clone)]
pub struct SnapshotMeta {
    /// 驱动器标识，如 "C"、"\\\\server\\share"
    pub source_key: String,
    /// 完整根前缀，如 "C:\\"
    pub source_root: String,
    /// USN journal 实例 ID（水位校验，0 = 未知/不适用）
    pub journal_id: u64,
    /// 已索引到的 USN 水位（下一次增量从这里读起）
    pub next_usn: i64,
    /// 卷序列号（卷格式化后变化，用于失效检测）
    pub volume_serial: u32,
    pub row_count: u32,
    pub unique_count: u32,
    pub name_blob_len: u32,
    pub children_len: u32,
    pub orphan_count: u32,
    pub total_files: u32,
    pub total_dirs: u32,
    pub is_complete: bool,
}

impl SnapshotMeta {
    pub fn new(source_key: &str, source_root: &str) -> Self {
        Self {
            source_key: source_key.to_string(),
            source_root: source_root.to_string(),
            journal_id: 0,
            next_usn: 0,
            volume_serial: 0,
            row_count: 0,
            unique_count: 0,
            name_blob_len: 0,
            children_len: 0,
            orphan_count: 0,
            total_files: 0,
            total_dirs: 0,
            is_complete: false,
        }
    }
}

/// 列式 section 枚举（声明顺序即磁盘布局顺序）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Section {
    /// 每行记录 id（u64，按 id 升序，硬链接同名多行相邻）
    Ids = 0,
    /// 父行索引（i32，-1 = 未解析，见 Orphan* sections）
    ParentIndexes,
    /// 每行的唯一名 id（u32）
    NameIds,
    /// 属性标志（u16，见 flags 模块）
    Flags,
    /// 文件大小（u64，目录为 0）
    Sizes,
    /// 创建时间（u32，unix 秒）
    CreationTimes,
    /// 修改时间（u32，unix 秒）
    LastWriteTimes,
    /// 访问时间（u32，unix 秒）
    LastAccessTimes,
    /// 唯一名偏移 CSR（u32 × unique_count+1）
    NameOffsets,
    /// 唯一名字符串池（UTF-8 字节）
    NameBlob,
    /// 唯一名 -> 行 CSR 起始（u32 × unique_count+1）
    UidStarts,
    /// 按 (uid, row) 排序的行号表（u32 × row_count）
    UidRows,
    /// 子行 CSR 起始（u32 × row_count+1）
    ChildStarts,
    /// 每行的已排序子行列表（u32 × children_len）
    Children,
    /// 孤儿行行号（u32 × orphan_count，升序）
    OrphanRows,
    /// 孤儿行的真实父 FRN（u64 × orphan_count）
    OrphanFrns,
    /// 每唯一名的 charmask 预过滤位图（u64 × unique_count）
    UniqueMasks,
    /// 每唯一名"纯 ASCII"位图（u64 bitmap，ceil(unique/64) 个）
    UniqueAsciiBits,
}

pub const SECTION_COUNT: usize = 18;

/// 由 meta 计算各 section 的 (偏移, 字节长度)，以及文件总长度。
///
/// 偏移从 header 之后开始，每个 section 16 字节对齐；
/// 这是布局的唯一权威定义，写入与读取共用。
pub fn section_layout(meta: &SnapshotMeta, sections_offset: u64) -> ([u64; SECTION_COUNT], u64) {
    let rc = meta.row_count as u64;
    let uc = meta.unique_count as u64;
    let oc = meta.orphan_count as u64;

    let sizes: [u64; SECTION_COUNT] = [
        8 * rc,                                  // Ids
        4 * rc,                                  // ParentIndexes (i32)
        4 * rc,                                  // NameIds
        2 * rc,                                  // Flags
        8 * rc,                                  // Sizes
        4 * rc,                                  // CreationTimes
        4 * rc,                                  // LastWriteTimes
        4 * rc,                                  // LastAccessTimes
        4 * (uc + 1),                            // NameOffsets
        meta.name_blob_len as u64,               // NameBlob
        4 * (uc + 1),                            // UidStarts
        4 * rc,                                  // UidRows
        4 * (rc + 1),                            // ChildStarts
        meta.children_len as u64,                // Children
        4 * oc,                                  // OrphanRows
        8 * oc,                                  // OrphanFrns
        8 * uc,                                  // UniqueMasks
        8 * uc.div_ceil(64),                    // UniqueAsciiBits
    ];

    let mut offsets = [0u64; SECTION_COUNT];
    let mut cursor = sections_offset;
    for (i, size) in sizes.iter().enumerate() {
        cursor = align(cursor);
        offsets[i] = cursor;
        cursor += size;
    }
    (offsets, align(cursor))
}

#[inline]
pub fn align(offset: u64) -> u64 {
    (offset + SECTION_ALIGNMENT - 1) & !(SECTION_ALIGNMENT - 1)
}

/// 计算唯一名的 charmask 预过滤位图（小写 ASCII 字母 + 数字各占一位，
/// 其他字符置 bit63 表示"含有无法预过滤的字符"）。
///
/// 查询侧的使用约定：required mask 只放入 ASCII 查询字符，
/// 判定 `(name_mask & required) == required`；name 的 bit63 永远不会被
/// required，因此含非 ASCII 字符的名字只会放宽不会误杀。
pub fn charmask_of(name_lower: &str) -> u64 {
    let mut mask = 0u64;
    for &b in name_lower.as_bytes() {
        match b {
            b'a'..=b'z' => mask |= 1u64 << (b - b'a'),
            b'0'..=b'9' => mask |= 1u64 << (26 + (b - b'0')),
            _ => mask |= 1u64 << 63,
        }
    }
    mask
}

/// 由查询串计算预过滤 required mask 与是否可用。
/// 非 ASCII 查询字符无法被该位图预过滤，直接跳过；
/// 没有任何 ASCII 字符时返回 can_filter = false（调用方应跳过预过滤）。
pub fn required_mask_of(query_lower: &str) -> (u64, bool) {
    let mut mask = 0u64;
    for &b in query_lower.as_bytes() {
        match b {
            b'a'..=b'z' => mask |= 1u64 << (b - b'a'),
            b'0'..=b'9' => mask |= 1u64 << (26 + (b - b'0')),
            _ => {} // 非 ASCII 字符不参与预过滤
        }
    }
    (mask, mask != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_charmask_prefilter_correct() {
        // 对任意 ASCII 查询，名字命中 ⇒ 名字的 mask 必然覆盖查询 mask
        let names = ["report_final_v2.txt", "notes.md", "简历2024.docx", "a1"];
        let queries = ["report", "final", "v2", "notes", "a1", "2024", "xyz", ""];

        for name in names {
            let name_mask = charmask_of(&name.to_lowercase());
            for q in queries {
                let (required, can_filter) = required_mask_of(&q.to_lowercase());
                if !can_filter {
                    continue;
                }
                if name.to_lowercase().contains(&q.to_lowercase()) {
                    assert_eq!(
                        name_mask & required,
                        required,
                        "命中名字 {:?} 的 mask 必须覆盖查询 {:?} 的 mask",
                        name,
                        q
                    );
                }
            }
        }

        // 非 ASCII 查询字符不进入 required mask（含中文查询不会误杀纯 ASCII 名）
        let (required, can_filter) = required_mask_of("简历");
        assert!(!can_filter);
        assert_eq!(required, 0);

        // 混合查询：ASCII 部分进入 mask
        let (required, can_filter) = required_mask_of("报告report");
        assert!(can_filter);
        assert_eq!(required, charmask_of("report"));
    }

    #[test]
    fn test_section_layout_aligned() {
        let mut meta = SnapshotMeta::new("C", "C:\\");
        meta.row_count = 10;
        meta.unique_count = 5;
        meta.name_blob_len = 100;
        meta.children_len = 9;
        meta.orphan_count = 2;

        let base = 128u64;
        let (offsets, total) = section_layout(&meta, base);
        for &off in &offsets {
            assert_eq!(off % SECTION_ALIGNMENT, 0, "section 必须 16 字节对齐");
        }
        // 严格递增（空 section 允许相等，但此处都非空）
        for w in offsets.windows(2) {
            assert!(w[1] > w[0]);
        }
        assert!(total > offsets[SECTION_COUNT - 1]);
    }
}
