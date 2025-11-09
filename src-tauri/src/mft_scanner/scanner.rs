// Windows USN Journal 扫描器 - 完整路径重建版本

use anyhow::{Result, Context};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use tracing::{info, error};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::IO::DeviceIoControl;

use crate::mft_scanner::types::*;
use crate::mft_scanner::database::Database;

pub struct UsnScanner {
    drive_letter: char,
    frn_map: FrnMap,  // 🔹 关键：FRN 映射表
}

impl UsnScanner {
    pub fn new(drive_letter: char) -> Self {
        Self {
            drive_letter,
            frn_map: FrnMap::new(),
        }
    }
    
    /// 检查是否有管理员权限
    pub fn check_admin_rights() -> bool {
        use windows::Win32::UI::Shell::IsUserAnAdmin;
        unsafe { IsUserAnAdmin().as_bool() }
    }
    
    /// 扫描并保存到数据库
    pub fn scan_to_database(&mut self, output_dir: &str, config: &ScanConfig) -> Result<()> {
        info!("🚀 Starting scan for drive {}:", self.drive_letter);
        
        // 1. 检查管理员权限
        if !Self::check_admin_rights() {
            error!("❌ Requires administrator privileges");
            return Err(anyhow::anyhow!("Administrator privileges required"));
        }
        info!("✓ Running with administrator privileges");
        
        // 2. 打开卷句柄
        info!("💾 Opening volume {}:...", self.drive_letter);
        let volume_handle = self.open_volume()?;
        info!("✓ Volume handle opened");
        
        // 3. 查询 USN Journal
        info!("📖 Querying USN Journal...");
        let journal_data = self.query_usn_journal(volume_handle)?;
        info!("✓ USN Journal ID: {:016X}", journal_data.usn_journal_id);
        
        // 4. 🔹 第一阶段：构建 FRN 映射表
        info!("🔍 Building FRN map (Phase 1)...");
        self.build_frn_map(volume_handle, &journal_data)?;
        info!("✓ FRN map built: {} entries", self.frn_map.len());
        
        // 5. 🔹 第二阶段：重建完整路径并保存
        info!("📝 Rebuilding paths and saving to database (Phase 2)...");
        let mut db = Database::create_for_write(self.drive_letter, output_dir)?;
        
        let mut entries = Vec::new();
        let mut count = 0;
        const BATCH_SIZE: usize = 100_000;  // 每 10 万条提交一次
        
        for (frn, parent_info) in &self.frn_map {
            // 🔹 递归查询完整路径
            match self.get_path(*frn) {
                Ok(full_path) => {
                    // 过滤忽略路径
                    if config.is_ignore(&full_path) {
                        continue;
                    }
                    
                    let ascii_sum = Database::calc_ascii_sum(&parent_info.filename);
                    
                    entries.push(MftFileEntry {
                        path: full_path,
                        ascii_sum,
                        priority: 0,  // TODO: 从配置读取
                    });
                    
                    count += 1;
                    
                    // 🔥 优化：增大批次，减少写入次数
                    // 批量提交 (每 10000 条记录)
                    if entries.len() >= BATCH_SIZE {
                        db.insert_batch(&entries)?;
                        info!("   Progress: {} files saved", count);
                        entries.clear();
                    }
                }
                Err(e) => {
                    // 路径重建失败，跳过
                    continue;
                }
            }
        }
        
        // 保存剩余记录
        if !entries.is_empty() {
            db.insert_batch(&entries)?;
        }
        
        info!("✅ Scan completed: {} files saved to database", count);
        
        unsafe { CloseHandle(volume_handle); }
        Ok(())
    }
    
    /// 🔹 第一阶段：构建 FRN 映射表
    fn build_frn_map(&mut self, volume_handle: HANDLE, journal_data: &UsnJournalData) -> Result<()> {
        let mut enum_data = MftEnumData {
            start_file_reference_number: 0,
            low_usn: 0,
            high_usn: journal_data.next_usn,
        };
        
        const BUFFER_SIZE: usize = 1024 * 1024;  // 1MB buffer
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut bytes_returned: u32 = 0;
        let mut iteration = 0;
        
        loop {
            iteration += 1;
            
            unsafe {
                let result = DeviceIoControl(
                    volume_handle,
                    FSCTL_ENUM_USN_DATA,
                    Some(&enum_data as *const _ as *const _),
                    std::mem::size_of::<MftEnumData>() as u32,
                    Some(buffer.as_mut_ptr() as *mut _),
                    BUFFER_SIZE as u32,
                    Some(&mut bytes_returned),
                    None,
                );
                
                if result.is_err() {
                    let error = GetLastError();
                    if error.0 == 38 {  // ERROR_HANDLE_EOF
                        break;
                    } else {
                        return Err(anyhow::anyhow!("DeviceIoControl failed: {:?}", error));
                    }
                }
                
                if bytes_returned < 8 {
                    break;
                }
                
                // 更新下一个起始位置
                let next_usn = i64::from_le_bytes(buffer[0..8].try_into().unwrap());
                enum_data.start_file_reference_number = next_usn as u64;
                
                // 解析 USN 记录并建立映射
                let mut offset = 8usize;
                while offset + std::mem::size_of::<UsnRecordV2>() <= bytes_returned as usize {
                    let record_ptr = buffer.as_ptr().add(offset) as *const UsnRecordV2;
                    let record = &*record_ptr;
                    
                    if record.record_length == 0 {
                        break;
                    }
                    
                    // 🔹 提取文件名
                    let name = self.extract_filename(record);
                    
                    // 🔹 建立映射：FRN → {ParentFRN, Filename}
                    self.frn_map.insert(
                        record.file_reference_number,
                        ParentInfo {
                            parent_frn: record.parent_file_reference_number,
                            filename: name,
                        },
                    );
                    
                    offset += record.record_length as usize;
                }
                
                if iteration % 100 == 0 {
                    info!("   Building FRN map: {} entries (iteration {})", 
                          self.frn_map.len(), iteration);
                }
            }
        }
        
        Ok(())
    }
    
    /// 🔹 第二阶段：递归查询重建完整路径
    fn get_path(&self, frn: u64) -> Result<String> {
        let mut path_parts = Vec::new();
        let mut current_frn = frn;
        let mut depth = 0;
        const MAX_DEPTH: usize = 100;  // 防止无限循环
        
        loop {
            depth += 1;
            if depth > MAX_DEPTH {
                return Err(anyhow::anyhow!("Path too deep"));
            }
            
            match self.frn_map.get(&current_frn) {
                Some(info) => {
                    path_parts.push(info.filename.clone());
                    current_frn = info.parent_frn;
                }
                None => {
                    // 到达根目录
                    break;
                }
            }
        }
        
        // 反转路径（从根到叶）
        path_parts.reverse();
        
        // 拼接完整路径
        let path = if path_parts.is_empty() {
            format!("{}:\\", self.drive_letter)
        } else {
            format!("{}:\\{}", self.drive_letter, path_parts.join("\\"))
        };
        
        Ok(path)
    }
    
    /// 提取文件名（UTF-16 转 String）
    fn extract_filename(&self, record: &UsnRecordV2) -> String {
        unsafe {
            let name_ptr = (record as *const UsnRecordV2 as *const u8)
                .add(record.file_name_offset as usize) as *const u16;
            let name_len = record.file_name_length as usize / 2;
            let name_slice = std::slice::from_raw_parts(name_ptr, name_len);
            String::from_utf16_lossy(name_slice)
        }
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
            
            Ok(handle)
        }
    }
    
    /// 查询 USN Journal
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
                Ok(_) => {},
                Err(e) => {
                    // 如果不存在，尝试创建
                    self.create_usn_journal(volume_handle)?;
                    return self.query_usn_journal(volume_handle);
                }
            }
        }
        
        Ok(journal_data)
    }
    
    /// 创建 USN Journal
    fn create_usn_journal(&self, volume_handle: HANDLE) -> Result<()> {
        let create_data = CreateUsnJournalData {
            maximum_size: 0x800000,      // 8MB
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
                Ok(_) => {},
                Err(e) => return Err(anyhow::anyhow!("Failed to create USN Journal: {:?}", e)),
            }
        }
        
        info!("✓ USN Journal created");
        Ok(())
    }
}
