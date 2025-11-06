// Windows USN Journal 扫描器 - 使用原生 Windows API

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use tracing::{info, error, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::IO::DeviceIoControl;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MftFileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: i64,
}

pub struct UsnScanner {
    drive_letter: char,
}

// USN Journal 数据结构
#[repr(C)]
#[derive(Debug, Default)]
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
#[derive(Debug)]
struct CreateUsnJournalData {
    maximum_size: u64,
    allocation_delta: u64,
}

#[repr(C)]
#[derive(Debug)]
struct MftEnumData {
    start_file_reference_number: u64,
    low_usn: i64,
    high_usn: i64,
}

#[repr(C)]
#[derive(Debug)]
#[allow(dead_code)]
struct UsnRecordV2 {
    record_length: u32,
    major_version: u16,
    minor_version: u16,
    file_reference_number: u64,
    parent_file_reference_number: u64,
    usn: i64,
    time_stamp: i64,
    reason: u32,
    source_info: u32,
    security_id: u32,
    file_attributes: u32,
    file_name_length: u16,
    file_name_offset: u16,
    // 后面跟着文件名 (WCHAR)
}

// IOCTL 代码
const FSCTL_QUERY_USN_JOURNAL: u32 = 0x000900f4;
const FSCTL_CREATE_USN_JOURNAL: u32 = 0x000900e7;
const FSCTL_ENUM_USN_DATA: u32 = 0x000900b3;

impl UsnScanner {
    pub fn new(drive_letter: char) -> Self {
        Self { drive_letter }
    }

    /// 检查是否有管理员权限
    pub fn check_admin_rights() -> bool {
        use windows::Win32::UI::Shell::IsUserAnAdmin;
        
        unsafe {
            IsUserAnAdmin().as_bool()
        }
    }

    /// 扫描指定驱动器的所有文件（使用 USN Journal API）
    pub fn scan(&self) -> Result<Vec<MftFileEntry>> {
        info!("🚀 Starting USN Journal scan for drive {}:", self.drive_letter);
        
        // 1. 检查管理员权限
        info!("🔐 Checking administrator privileges...");
        if !Self::check_admin_rights() {
            error!("❌ Requires administrator privileges");
            return Err(anyhow::anyhow!("Administrator privileges required for USN Journal scanning"));
        }
        info!("✓ Running with administrator privileges");
        
        // 2. 打开卷句柄
        info!("💾 Opening volume {}:...", self.drive_letter);
        let volume_handle = self.open_volume()?;
        info!("✓ Volume handle opened successfully");
        
        // 3. 查询 USN Journal 数据
        info!("📖 Querying USN Journal data...");
        let journal_data = match self.query_usn_journal(volume_handle) {
            Ok(data) => {
                info!("✓ USN Journal ID: {:016X}", data.usn_journal_id);
                data
            }
            Err(e) => {
                error!("❌ Failed to query USN Journal: {:#}", e);
                unsafe { CloseHandle(volume_handle); }
                return Err(e);
            }
        };
        
        // 4. 枚举所有文件
        info!("🔍 Enumerating files via USN Journal...");
        let files = match self.enum_usn_data(volume_handle, &journal_data) {
            Ok(f) => f,
            Err(e) => {
                error!("❌ Failed to enumerate USN data: {:#}", e);
                unsafe { CloseHandle(volume_handle); }
                return Err(e);
            }
        };
        
        info!("✓ Scan completed: {} files found", files.len());
        
        // 关闭句柄
        unsafe { CloseHandle(volume_handle); }
        
        Ok(files)
    }
    
    /// 打开卷句柄
    fn open_volume(&self) -> Result<HANDLE> {
        let volume_path = format!("\\\\.\\{}:", self.drive_letter);
        let wide_path: Vec<u16> = OsStr::new(&volume_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        unsafe {
            let handle = CreateFileW(
                windows::core::PCWSTR(wide_path.as_ptr()),
                FILE_GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            )?;
            
            info!("   Volume handle: {:?}", handle);
            Ok(handle)
        }
    }
    
    /// 查询 USN Journal 数据
    fn query_usn_journal(&self, volume_handle: HANDLE) -> Result<UsnJournalData> {
        let mut journal_data = UsnJournalData::default();
        let mut bytes_returned: u32 = 0;
        
        unsafe {
            match DeviceIoControl(
                volume_handle,
                FSCTL_QUERY_USN_JOURNAL,
                None,
                0,
                Some(&mut journal_data as *mut _ as *mut _),
                std::mem::size_of::<UsnJournalData>() as u32,
                Some(&mut bytes_returned),
                None,
            ) {
                Ok(_) => {}
                Err(e) => {
                    error!("❌ FSCTL_QUERY_USN_JOURNAL failed with error: {:?}", e);
                    
                    // 如果USN Journal不存在，尝试创建
                    if e.code().0 as u32 == 0x80070490 { // ERROR_JOURNAL_NOT_ACTIVE
                        info!("   USN Journal not active, attempting to create...");
                        return self.create_usn_journal(volume_handle);
                    }
                    
                    return Err(anyhow::anyhow!("Failed to query USN Journal: {:?}", e));
                }
            }
        }
        
        Ok(journal_data)
    }
    
    /// 创建 USN Journal
    fn create_usn_journal(&self, volume_handle: HANDLE) -> Result<UsnJournalData> {
        let create_data = CreateUsnJournalData {
            maximum_size: 0x800000,  // 8MB
            allocation_delta: 0x100000,  // 1MB
        };
        
        let mut bytes_returned: u32 = 0;
        
        unsafe {
            match DeviceIoControl(
                volume_handle,
                FSCTL_CREATE_USN_JOURNAL,
                Some(&create_data as *const _ as *const _),
                std::mem::size_of::<CreateUsnJournalData>() as u32,
                None,
                0,
                Some(&mut bytes_returned),
                None,
            ) {
                Ok(_) => {
                    info!("✓ USN Journal created successfully");
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to create USN Journal: {:?}", e));
                }
            }
        }
        
        // 重新查询
        self.query_usn_journal(volume_handle)
    }
    
    /// 枚举 USN 数据
    fn enum_usn_data(&self, volume_handle: HANDLE, journal_data: &UsnJournalData) -> Result<Vec<MftFileEntry>> {
        let mut files = Vec::new();
        
        // 设置枚举参数
        let mut enum_data = MftEnumData {
            start_file_reference_number: 0,
            low_usn: 0,
            high_usn: journal_data.next_usn,
        };
        
        const BUFFER_SIZE: usize = 1024 * 1024; // 1MB buffer
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut bytes_returned: u32 = 0;
        
        info!("   Starting enumeration (NextUsn: {})", journal_data.next_usn);
        let mut iteration = 0;
        
        loop {
            iteration += 1;
            
            unsafe {
                match DeviceIoControl(
                    volume_handle,
                    FSCTL_ENUM_USN_DATA,
                    Some(&enum_data as *const _ as *const _),
                    std::mem::size_of::<MftEnumData>() as u32,
                    Some(buffer.as_mut_ptr() as *mut _),
                    BUFFER_SIZE as u32,
                    Some(&mut bytes_returned),
                    None,
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        if e.code().0 as u32 == 38 { // ERROR_HANDLE_EOF
                            info!("   ✓ Reached end of USN data");
                            break;
                        } else {
                            warn!("   DeviceIoControl iteration {} failed: {:?}", iteration, e);
                            break;
                        }
                    }
                }
                
                if bytes_returned == 0 {
                    break;
                }
                
                // 第一个8字节是下一个起始USN
                if bytes_returned < 8 {
                    break;
                }
                
                let next_usn = i64::from_le_bytes(buffer[0..8].try_into().unwrap());
                enum_data.start_file_reference_number = next_usn as u64;
                
                // 解析USN记录
                let mut offset = 8usize;
                while offset + std::mem::size_of::<UsnRecordV2>() <= bytes_returned as usize {
                    let record_ptr = buffer.as_ptr().add(offset) as *const UsnRecordV2;
                    let record = &*record_ptr;
                    
                    if record.record_length == 0 {
                        break;
                    }
                    
                    // 提取文件名
                    let name_offset = offset + record.file_name_offset as usize;
                    let name_length = record.file_name_length as usize;
                    
                    if name_offset + name_length <= bytes_returned as usize {
                        let name_slice = &buffer[name_offset..name_offset + name_length];
                        let name_u16 = std::slice::from_raw_parts(
                            name_slice.as_ptr() as *const u16,
                            name_length / 2,
                        );
                        let name = String::from_utf16_lossy(name_u16);
                        
                        // 检查文件属性
                        let is_dir = (record.file_attributes & FILE_ATTRIBUTE_DIRECTORY.0) != 0;
                        
                        // 跳过系统文件（可选）
                        let is_system = (record.file_attributes & FILE_ATTRIBUTE_SYSTEM.0) != 0;
                        
                        if !is_system {
                            files.push(MftFileEntry {
                                path: String::new(), // USN不直接提供完整路径，需要后续解析
                                name,
                                is_dir,
                                size: 0, // USN_RECORD_V2没有文件大小
                                modified: record.time_stamp,
                            });
                        }
                    }
                    
                    offset += record.record_length as usize;
                }
                
                if iteration % 100 == 0 {
                    info!("   Progress: {} files found (iteration {})", files.len(), iteration);
                }
            }
        }
        
        Ok(files)
    }
}
