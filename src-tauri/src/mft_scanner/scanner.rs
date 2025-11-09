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
            frn_map: FrnMap::default(),  // 🔥 使用 FxHashMap
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
        
        // 4. 🔹 新策略：流式扫描 + 即时写入 (避免 10GB 内存占用)
        info!("🔍 Streaming scan with immediate database write...");
        self.stream_scan_to_database(volume_handle, &journal_data, output_dir, config)?;
        
        info!("✓ Scan completed");
        
        unsafe { let _ = CloseHandle(volume_handle); }
        Ok(())
    }
    
    /// 🔹 流式扫描：边扫描边写入，避免内存爆炸
    fn stream_scan_to_database(
        &mut self,
        volume_handle: HANDLE,
        journal_data: &UsnJournalData,
        output_dir: &str,
        config: &ScanConfig,
    ) -> Result<()> {
        // 🔥 阶段 1：只构建 FRN Map（不重建路径）
        info!("📍 Phase 1: Building FRN map (minimal memory)...");
        self.build_frn_map_minimal(volume_handle, journal_data)?;
        info!("✓ FRN map built: {} entries", self.frn_map.len());
        
        // 🔥 阶段 2：流式重建路径 + 批量写入数据库
        info!("📝 Phase 2: Streaming path reconstruction and database write...");
        let mut db = Database::create_for_write(self.drive_letter, output_dir)?;
        
        const BATCH_SIZE: usize = 5_000;  // 🔥 5千条批量 (降低内存)
        let mut entries = Vec::with_capacity(BATCH_SIZE);
        let mut total_count = 0;
        
        // 🔥 关键优化：重用 String buffer 和 path_parts 数组
        let mut path_buffer = String::with_capacity(512);
        let mut path_parts: Vec<&str> = Vec::with_capacity(50);
        
        // 🔥 核心优化：使用引用迭代，避免 collect
        // HashMap 的 keys() 返回引用，我们在内部循环中复制单个 u64
        for frn_ref in self.frn_map.keys() {
            let frn = *frn_ref;  // 只复制一个 u64 (8 bytes)
            
            if let Some(parent_info) = self.frn_map.get(&frn) {
                // 🔹 重建路径（重用 buffer）
                path_parts.clear();
                path_buffer.clear();
                
                if let Ok(()) = self.get_path_reuse(frn, &mut path_parts, &mut path_buffer) {
                    if !config.is_ignore(&path_buffer) {
                        let ascii_sum = Database::calc_ascii_sum(&parent_info.filename);
                        
                        entries.push(MftFileEntry {
                            path: path_buffer.clone(),  // 只在这里克隆一次
                            ascii_sum,
                            priority: 0,
                        });
                        
                        total_count += 1;
                        
                        // 🔥 批量写入
                        if entries.len() >= BATCH_SIZE {
                            db.insert_batch(&entries)?;
                            entries.clear();
                            entries.shrink_to(BATCH_SIZE);  // 释放多余容量
                            
                            if total_count % 50_000 == 0 {
                                info!("   Progress: {} files saved", total_count);
                            }
                        }
                    }
                }
            }
        }
        
        // 保存剩余记录
        if !entries.is_empty() {
            db.insert_batch(&entries)?;
        }
        
        // 🔥 释放 FRN map 内存
        self.frn_map.clear();
        self.frn_map.shrink_to_fit();
        
        info!("✅ Stream scan completed: {} files saved", total_count);
        
        Ok(())
    }
    
    /// 🔥 最小化内存：只构建 FRN Map，不重建路径
    fn build_frn_map_minimal(&mut self, volume_handle: HANDLE, journal_data: &UsnJournalData) -> Result<()> {
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
                
                // 🔥 只解析并建立映射，不重建路径
                let mut offset = 8usize;
                while offset + std::mem::size_of::<UsnRecordV2>() <= bytes_returned as usize {
                    let record_ptr = buffer.as_ptr().add(offset) as *const UsnRecordV2;
                    let record = &*record_ptr;
                    
                    if record.record_length == 0 {
                        break;
                    }
                    
                    let frn = record.file_reference_number;
                    let filename = self.extract_filename(record);
                    
                    // 🔹 只建立映射
                    self.frn_map.insert(
                        frn,
                        ParentInfo {
                            parent_frn: record.parent_file_reference_number,
                            filename,
                        },
                    );
                    
                    offset += record.record_length as usize;
                }
                
                if iteration % 100 == 0 {
                    info!("   Building FRN map: {} entries", self.frn_map.len());
                }
            }
        }
        
        Ok(())
    }
    
    /// 🔹 递归查询完整路径（重用 buffer，避免内存分配）
    fn get_path_reuse<'a>(&'a self, frn: u64, path_parts: &mut Vec<&'a str>, buffer: &mut String) -> Result<()> {
        let mut current_frn = frn;
        let mut depth = 0;
        const MAX_DEPTH: usize = 100;
        
        // 收集路径组件（引用）
        loop {
            depth += 1;
            if depth > MAX_DEPTH {
                return Err(anyhow::anyhow!("Path too deep"));
            }
            
            match self.frn_map.get(&current_frn) {
                Some(info) => {
                    path_parts.push(&info.filename);
                    current_frn = info.parent_frn;
                }
                None => break,
            }
        }
        
        // 拼接路径到 buffer
        buffer.push(self.drive_letter);
        buffer.push_str(":\\");
        
        for part in path_parts.iter().rev() {
            buffer.push_str(part);
            buffer.push('\\');
        }
        
        // 移除末尾的反斜杠
        if buffer.ends_with('\\') && buffer.len() > 3 {
            buffer.pop();
        }
        
        Ok(())
    }
    
    /// 🔹 递归查询完整路径
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
                    path_parts.push(&info.filename);  // 🔥 存储引用而非克隆
                    current_frn = info.parent_frn;
                }
                None => {
                    // 到达根目录
                    break;
                }
            }
        }
        
        // 🔥 优化: 预分配容量并直接拼接,避免join()的额外分配
        let estimated_len = path_parts.iter().map(|s| s.len()).sum::<usize>() 
            + path_parts.len()  // 反斜杠
            + 3;  // "C:\"
        
        let mut path = String::with_capacity(estimated_len);
        path.push(self.drive_letter);
        path.push_str(":\\");
        
        // 反转并拼接(从根到叶)
        for (i, part) in path_parts.iter().rev().enumerate() {
            if i > 0 {
                path.push('\\');
            }
            path.push_str(part);
        }
        
        
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
