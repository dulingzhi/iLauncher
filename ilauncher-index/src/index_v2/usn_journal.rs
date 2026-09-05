// IndexV2 USN Journal 读取与解析
//
// - UsnEntry：纯数据记录（与 DeviceIoControl 解耦，可单测）
// - parse_usn_buffer：安全解析（逐字段 from_le_bytes，无未对齐裸指针）
// - check_water_level：水位有效性判定（纯函数）
// - Windows-only：open_volume / query_journal / read_all_pending（卷 I/O）
//
// 水位语义（对齐方案文档 Phase 2）：
// - 快照 header 持久化 journal_id + next_usn（compact 时更新）
// - 启动：mmap 旧快照（O(1)）→ 校验水位 → 从 next_usn 批量读 journal replay
// - journal_id 不匹配（journal 被重建）→ 水位失效 → 该盘需全量重建

use anyhow::{bail, Context, Result};

// ── USN reason 位（winnt.h） ────────────────────────────────────────────────
pub const REASON_DATA_OVERWRITE: u32 = 0x0000_0010;
pub const REASON_DATA_EXTEND: u32 = 0x0000_0020;
pub const REASON_DATA_TRUNCATION: u32 = 0x0000_0040;
pub const REASON_BASIC_INFO_CHANGE: u32 = 0x0000_0080;
pub const REASON_FILE_CREATE: u32 = 0x0000_0100;
pub const REASON_FILE_DELETE: u32 = 0x0000_0200;
pub const REASON_RENAME_OLD_NAME: u32 = 0x0000_1000;
pub const REASON_RENAME_NEW_NAME: u32 = 0x0000_2000;

/// catch-up 关心的全部 reason（闭集过滤）
pub const CATCH_UP_REASON_MASK: u32 = REASON_FILE_CREATE
    | REASON_FILE_DELETE
    | REASON_RENAME_OLD_NAME
    | REASON_RENAME_NEW_NAME
    | REASON_BASIC_INFO_CHANGE
    | REASON_DATA_OVERWRITE
    | REASON_DATA_EXTEND
    | REASON_DATA_TRUNCATION;

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

/// 一条解析后的 USN 记录（纯数据，跨平台可测）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsnEntry {
    pub frn: u64,
    pub parent_frn: u64,
    pub usn: i64,
    /// 变更时间（Windows FILETIME，100ns since 1601）
    pub timestamp_filetime: i64,
    pub reason: u32,
    pub file_attributes: u32,
    pub name: String,
}

impl UsnEntry {
    #[inline]
    pub fn is_dir(&self) -> bool {
        (self.file_attributes & FILE_ATTRIBUTE_DIRECTORY) != 0
    }

    /// 变更时间 → unix 秒（索引 LastWriteTimes 列的语义）
    pub fn modified_unix(&self) -> u32 {
        let secs = self.timestamp_filetime / 10_000_000 - 11_644_473_600;
        secs.max(0) as u32
    }
}

/// parse_usn_buffer 的输出
#[derive(Debug, Default)]
pub struct ParsedJournal {
    pub entries: Vec<UsnEntry>,
    /// 下一条待读记录的 USN（作为下一轮 start_usn）
    pub next_start_usn: i64,
}

/// 解析 FSCTL_READ_USN_JOURNAL 输出缓冲：
/// 前 8 字节 = 下一条记录的 USN，之后串行排列变长 USN_RECORD_V2。
/// 纯函数、零 unsafe：逐字段 from_le_bytes（DeviceIoControl 缓冲区不保证对齐，
/// 裸指针强转是未对齐读 UB）。
pub fn parse_usn_buffer(buf: &[u8]) -> Result<ParsedJournal> {
    if buf.len() < 8 {
        bail!("USN journal 缓冲区过小（{} 字节）", buf.len());
    }
    let next_start_usn = i64::from_le_bytes(buf[0..8].try_into()?);
    let mut out = ParsedJournal {
        entries: Vec::new(),
        next_start_usn,
    };

    let mut offset = 8usize;
    while offset + 60 <= buf.len() {
        let rec = &buf[offset..];
        let record_length = u32::from_le_bytes(rec[0..4].try_into()?) as usize;
        if record_length < 60 || offset + record_length > buf.len() {
            bail!("USN 记录长度非法（{} @ offset {}）", record_length, offset);
        }

        let major_version = u16::from_le_bytes(rec[4..6].try_into()?);
        if major_version != 2 {
            bail!("USN 记录主版本 {} 不支持", major_version);
        }

        let frn = u64::from_le_bytes(rec[8..16].try_into()?);
        let parent_frn = u64::from_le_bytes(rec[16..24].try_into()?);
        let usn = i64::from_le_bytes(rec[24..32].try_into()?);
        let timestamp_filetime = i64::from_le_bytes(rec[32..40].try_into()?);
        let reason = u32::from_le_bytes(rec[40..44].try_into()?);
        let file_attributes = u32::from_le_bytes(rec[52..56].try_into()?);
        let file_name_length = u16::from_le_bytes(rec[56..58].try_into()?) as usize;
        let file_name_offset = u16::from_le_bytes(rec[58..60].try_into()?) as usize;

        let name_start = file_name_offset;
        let name_end = name_start + file_name_length;
        if name_end > record_length {
            bail!("USN 记录文件名越界（{}..{} / {}）", name_start, name_end, record_length);
        }
        let name_utf16: Vec<u16> = rec[name_start..name_end]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let name = String::from_utf16_lossy(&name_utf16);

        out.entries.push(UsnEntry {
            frn,
            parent_frn,
            usn,
            timestamp_filetime,
            reason,
            file_attributes,
            name,
        });

        offset += record_length;
    }

    Ok(out)
}

// ── 水位判定（纯函数） ─────────────────────────────────────────────────────

/// 水位有效性
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterLevel {
    /// journal_id 一致且 next_usn 有效：可从 next_usn 继续 replay
    Valid,
    /// 快照无任何水位（如 v3_export 首次生成未携带）：以当前 journal 水位为基线，
    /// 不 replay 历史（扫描完成至今的变更丢失，由下次 compact 前的人工重建兜底的
    /// 设计取舍；生产路径应要求扫描器写入水位）
    NoWaterLevel,
    /// journal 被删除重建（id 变化）：增量链断裂，该盘必须全量重建
    JournalRecreated,
}

pub fn check_water_level(snap_journal_id: u64, snap_next_usn: i64, current_journal_id: u64) -> WaterLevel {
    if snap_journal_id == 0 || snap_next_usn <= 0 {
        WaterLevel::NoWaterLevel
    } else if snap_journal_id != current_journal_id {
        WaterLevel::JournalRecreated
    } else {
        WaterLevel::Valid
    }
}

// ── Windows 卷 I/O ─────────────────────────────────────────────────────────

#[cfg(windows)]
mod sys {
    use super::*;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_READ, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    const FSCTL_QUERY_USN_JOURNAL: u32 = 0x000900F4;
    const FSCTL_READ_USN_JOURNAL: u32 = 0x000900BB;
    const ERROR_HANDLE_EOF: u32 = 38;
    const ERROR_JOURNAL_ENTRY_DELETED: u32 = 1179;

    #[repr(C)]
    #[derive(Default)]
    struct UsnJournalData {
        usn_journal_id: u64,
        first_usn: i64,
        next_usn: i64,
        lowest_valid_usn: i64,
        max_usn: i64,
        maximum_size: u64,
        allocation_delta: u64,
    }

    #[repr(C)]
    struct ReadUsnJournalData {
        start_usn: i64,
        reason_mask: u32,
        return_only_on_close: u32,
        timeout: u64,
        bytes_to_wait: u64,
        usn_journal_id: u64,
    }

    pub fn open_volume(drive: char) -> Result<HANDLE> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let volume_path = format!(r"\\.\{}:", drive);
        let wide: Vec<u16> = OsStr::new(&volume_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            Ok(CreateFileW(
                windows::core::PCWSTR(wide.as_ptr()),
                FILE_GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )?)
        }
    }

    /// (journal_id, next_usn)
    pub fn query_journal(handle: HANDLE) -> Result<(u64, i64)> {
        let mut data = UsnJournalData::default();
        let mut returned: u32 = 0;
        unsafe {
            DeviceIoControl(
                handle,
                FSCTL_QUERY_USN_JOURNAL,
                None,
                0,
                Some(&mut data as *mut _ as *mut std::ffi::c_void),
                std::mem::size_of::<UsnJournalData>() as u32,
                Some(&mut returned),
                None,
            )?;
        }
        Ok((data.usn_journal_id, data.next_usn))
    }

    /// 从 start_usn 非阻塞批量读取全部待定 journal 记录。
    /// 返回 (entries, next_usn)；next_usn 作为新水位持久化。
    pub fn read_all_pending(
        handle: HANDLE,
        journal_id: u64,
        start_usn: i64,
    ) -> Result<(Vec<UsnEntry>, i64)> {
        const BUFFER_SIZE: usize = 4 * 1024 * 1024;
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut out: Vec<UsnEntry> = Vec::new();
        let mut cursor = start_usn;

        loop {
            let input = ReadUsnJournalData {
                start_usn: cursor,
                reason_mask: CATCH_UP_REASON_MASK,
                return_only_on_close: 0,
                timeout: 0,
                bytes_to_wait: 0,
                usn_journal_id: journal_id,
            };
            let mut returned: u32 = 0;

            let result = unsafe {
                DeviceIoControl(
                    handle,
                    FSCTL_READ_USN_JOURNAL,
                    Some(&input as *const _ as *const std::ffi::c_void),
                    std::mem::size_of::<ReadUsnJournalData>() as u32,
                    Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
                    BUFFER_SIZE as u32,
                    Some(&mut returned),
                    None,
                )
            };

            if let Err(e) = result {
                let code = e.code().0 as u32;
                if code == ERROR_HANDLE_EOF {
                    break;
                }
                if code == ERROR_JOURNAL_ENTRY_DELETED {
                    bail!("USN 水位失效：start_usn={} 之前的记录已被 journal 回收", cursor);
                }
                return Err(e).context("FSCTL_READ_USN_JOURNAL 失败");
            }

            if returned <= 8 {
                break;
            }

            let parsed = parse_usn_buffer(&buffer[..returned as usize])?;
            if parsed.entries.is_empty() {
                break;
            }
            cursor = parsed.next_start_usn;
            out.extend(parsed.entries);
        }

        Ok((out, cursor))
    }

    pub fn close(handle: HANDLE) {
        unsafe { let _ = CloseHandle(handle); }
    }
}

#[cfg(windows)]
pub use sys::{close as close_volume, open_volume, query_journal, read_all_pending};

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一条 USN_RECORD_V2 字节流（next_usn 头 + N 条记录）
    fn build_journal_buffer(records: &[(u64, u64, i64, i64, u32, u32, &str)], next_usn: i64) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&next_usn.to_le_bytes());
        for &(frn, parent, usn, ft, reason, attrs, name) in records {
            let name_wide: Vec<u16> = name.encode_utf16().collect();
            let name_bytes = name_wide.len() * 2;
            let record_length = 60 + name_bytes;
            buf.extend_from_slice(&(record_length as u32).to_le_bytes());
            buf.extend_from_slice(&2u16.to_le_bytes()); // major
            buf.extend_from_slice(&0u16.to_le_bytes()); // minor
            buf.extend_from_slice(&frn.to_le_bytes());
            buf.extend_from_slice(&parent.to_le_bytes());
            buf.extend_from_slice(&usn.to_le_bytes());
            buf.extend_from_slice(&ft.to_le_bytes());
            buf.extend_from_slice(&reason.to_le_bytes());
            buf.extend_from_slice(&0u32.to_le_bytes()); // source_info
            buf.extend_from_slice(&0u32.to_le_bytes()); // security_id
            buf.extend_from_slice(&attrs.to_le_bytes());
            buf.extend_from_slice(&(name_bytes as u16).to_le_bytes());
            buf.extend_from_slice(&60u16.to_le_bytes()); // name offset
            for w in name_wide {
                buf.extend_from_slice(&w.to_le_bytes());
            }
        }
        buf
    }

    #[test]
    fn test_parse_usn_buffer_roundtrip() {
        let ft = (1_700_000_000i64 + 11_644_473_600) * 10_000_000;
        let buf = build_journal_buffer(
            &[
                (102, 101, 1000, ft, REASON_FILE_CREATE, 0, "report.txt"),
                (101, 100, 1001, ft, REASON_RENAME_NEW_NAME, 0x10, "alice2"),
            ],
            1002,
        );
        let parsed = parse_usn_buffer(&buf).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.next_start_usn, 1002);

        let e0 = &parsed.entries[0];
        assert_eq!(e0.frn, 102);
        assert_eq!(e0.parent_frn, 101);
        assert_eq!(e0.usn, 1000);
        assert_eq!(e0.name, "report.txt");
        assert!(!e0.is_dir());
        assert_eq!(e0.modified_unix(), 1_700_000_000);

        let e1 = &parsed.entries[1];
        assert!(e1.is_dir());
        assert_eq!(e1.reason, REASON_RENAME_NEW_NAME);
    }

    #[test]
    fn test_parse_rejects_garbage() {
        assert!(parse_usn_buffer(&[0u8; 4]).is_err());
        // 非法 record_length
        let mut buf = 0i64.to_le_bytes().to_vec();
        buf.extend_from_slice(&10u32.to_le_bytes()); // length < 60
        buf.extend_from_slice(&[0u8; 64]);
        assert!(parse_usn_buffer(&buf).is_err());
    }

    #[test]
    fn test_check_water_level() {
        assert_eq!(check_water_level(42, 1000, 42), WaterLevel::Valid);
        assert_eq!(check_water_level(0, 1000, 42), WaterLevel::NoWaterLevel);
        assert_eq!(check_water_level(42, 0, 42), WaterLevel::NoWaterLevel);
        assert_eq!(check_water_level(42, 1000, 43), WaterLevel::JournalRecreated);
    }
}
