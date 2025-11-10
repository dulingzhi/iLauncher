// USN Journal 增量更新器 - 基于 prompt.txt 方案

use anyhow::Result;
use roaring::RoaringBitmap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, debug, error};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::IO::DeviceIoControl;

use super::types::*;

/// USN 增量更新器
pub struct UsnIncrementalUpdater {
    drive_letter: char,
    output_dir: String,
    last_usn: i64,
    index_cache: HashMap<String, RoaringBitmap>,  // gram -> bitmap 缓存
}

impl UsnIncrementalUpdater {
    pub fn new(drive_letter: char, output_dir: String) -> Self {
        Self {
            drive_letter,
            output_dir,
            last_usn: 0,
            index_cache: HashMap::new(),
        }
    }
    
    /// 初始化 USN（读取当前位置）
    pub fn initialize(&mut self) -> Result<()> {
        let volume_handle = self.open_volume()?;
        
        let mut journal_data: UsnJournalData = Default::default();
        let mut bytes_returned: u32 = 0;
        
        unsafe {
            DeviceIoControl(
                volume_handle,
                FSCTL_QUERY_USN_JOURNAL,
                None,
                0,
                Some(&mut journal_data as *mut _ as *mut std::ffi::c_void),
                std::mem::size_of::<UsnJournalData>() as u32,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        self.last_usn = journal_data.next_usn;
        
        unsafe { let _ = CloseHandle(volume_handle); }
        
        info!("✓ USN initialized at: {}", self.last_usn);
        
        Ok(())
    }
    
    /// 启动监控（带停止信号）
    pub fn start_monitoring(&mut self, running: Arc<AtomicBool>) -> Result<()> {
        info!("👀 Starting USN monitoring for drive {}:", self.drive_letter);
        
        while running.load(Ordering::SeqCst) {
            if let Err(e) = self.process_usn_changes() {
                error!("USN processing error: {:#}", e);
                std::thread::sleep(Duration::from_secs(5));
            }
            
            // 每 100ms 轮询一次
            std::thread::sleep(Duration::from_millis(100));
        }
        
        info!("USN monitoring stopped for drive {}", self.drive_letter);
        
        Ok(())
    }
    
    /// 处理 USN 变更
    fn process_usn_changes(&mut self) -> Result<()> {
        let volume_handle = self.open_volume()?;
        
        let journal_data = self.query_usn_journal(volume_handle)?;
        
        let read_data = ReadUsnJournalData {
            start_usn: self.last_usn,
            reason_mask: 0xFFFFFFFF,  // 监听所有变更
            return_only_on_close: 0,
            timeout: 0,
            bytes_to_wait_for: 0,
            usn_journal_id: journal_data.usn_journal_id,
        };
        
        const BUFFER_SIZE: usize = 1024 * 1024;
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut bytes_returned: u32 = 0;
        
        unsafe {
            let result = DeviceIoControl(
                volume_handle,
                FSCTL_READ_USN_JOURNAL,
                Some(&read_data as *const _ as *const std::ffi::c_void),
                std::mem::size_of::<ReadUsnJournalData>() as u32,
                Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
                BUFFER_SIZE as u32,
                Some(&mut bytes_returned),
                None,
            );
            
            if result.is_err() {
                let error = GetLastError();
                if error.0 != 38 {  // 不是 EOF
                    return Err(anyhow::anyhow!("Read USN failed: {:?}", error));
                }
            }
            
            if bytes_returned > 8 {
                // 更新 last_usn
                let next_usn = i64::from_le_bytes(buffer[0..8].try_into()?);
                self.last_usn = next_usn;
                
                // 解析变更记录
                let mut offset = 8usize;
                let mut changes = 0;
                
                while offset + std::mem::size_of::<UsnRecordV2>() <= bytes_returned as usize {
                    let record_ptr = buffer.as_ptr().add(offset) as *const UsnRecordV2;
                    let record = &*record_ptr;
                    
                    if record.record_length == 0 {
                        break;
                    }
                    
                    // 处理变更
                    self.handle_usn_record(record)?;
                    changes += 1;
                    
                    offset += record.record_length as usize;
                }
                
                if changes > 0 {
                    debug!("Processed {} USN changes", changes);
                    
                    // 每 1000 条刷新缓存
                    if self.index_cache.len() > 1000 {
                        self.flush_index_cache()?;
                    }
                }
            }
            
            let _ = CloseHandle(volume_handle);
        }
        
        Ok(())
    }
    
    /// 处理单条 USN 记录
    unsafe fn handle_usn_record(&mut self, record: &UsnRecordV2) -> Result<()> {
        let filename = self.extract_filename(record);
        let reason = record.reason;
        
        // 文件创建
        if reason & 0x00000100 != 0 {  // USN_REASON_FILE_CREATE
            debug!("File created: {}", filename);
            self.add_file(&filename, record.file_reference_number)?;
        }
        
        // 文件删除
        if reason & 0x00000200 != 0 {  // USN_REASON_FILE_DELETE
            debug!("File deleted: {}", filename);
            self.remove_file(record.file_reference_number)?;
        }
        
        // 文件重命名
        if reason & 0x00001000 != 0 {  // USN_REASON_RENAME_NEW_NAME
            debug!("File renamed: {}", filename);
            self.update_file_name(&filename, record.file_reference_number)?;
        }
        
        Ok(())
    }
    
    /// 提取文件名
    unsafe fn extract_filename(&self, record: &UsnRecordV2) -> String {
        let name_offset = record.file_name_offset as usize;
        let name_len = record.file_name_length as usize / 2;
        
        let name_ptr = (record as *const UsnRecordV2 as *const u8).add(name_offset) as *const u16;
        let name_slice = std::slice::from_raw_parts(name_ptr, name_len);
        
        String::from_utf16_lossy(name_slice)
    }
    
    /// 添加文件（更新索引）
    fn add_file(&mut self, filename: &str, _frn: u64) -> Result<()> {
        let filename_lower = filename.to_lowercase();
        
        // 生成 3-gram
        let grams = self.split_to_3grams(&filename_lower);
        
        // TODO: 获取新文件的 ID
        let file_id = 0u32;  // 需要从路径文件中分配新 ID
        
        // 更新索引缓存
        for gram in grams {
            self.index_cache
                .entry(gram)
                .or_insert_with(RoaringBitmap::new)
                .insert(file_id);
        }
        
        Ok(())
    }
    
    /// 删除文件（更新索引）
    fn remove_file(&mut self, _frn: u64) -> Result<()> {
        // TODO: 标记删除（需要维护 FRN -> FileID 映射）
        Ok(())
    }
    
    /// 更新文件名（更新索引）
    fn update_file_name(&mut self, new_name: &str, _frn: u64) -> Result<()> {
        // TODO: 删除旧 3-gram + 添加新 3-gram
        self.add_file(new_name, _frn)
    }
    
    /// 刷新索引缓存到磁盘
    fn flush_index_cache(&mut self) -> Result<()> {
        if self.index_cache.is_empty() {
            return Ok(());
        }
        
        info!("💾 Flushing index cache: {} grams", self.index_cache.len());
        
        // TODO: 实现增量合并逻辑
        // 1. 加载现有 FST + Bitmap
        // 2. 合并新的 bitmap
        // 3. 重新写入
        
        self.index_cache.clear();
        
        Ok(())
    }
    
    /// 拆分为 3-gram
    fn split_to_3grams(&self, text: &str) -> Vec<String> {
        if text.len() < 3 {
            return vec![text.to_string()];
        }
        
        let chars: Vec<char> = text.chars().collect();
        chars.windows(3)
            .map(|w| w.iter().collect())
            .collect()
    }
    
    /// 打开卷句柄
    fn open_volume(&self) -> Result<HANDLE> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        
        let volume_path = format!(r"\\.\{}:", self.drive_letter);
        let wide: Vec<u16> = OsStr::new(&volume_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        unsafe {
            let handle = CreateFileW(
                windows::core::PCWSTR(wide.as_ptr()),
                FILE_GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )?;
            
            Ok(handle)
        }
    }
    
    /// 查询 USN Journal
    fn query_usn_journal(&self, volume_handle: HANDLE) -> Result<UsnJournalData> {
        let mut journal_data: UsnJournalData = Default::default();
        let mut bytes_returned: u32 = 0;
        
        unsafe {
            DeviceIoControl(
                volume_handle,
                FSCTL_QUERY_USN_JOURNAL,
                None,
                0,
                Some(&mut journal_data as *mut _ as *mut std::ffi::c_void),
                std::mem::size_of::<UsnJournalData>() as u32,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(journal_data)
    }
}
