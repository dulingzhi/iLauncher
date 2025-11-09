// USN Journal 实时监控器 - 持续监听文件变化

use anyhow::{Result, Context};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, error, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::IO::DeviceIoControl;

use crate::mft_scanner::types::*;
use crate::mft_scanner::database::Database;

pub struct UsnMonitor {
    drive_letter: char,
    frn_map: FrnMap,
}

impl UsnMonitor {
    pub fn new(drive_letter: char) -> Self {
        Self {
            drive_letter,
            frn_map: FrnMap::default(),  // 🔥 使用 default() 替代 new()
        }
    }
    
    /// 🔹 启动实时监控（阻塞式运行）
    pub fn start_monitoring(&mut self, output_dir: &str, config: &ScanConfig) -> Result<()> {
        self.start_monitoring_with_signal(output_dir, config, Arc::new(AtomicBool::new(true)))
    }
    
    /// 🔹 启动实时监控（支持外部停止信号）
    pub fn start_monitoring_with_signal(
        &mut self, 
        output_dir: &str, 
        config: &ScanConfig,
        running: Arc<AtomicBool>
    ) -> Result<()> {
        info!("👀 Starting real-time monitoring for drive {}:", self.drive_letter);
        
        // 1. 检查管理员权限
        if !Self::check_admin_rights() {
            error!("❌ Requires administrator privileges");
            return Err(anyhow::anyhow!("Administrator privileges required"));
        }
        
        // 2. 打开卷句柄
        let volume_handle = self.open_volume()?;
        info!("✓ Volume handle opened");
        
        // 3. 查询 USN Journal
        let journal_data = self.query_usn_journal(volume_handle)?;
        info!("✓ USN Journal ID: {:016X}", journal_data.usn_journal_id);
        
        // 4. � 跳过加载 FRN Map（避免巨大内存占用）
        // Monitor 模式下，文件变化会实时构建路径，不需要预加载所有映射
        info!("💡 Monitor mode: FRN map will be built incrementally on demand");
        
        // 5. 🔹 进入监控循环（阻塞式）
        info!("🔄 Entering monitoring loop (blocking mode)...");
        
        let mut read_data = ReadUsnJournalData {
            start_usn: journal_data.next_usn,
            reason_mask: 0xFFFFFFFF,  // 监听所有变化
            return_only_on_close: 0,
            timeout: 0,  // 无超时，阻塞等待
            bytes_to_wait_for: 1,
            usn_journal_id: journal_data.usn_journal_id,
        };
        
        const BUFFER_SIZE: usize = 64 * 1024;  // 64KB buffer
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut bytes_returned: u32 = 0;
        
        loop {
            // 🔹 检查停止信号
            if !running.load(Ordering::SeqCst) {
                info!("🛑 Stop signal received, exiting monitor loop for drive {}", self.drive_letter);
                break Ok(());
            }
            
            unsafe {
                // 🔹 阻塞式读取 USN Journal（线程休眠直到有文件变化）
                let result = DeviceIoControl(
                    volume_handle,
                    FSCTL_READ_USN_JOURNAL,
                    Some(&read_data as *const _ as *const _),
                    std::mem::size_of::<ReadUsnJournalData>() as u32,
                    Some(buffer.as_mut_ptr() as *mut _),
                    BUFFER_SIZE as u32,
                    Some(&mut bytes_returned),
                    None,
                );
                
                if result.is_err() {
                    let error = GetLastError();
                    warn!("Read USN Journal failed: {:?}, retrying...", error);
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
                
                if bytes_returned < 8 {
                    continue;
                }
                
                // 更新下次读取位置
                let next_usn = i64::from_le_bytes(buffer[0..8].try_into().unwrap());
                read_data.start_usn = next_usn;
                
                // � 解析 USN 记录并更新数据库（临时打开写连接）
                self.process_usn_records(&buffer, bytes_returned as usize, output_dir, config)?;
            }
        }
    }
    
    /// 🔹 处理 USN 记录
    fn process_usn_records(
        &mut self,
        buffer: &[u8],
        bytes_returned: usize,
        output_dir: &str,
        config: &ScanConfig,
    ) -> Result<()> {
        let mut offset = 8usize;
        let mut entries = Vec::new();
        
        unsafe {
            while offset + std::mem::size_of::<UsnRecordV2>() <= bytes_returned {
                let record_ptr = buffer.as_ptr().add(offset) as *const UsnRecordV2;
                let record = &*record_ptr;
                
                if record.record_length == 0 {
                    break;
                }
                
                // 提取文件名
                let name = self.extract_filename(record);
                let frn = record.file_reference_number;
                let parent_frn = record.parent_file_reference_number;
                
                // 🔹 根据 Reason 判断操作类型
                const USN_REASON_FILE_CREATE: u32 = 0x00000100;
                const USN_REASON_FILE_DELETE: u32 = 0x00000200;
                const USN_REASON_RENAME_NEW_NAME: u32 = 0x00002000;
                const USN_REASON_RENAME_OLD_NAME: u32 = 0x00001000;
                
                if record.reason & USN_REASON_FILE_DELETE != 0 {
                    // 🔹 文件删除
                    self.frn_map.remove(&frn);
                    // TODO: 从数据库删除
                    info!("   🗑️  Deleted: {}", name);
                    
                } else if record.reason & USN_REASON_RENAME_OLD_NAME != 0 {
                    // 🔹 重命名（旧名）- 暂存
                    
                } else if record.reason & USN_REASON_RENAME_NEW_NAME != 0 {
                    // 🔹 重命名（新名）- 更新映射
                    self.frn_map.insert(frn, ParentInfo {
                        parent_frn,
                        filename: name.clone(),
                    });
                    
                    // 重建路径并更新数据库
                    if let Ok(full_path) = self.get_path(frn) {
                        if !config.is_ignore(&full_path) {
                            info!("   ✏️  Renamed: {}", full_path);
                            // TODO: 更新数据库
                        }
                    }
                    
                } else if record.reason & USN_REASON_FILE_CREATE != 0 {
                    // 🔹 文件创建
                    self.frn_map.insert(frn, ParentInfo {
                        parent_frn,
                        filename: name.clone(),
                    });
                    
                    // 重建路径并插入数据库
                    if let Ok(full_path) = self.get_path(frn) {
                        if !config.is_ignore(&full_path) {
                            let ascii_sum = Database::calc_ascii_sum(&name);
                            
                            entries.push(MftFileEntry {
                                path: full_path.clone(),
                                ascii_sum,
                                priority: 0,
                            });
                            
                            info!("   ➕ Created: {}", full_path);
                        }
                    }
                }
                
                offset += record.record_length as usize;
            }
        }
        
        // 🔥 批量插入（临时打开写连接，立即释放）
        if !entries.is_empty() {
            let mut db = Database::create_for_write(self.drive_letter, output_dir)?;
            db.insert_batch(&entries)?;
            drop(db);  // 🔥 立即释放写锁，避免阻塞读连接
            info!("   ✅ Inserted {} new entries", entries.len());
        }
        
        Ok(())
    }
    
    /// 🔹 从数据库加载 FRN 映射表
    fn load_frn_map_from_db(&mut self, output_dir: &str) -> Result<()> {
        use crate::mft_scanner::database::Database;
        
        info!("📚 Loading FRN map from database for drive {}...", self.drive_letter);
        let start = std::time::Instant::now();
        
        let mut db = Database::open(self.drive_letter, output_dir)?;
        
        // 从数据库查询所有文件路径，重建 FRN 映射
        let entries = db.get_all_entries()?;
        
        info!("⚠️  FRN map reconstruction requires re-scanning MFT (not implemented)");
        info!("💡 Monitoring will work for new files, but existing file paths may be incomplete");
        
        // TODO: 完整实现需要：
        // 1. 在数据库中存储 FRN 字段
        // 2. 或重新扫描 MFT 构建 FRN 映射
        // 临时方案：只监控新建文件，现有文件路径可能不完整
        
        info!("📚 Database has {} entries, FRN map: {} entries in {:.2}s", 
            entries.len(),
            self.frn_map.len(), 
            start.elapsed().as_secs_f64()
        );
        
        Ok(())
    }
    
    /// 🔹 递归查询完整路径（同 scanner.rs）
    fn get_path(&self, frn: u64) -> Result<String> {
        let mut path_parts = Vec::new();
        let mut current_frn = frn;
        let mut depth = 0;
        const MAX_DEPTH: usize = 100;
        
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
                    break;
                }
            }
        }
        
        path_parts.reverse();
        
        let path = if path_parts.is_empty() {
            format!("{}:\\", self.drive_letter)
        } else {
            format!("{}:\\{}", self.drive_letter, path_parts.join("\\"))
        };
        
        Ok(path)
    }
    
    /// 提取文件名
    fn extract_filename(&self, record: &UsnRecordV2) -> String {
        unsafe {
            let name_ptr = (record as *const UsnRecordV2 as *const u8)
                .add(record.file_name_offset as usize) as *const u16;
            let name_len = record.file_name_length as usize / 2;
            let name_slice = std::slice::from_raw_parts(name_ptr, name_len);
            String::from_utf16_lossy(name_slice)
        }
    }
    
    /// 检查管理员权限
    fn check_admin_rights() -> bool {
        use windows::Win32::UI::Shell::IsUserAnAdmin;
        unsafe { IsUserAnAdmin().as_bool() }
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
            DeviceIoControl(
                volume_handle,
                FSCTL_QUERY_USN_JOURNAL,
                None,
                0,
                Some(&mut journal_data as *mut _ as *mut _),
                std::mem::size_of::<UsnJournalData>() as u32,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(journal_data)
    }
}
