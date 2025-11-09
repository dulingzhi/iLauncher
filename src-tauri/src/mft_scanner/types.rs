// MFT 扫描器类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 父目录信息
#[derive(Debug, Clone)]
pub struct ParentInfo {
    /// 父目录的 FRN
    pub parent_frn: u64,
    /// 当前文件/目录名
    pub filename: String,
}

/// FRN 映射表：FRN → {ParentFRN, Filename}
pub type FrnMap = HashMap<u64, ParentInfo>;

/// MFT 文件条目（FTS5 优化版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MftFileEntry {
    /// 完整路径，如 "C:\Users\Documents\file.txt"
    pub path: String,
    /// 优先级（5=.exe, 4=.lnk, 3=.bat, 2=.txt, 1=其他, 0=默认, -1=文件夹）
    pub priority: i32,
    /// ASCII 值总和（保留兼容性，扫描器仍需要）
    pub ascii_sum: i32,
}

impl MftFileEntry {
    /// 提取文件名
    pub fn name(&self) -> String {
        self.path
            .trim_end_matches('\\')
            .split('\\')
            .last()
            .unwrap_or("")
            .to_string()
    }
    
    /// 判断是否是目录
    pub fn is_dir(&self) -> bool {
        self.path.ends_with('\\')
    }
    
    /// 文件大小（FTS5 版本不存储，返回 0）
    pub fn size(&self) -> u64 {
        0
    }
    
    /// 修改时间（FTS5 版本不存储，返回 0）
    pub fn modified(&self) -> i64 {
        0
    }
}

/// 扫描配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// 要扫描的驱动器列表，如 ["C", "D", "E"]
    pub drives: Vec<char>,
    /// 数据库输出目录，如 "D:\\MFTDatabase"
    pub output_dir: String,
    /// 忽略路径列表（小写）
    pub ignore_paths: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        // 自动检测所有 NTFS 驱动器
        let drives = Self::detect_ntfs_drives();
        
        Self {
            drives,
            output_dir: "D:\\MFTDatabase".to_string(),
            ignore_paths: vec![
                "c:\\windows\\winsxs".to_string(),
                "c:\\$recycle.bin".to_string(),
                "appdata\\local\\temp".to_string(),
            ],
        }
    }
}

impl ScanConfig {
    /// 检测所有 NTFS 驱动器
    #[cfg(target_os = "windows")]
    pub fn detect_ntfs_drives() -> Vec<char> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        
        let mut drives = Vec::new();
        
        unsafe {
            use windows::Win32::Storage::FileSystem::{GetLogicalDrives, GetDriveTypeW, GetVolumeInformationW};
            use windows::core::PCWSTR;
            
            // 获取所有逻辑驱动器的位掩码
            let drive_mask = GetLogicalDrives();
            
            for i in 0..26 {
                // 检查该位是否被设置（表示驱动器存在）
                if (drive_mask & (1 << i)) != 0 {
                    let drive_letter = (b'A' + i) as char;
                    let root_path: Vec<u16> = format!("{}:\\", drive_letter)
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    
                    // 检查是否是固定磁盘（不包括光驱、U盘等）
                    // 3 = DRIVE_FIXED
                    let drive_type = GetDriveTypeW(PCWSTR(root_path.as_ptr()));
                    
                    if drive_type == 3 {
                        // 检查文件系统类型
                        let mut fs_name = vec![0u16; 32];
                        let result = GetVolumeInformationW(
                            PCWSTR(root_path.as_ptr()),
                            None,
                            None,
                            None,
                            None,
                            Some(&mut fs_name),
                        );
                        
                        if result.is_ok() {
                            let fs_type = OsString::from_wide(&fs_name)
                                .to_string_lossy()
                                .trim_end_matches('\0')
                                .to_string();
                            
                            // 只添加 NTFS 驱动器
                            if fs_type == "NTFS" {
                                drives.push(drive_letter);
                            }
                        }
                    }
                }
            }
        }
        
        // 如果没有检测到驱动器，至少包含 C 盘
        if drives.is_empty() {
            drives.push('C');
        }
        
        drives
    }
    
    #[cfg(not(target_os = "windows"))]
    pub fn detect_ntfs_drives() -> Vec<char> {
        vec!['C']
    }
    
    /// 从 JSON 文件加载配置
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    /// 保存配置到 JSON 文件
    pub fn save_to_file(&self, path: &str) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    /// 检查路径是否应该被忽略
    pub fn is_ignore(&self, path: &str) -> bool {
        // 过滤包含 $ 的系统路径
        if path.contains('$') {
            return true;
        }
        
        let path_lower = path.to_lowercase();
        self.ignore_paths.iter().any(|pattern| path_lower.contains(pattern))
    }
}

/// USN Journal 数据
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct UsnJournalData {
    pub usn_journal_id: u64,
    pub first_usn: i64,
    pub next_usn: i64,
    pub lowest_valid_usn: i64,
    pub max_usn: i64,
    pub maximum_size: u64,
    pub allocation_delta: u64,
}

/// 创建 USN Journal 数据
#[repr(C)]
#[derive(Debug)]
pub struct CreateUsnJournalData {
    pub maximum_size: u64,
    pub allocation_delta: u64,
}

/// MFT 枚举数据
#[repr(C)]
#[derive(Debug)]
pub struct MftEnumData {
    pub start_file_reference_number: u64,
    pub low_usn: i64,
    pub high_usn: i64,
}

/// 读取 USN Journal 数据
#[repr(C)]
#[derive(Debug)]
pub struct ReadUsnJournalData {
    pub start_usn: i64,
    pub reason_mask: u32,
    pub return_only_on_close: u32,
    pub timeout: u64,
    pub bytes_to_wait_for: u64,
    pub usn_journal_id: u64,
}

/// USN 记录 V2
#[repr(C)]
#[derive(Debug)]
#[allow(dead_code)]
pub struct UsnRecordV2 {
    pub record_length: u32,
    pub major_version: u16,
    pub minor_version: u16,
    pub file_reference_number: u64,
    pub parent_file_reference_number: u64,
    pub usn: i64,
    pub time_stamp: i64,
    pub reason: u32,
    pub source_info: u32,
    pub security_id: u32,
    pub file_attributes: u32,
    pub file_name_length: u16,
    pub file_name_offset: u16,
    // 后面跟着文件名 (WCHAR)
}

// IOCTL 代码
pub const FSCTL_QUERY_USN_JOURNAL: u32 = 0x000900f4;
pub const FSCTL_CREATE_USN_JOURNAL: u32 = 0x000900e7;
pub const FSCTL_ENUM_USN_DATA: u32 = 0x000900b3;
pub const FSCTL_READ_USN_JOURNAL: u32 = 0x000900bb;

// 文件属性常量
pub const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
pub const FILE_ATTRIBUTE_SYSTEM: u32 = 0x00000004;

