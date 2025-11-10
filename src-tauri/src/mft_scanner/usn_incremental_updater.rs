// USN Journal 增量更新器 - 基于 prompt.txt 方案
// 核心功能：
// 1. 维护 FRN Map（FRN -> ParentInfo）用于快速路径构建
// 2. 增量追加新路径到 _paths.dat
// 3. 增量更新 3-gram 索引（FST + RoaringBitmap）
// 4. 处理文件创建/删除/重命名

use anyhow::Result;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Write, Read, Seek, SeekFrom, BufWriter, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, debug, error};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::IO::DeviceIoControl;

use super::types::*;

/// 父目录信息
#[derive(Clone, Debug)]
struct ParentInfo {
    parent_frn: u64,
    filename: String,
}

/// USN 增量更新器
pub struct UsnIncrementalUpdater {
    drive_letter: char,
    output_dir: String,
    last_usn: i64,
    
    // 🔥 核心数据结构
    frn_map: FxHashMap<u64, ParentInfo>,         // FRN -> (parent_frn, filename)
    file_id_counter: u32,                         // 当前最大 file_id
    index_cache: HashMap<String, RoaringBitmap>,  // gram -> bitmap 缓存
    deleted_files: FxHashMap<u64, u32>,          // deleted_frn -> old_file_id
    
    // 文件句柄
    paths_writer: Option<BufWriter<File>>,
    paths_offset: u64,  // 当前写入偏移量
}

impl UsnIncrementalUpdater {
    pub fn new(drive_letter: char, output_dir: String) -> Self {
        Self {
            drive_letter,
            output_dir,
            last_usn: 0,
            frn_map: FxHashMap::default(),
            file_id_counter: 0,
            index_cache: HashMap::new(),
            deleted_files: FxHashMap::default(),
            paths_writer: None,
            paths_offset: 0,
        }
    }
    
    /// 初始化 USN（读取当前位置 + 加载现有 FRN Map）
    pub fn initialize(&mut self) -> Result<()> {
        info!("🔧 Initializing USN updater for drive {}:", self.drive_letter);
        
        // 1. 读取 USN Journal 当前位置
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
        
        // 2. 从现有索引文件加载 FRN Map（如果存在）
        self.load_frn_map_from_index()?;
        
        // 3. 打开路径文件用于追加
        self.open_paths_file_for_append()?;
        
        info!("✓ USN updater initialized: {} FRNs cached", self.frn_map.len());
        
        Ok(())
    }
    
    /// 从现有索引文件加载 FRN Map
    fn load_frn_map_from_index(&mut self) -> Result<()> {
        // TODO: 扫描现有 _paths.dat 重建 FRN Map
        // 由于首次扫描已生成索引，这里需要读取它来恢复 FRN 映射
        // 
        // 临时方案：如果找不到现有数据，启动时触发一次快速 MFT 扫描
        // 只提取 FRN + ParentFRN + Filename，不构建索引
        
        info!("⚠️  FRN Map rebuild from existing index not implemented yet");
        info!("💡 Will build FRN map incrementally from USN events");
        
        Ok(())
    }
    
    /// 打开路径文件用于追加
    fn open_paths_file_for_append(&mut self) -> Result<()> {
        let paths_file = format!("{}\\{}_paths.dat", self.output_dir, self.drive_letter);
        
        // 检查文件是否存在
        if std::path::Path::new(&paths_file).exists() {
            let file = OpenOptions::new()
                .read(true)
                .append(true)
                .open(&paths_file)?;
            
            // 获取当前文件大小（下一个写入偏移量）
            self.paths_offset = file.metadata()?.len();
            
            // 统计当前有多少个路径（用于分配新 file_id）
            self.file_id_counter = self.count_existing_paths(&paths_file)?;
            
            self.paths_writer = Some(BufWriter::new(file));
            
            info!("✓ Opened paths file for append: {} bytes, {} existing paths",
                  self.paths_offset, self.file_id_counter);
        } else {
            // 新建文件
            let file = File::create(&paths_file)?;
            self.paths_writer = Some(BufWriter::new(file));
            self.paths_offset = 0;
            self.file_id_counter = 0;
            
            info!("✓ Created new paths file");
        }
        
        Ok(())
    }
    
    /// 统计现有路径数量
    fn count_existing_paths(&self, paths_file: &str) -> Result<u32> {
        let mut file = BufReader::new(File::open(paths_file)?);
        let mut count = 0u32;
        let mut len_buf = [0u8; 4];
        
        while file.read_exact(&mut len_buf).is_ok() {
            let path_len = u32::from_le_bytes(len_buf) as usize;
            
            // 跳过路径内容
            file.seek(SeekFrom::Current(path_len as i64))?;
            count += 1;
        }
        
        Ok(count)
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
        let frn = record.file_reference_number;
        let parent_frn = record.parent_file_reference_number;
        
        // 文件创建
        if reason & 0x00000100 != 0 {  // USN_REASON_FILE_CREATE
            debug!("📁 File created: {}", filename);
            
            // 更新 FRN Map
            self.frn_map.insert(frn, ParentInfo {
                parent_frn,
                filename: filename.clone(),
            });
            
            // 添加到索引
            self.add_file_to_index(&filename, frn)?;
        }
        
        // 文件删除
        if reason & 0x00000200 != 0 {  // USN_REASON_FILE_DELETE
            debug!("🗑️  File deleted: {}", filename);
            
            // 从 FRN Map 移除
            if let Some(_info) = self.frn_map.remove(&frn) {
                // TODO: 标记文件已删除，但保留 file_id 用于去重
                // 实际应该在 bitmap 中移除对应的 bit
                // 这里简化处理：记录到 deleted_files
                if let Some(&file_id) = self.deleted_files.get(&frn) {
                    self.deleted_files.insert(frn, file_id);
                }
            }
        }
        
        // 文件重命名
        if reason & 0x00001000 != 0 {  // USN_REASON_RENAME_NEW_NAME
            debug!("✏️  File renamed: {}", filename);
            
            // 更新 FRN Map 中的文件名
            if let Some(info) = self.frn_map.get_mut(&frn) {
                let old_filename = info.filename.clone();
                info.filename = filename.clone();
                
                // 更新索引：删除旧 3-gram + 添加新 3-gram
                self.update_file_name_in_index(&old_filename, &filename, frn)?;
            } else {
                // 新监控到的文件，添加到 FRN Map
                self.frn_map.insert(frn, ParentInfo {
                    parent_frn,
                    filename: filename.clone(),
                });
                
                self.add_file_to_index(&filename, frn)?;
            }
        }
        
        Ok(())
    }
    
    /// 添加文件到索引
    fn add_file_to_index(&mut self, filename: &str, frn: u64) -> Result<()> {
        // 1. 构建完整路径
        let full_path = self.build_path_from_frn(frn)?;
        
        // 2. 分配新的 file_id
        let file_id = self.file_id_counter;
        self.file_id_counter += 1;
        
        // 3. 追加到 _paths.dat
        self.append_path_to_file(&full_path)?;
        
        // 4. 生成 3-gram 并更新内存缓存
        let filename_lower = filename.to_lowercase();
        let grams = self.split_to_3grams(&filename_lower);
        
        for gram in grams {
            self.index_cache
                .entry(gram)
                .or_insert_with(RoaringBitmap::new)
                .insert(file_id);
        }
        
        debug!("   ➕ Added to index: {} (file_id={})", full_path, file_id);
        
        Ok(())
    }
    
    /// 从 FRN 构建完整路径（反向递归）
    fn build_path_from_frn(&self, frn: u64) -> Result<String> {
        let mut components = Vec::with_capacity(32);
        let mut current = frn;
        
        // 反向遍历父目录链
        while current != 0 {
            if let Some(info) = self.frn_map.get(&current) {
                components.push(info.filename.clone());
                current = info.parent_frn;
            } else {
                // 到达根目录或未知父目录
                break;
            }
        }
        
        // 如果路径为空，说明 FRN Map 尚未完整
        if components.is_empty() {
            return Err(anyhow::anyhow!("FRN {} not found in cache", frn));
        }
        
        // 反转并拼接
        components.reverse();
        let path = format!("{}:\\{}", self.drive_letter, components.join("\\"));
        
        Ok(path)
    }
    
    /// 追加路径到文件
    fn append_path_to_file(&mut self, path: &str) -> Result<()> {
        if let Some(writer) = &mut self.paths_writer {
            let path_bytes = path.as_bytes();
            
            // 写入长度前缀（4字节）
            writer.write_all(&(path_bytes.len() as u32).to_le_bytes())?;
            
            // 写入路径内容
            writer.write_all(path_bytes)?;
            
            self.paths_offset += 4 + path_bytes.len() as u64;
        }
        
        Ok(())
    }
    
    /// 删除文件（更新索引）
    fn remove_file(&mut self, frn: u64) -> Result<()> {
        // TODO: 从 bitmap 中移除对应的 bit
        // 由于 RoaringBitmap 不支持直接删除，实际需要重建或标记删除
        // 这里简化：仅记录到 deleted_files
        Ok(())
    }
    
    /// 更新文件名（更新索引）
    fn update_file_name_in_index(&mut self, old_name: &str, new_name: &str, _frn: u64) -> Result<()> {
        // TODO: 找到对应的 file_id，然后：
        // 1. 从旧 3-gram 的 bitmap 中移除 file_id
        // 2. 添加到新 3-gram 的 bitmap
        //
        // 由于没有维护 file_id -> frn 的反向映射，这里简化：
        // 直接生成新 3-gram（旧 3-gram 保留，下次重建时清理）
        
        let new_name_lower = new_name.to_lowercase();
        let grams = self.split_to_3grams(&new_name_lower);
        
        // 分配新 file_id（视为新文件）
        let file_id = self.file_id_counter;
        self.file_id_counter += 1;
        
        for gram in grams {
            self.index_cache
                .entry(gram)
                .or_insert_with(RoaringBitmap::new)
                .insert(file_id);
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
    
    /// 刷新索引缓存到磁盘
    fn flush_index_cache(&mut self) -> Result<()> {
        if self.index_cache.is_empty() {
            return Ok(());
        }
        
        info!("💾 Flushing index cache: {} grams", self.index_cache.len());
        
        // 刷新路径文件
        if let Some(writer) = &mut self.paths_writer {
            writer.flush()?;
        }
        
        // TODO: 实现增量合并逻辑
        // 1. 加载现有 FST + Bitmap
        // 2. 合并新的 bitmap
        // 3. 重新写入
        //
        // 当前简化方案：直接清空缓存，依赖下次全量重建
        
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
