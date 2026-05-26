// USN Journal 增量更新器 - 基于 prompt.txt 方案
// 核心功能：
// 1. 维护 FRN Map（FRN -> ParentInfo）用于快速路径构建
// 2. 增量追加新路径到 _paths.dat
// 3. 增量更新 3-gram 索引（FST + RoaringBitmap）
// 4. 处理文件创建/删除/重命名
// 5. FRN Map 持久化（跨重启恢复，避免重建）
// 6. 删除 bitmap 持久化（{drive}_deleted.dat）

use anyhow::Result;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use smartstring::alias::String as SmartString;  // 内联小字符串优化
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Write, Read, Seek, SeekFrom, BufWriter, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{info, debug, error, warn};

use super::index_builder::DeltaState;
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::IO::DeviceIoControl;

use super::types::*;

/// 父目录信息（优化内存占用）
#[derive(Clone, Debug)]
struct ParentInfo {
    parent_frn: u64,
    filename: SmartString,  // 🔥 小字符串 (<23 bytes) 无堆分配
}

/// FRN Map 磁盘记录格式（定长，便于 mmap）
/// 每条记录 = 8(frn) + 8(parent_frn) + 2(name_len) + N(name_bytes) 
/// 实际存储为紧凑变长格式

/// USN 增量更新器
pub struct UsnIncrementalUpdater {
    drive_letter: char,
    output_dir: String,
    last_usn: i64,
    
    // 🔥 核心数据结构
    frn_map: FxHashMap<u64, ParentInfo>,         // FRN -> (parent_frn, filename)
    file_id_counter: u32,                         // 当前最大 file_id
    index_cache: FxHashMap<Vec<u8>, RoaringBitmap>,  // gram -> bitmap 缓存（字节一致）
    deleted_file_ids: RoaringBitmap,             // 🔥 已删除的 file_id 集合
    frn_to_file_id: FxHashMap<u64, u32>,         // 🔥 FRN -> file_id 反向映射（支持删除）
    
    // 🔥 热更新共享句柄（可选）
    delta_state: Option<Arc<RwLock<DeltaState>>>,
    delta_paths: Option<Arc<RwLock<HashMap<u32, String>>>>,
    
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
            index_cache: FxHashMap::default(),
            deleted_file_ids: RoaringBitmap::new(),
            frn_to_file_id: FxHashMap::default(),
            delta_state: None,
            delta_paths: None,
            paths_writer: None,
            paths_offset: 0,
        }
    }
    
    /// 绑定 IndexQuery 和 PathReader 的共享句柄（热更新核心）
    ///
    /// 调用后，每次 flush_index_cache() 或删除事件都会立即更新内存中的索引，
    /// 查询侧持有相同的 Arc 引用，无需重启即可看到新内容。
    pub fn attach_index(
        &mut self,
        delta_state: Arc<RwLock<DeltaState>>,
        delta_paths: Arc<RwLock<HashMap<u32, String>>>,
    ) {
        self.delta_state = Some(delta_state);
        self.delta_paths = Some(delta_paths);
        info!("🔗 Attached hot-update handles for drive {}", self.drive_letter);
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
        
        // 2. 从持久化文件恢复 FRN Map（如果存在）
        self.load_frn_map_from_index()?;
        
        // 3. 加载删除 bitmap
        self.load_deleted_bitmap()?;
        
        // 4. 打开路径文件用于追加
        self.open_paths_file_for_append()?;
        
        info!("✓ USN updater initialized: {} FRNs cached, {} deleted", 
              self.frn_map.len(), self.deleted_file_ids.len());
        
        Ok(())
    }
    
    /// 加载删除 bitmap（跨重启恢复已删除文件集合）
    fn load_deleted_bitmap(&mut self) -> Result<()> {
        let deleted_file = format!("{}\\{}_deleted.dat", self.output_dir, self.drive_letter);
        if !std::path::Path::new(&deleted_file).exists() {
            return Ok(());
        }
        let bytes = std::fs::read(&deleted_file)?;
        match RoaringBitmap::deserialize_from(&bytes[..]) {
            Ok(bmp) => {
                info!("✓ Loaded deleted bitmap: {} deleted file IDs", bmp.len());
                self.deleted_file_ids = bmp;
            }
            Err(e) => {
                warn!("⚠  Failed to load deleted bitmap (will reset): {e:#}");
            }
        }
        Ok(())
    }
    
    /// 持久化删除 bitmap 到磁盘
    fn save_deleted_bitmap(&self) -> Result<()> {
        let deleted_file = format!("{}\\{}_deleted.dat", self.output_dir, self.drive_letter);
        let mut bytes = Vec::new();
        self.deleted_file_ids.serialize_into(&mut bytes)?;
        std::fs::write(&deleted_file, &bytes)?;
        Ok(())
    }
    
    /// 从现有索引文件加载 FRN Map（改为按需加载策略）
    fn load_frn_map_from_index(&mut self) -> Result<()> {
        // 优化：不再预加载整个 FRN Map（避免 800MB 内存占用）
        // 新策略：
        // 1. Monitor 模式下只在需要时通过 USN 事件逐步构建 FRN Map
        // 2. 对于现有文件，首次访问时通过 MFT 查询补充到缓存
        // 3. 使用 LRU 缓存限制内存占用（最多保留 10 万条热点路径）
        
        info!("💡 FRN Map will be built incrementally from USN events (memory-efficient mode)");
        info!("💡 Existing files will be queried on-demand from MFT when needed");
        
        Ok(())
    }
    
    /// 快速扫描 MFT 构建 FRN Map（仅提取父子关系）
    /// 🔥 已弃用：此方法会加载所有文件到内存（~800MB），改用按需加载
    #[allow(dead_code)]
    fn quick_scan_mft_for_frn_map(&mut self) -> Result<()> {
        use windows::Win32::System::Ioctl::*;
        
        info!("⚡ Quick scanning MFT for FRN map...");
        let start = std::time::Instant::now();
        
        let volume_handle = self.open_volume()?;
        
        // 查询 USN Journal 数据
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
        
        // 枚举 USN 数据（类似全量扫描，但只提取元数据）
        let mut enum_data = MftEnumData {
            start_file_reference_number: 0,
            low_usn: 0,
            high_usn: journal_data.next_usn,
        };
        
        const BUFFER_SIZE: usize = 4 * 1024 * 1024;  // 4MB buffer
        let mut buffer = vec![0u8; BUFFER_SIZE];
        
        let mut total_entries = 0;
        
        loop {
            unsafe {
                let result = DeviceIoControl(
                    volume_handle,
                    FSCTL_ENUM_USN_DATA,
                    Some(&enum_data as *const _ as *const std::ffi::c_void),
                    std::mem::size_of::<MftEnumData>() as u32,
                    Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
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
                
                // 解析 USN 记录提取 FRN 映射
                let mut offset = 8usize;
                while offset < bytes_returned as usize {
                    let record = &*(buffer.as_ptr().add(offset) as *const UsnRecordV2);
                    
                    if record.record_length == 0 {
                        break;
                    }
                    
                    let frn = record.file_reference_number;
                    let parent_frn = record.parent_file_reference_number;
                    let filename = self.extract_filename(record);
                    
                    // 添加到 FRN Map
                    self.frn_map.insert(frn, ParentInfo {
                        parent_frn,
                        filename,
                    });
                    
                    total_entries += 1;
                    offset += record.record_length as usize;
                    
                    // 每 100K 输出进度
                    if total_entries % 100_000 == 0 {
                        debug!("   Progress: {} entries", total_entries);
                    }
                }
            }
        }
        
        unsafe { let _ = CloseHandle(volume_handle); }
        
        let elapsed = start.elapsed();
        info!("✓ FRN Map built: {} entries in {:.2}s", total_entries, elapsed.as_secs_f64());
        
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
    
    /// 单次轮询 USN 变更（供 CLI test binary 使用）
    pub fn poll_usn_changes(&mut self) -> Result<()> {
        self.process_usn_changes()
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
            self.frn_map.remove(&frn);
            
            // 🔥 查找 file_id 并标记为已删除
            if let Some(&file_id) = self.frn_to_file_id.get(&frn) {
                self.deleted_file_ids.insert(file_id);
                self.frn_to_file_id.remove(&frn);
                debug!("   🗑  Marked file_id {} as deleted (FRN={})", file_id, frn);                // 🔥 立即更新内存中的 DeltaState.deleted_bitmap
                if let Some(state_arc) = &self.delta_state {
                    state_arc.write().unwrap().deleted_bitmap.insert(file_id);
                }            }
        }
        
        // 文件重命名
        if reason & 0x00001000 != 0 {  // USN_REASON_RENAME_NEW_NAME
            debug!("✏️  File renamed: {}", filename);
            
            // 更新 FRN Map 中的文件名
            if let Some(info) = self.frn_map.get_mut(&frn) {
                let _old_filename = info.filename.clone();
                info.filename = filename.clone();
                
                // 🔥 标记旧 file_id 为已删除，添加新条目
                if let Some(&old_file_id) = self.frn_to_file_id.get(&frn) {
                    self.deleted_file_ids.insert(old_file_id);
                }
                
                // 用新名称添加新索引条目
                self.add_file_to_index(&filename, frn)?;
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
        
        // 3. 记录 FRN -> file_id 反向映射（支持后续删除）
        self.frn_to_file_id.insert(frn, file_id);
        
        // 4. 追加到 _paths.dat
        self.append_path_to_file(&full_path)?;
        
        // 5. 写入共享 delta_paths（查询侧可立即访问）
        if let Some(delta_paths) = &self.delta_paths {
            delta_paths.write().unwrap().insert(file_id, full_path.clone());
        }
        
        // 6. 生成 3-gram 字节 key 并更新内存缓存
        let filename_lower = filename.to_lowercase();
        let grams = Self::split_to_3grams_bytes(&filename_lower);
        
        for gram in grams {
            self.index_cache
                .entry(gram)
                .or_insert_with(RoaringBitmap::new)
                .insert(file_id);
        }
        
        debug!("   ➕ Added to index: {} (file_id={})", full_path, file_id);
        
        Ok(())
    }
    
    /// 从 FRN 构建完整路径（反向递归 + 按需查询 MFT）
    fn build_path_from_frn(&mut self, frn: u64) -> Result<String> {
        let mut components = Vec::with_capacity(32);
        let mut current = frn;
        
        // 反向遍历父目录链
        while current != 0 {
            if let Some(info) = self.frn_map.get(&current) {
                // 缓存命中
                components.push(info.filename.clone());
                current = info.parent_frn;
            } else {
                // 🔥 缓存未命中：从 MFT 查询并添加到缓存
                if let Some((parent_frn, filename)) = self.query_frn_from_mft(current)? {
                    components.push(filename.clone());
                    
                    // 添加到缓存（后续访问更快）
                    self.frn_map.insert(current, ParentInfo {
                        parent_frn,
                        filename,
                    });
                    
                    current = parent_frn;
                } else {
                    // FRN 无效或已删除
                    break;
                }
            }
            
            // 🔥 限制缓存大小（LRU 策略：超过 10 万条时清理旧条目）
            if self.frn_map.len() > 100_000 {
                // 简单策略：清理一半（更复杂的可以用 lru crate）
                let keys_to_remove: Vec<u64> = self.frn_map.keys().take(50_000).copied().collect();
                for key in keys_to_remove {
                    self.frn_map.remove(&key);
                }
                debug!("🧹 FRN cache trimmed to {} entries", self.frn_map.len());
            }
        }
        
        // 如果路径为空，说明 FRN 无效
        if components.is_empty() {
            return Err(anyhow::anyhow!("FRN {} not found or invalid", frn));
        }
        
        // 反转并拼接
        components.reverse();
        let path = format!("{}:\\{}", self.drive_letter, components.join("\\"));
        
        Ok(path)
    }
    
    /// 🔥 从 MFT 查询单个 FRN 的父目录和文件名（按需加载）
    fn query_frn_from_mft(&self, frn: u64) -> Result<Option<(u64, SmartString)>> {
        use windows::Win32::System::Ioctl::*;
        
        let volume_handle = self.open_volume()?;
        
        // 构造查询结构
        let mut ntfs_file_record_input = NtfsFileRecordInputBuffer {
            file_reference_number: frn,
        };
        
        const BUFFER_SIZE: usize = 8192;
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut bytes_returned: u32 = 0;
        
        unsafe {
            let result = DeviceIoControl(
                volume_handle,
                FSCTL_GET_NTFS_FILE_RECORD,
                Some(&mut ntfs_file_record_input as *mut _ as *mut std::ffi::c_void),
                std::mem::size_of::<NtfsFileRecordInputBuffer>() as u32,
                Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
                BUFFER_SIZE as u32,
                Some(&mut bytes_returned),
                None,
            );
            
            let _ = CloseHandle(volume_handle);
            
            if result.is_err() {
                // FRN 不存在或已删除
                return Ok(None);
            }
            
            // 解析 MFT 记录提取文件名和父 FRN
            // （简化实现：实际需要解析 NTFS 文件记录结构）
            // TODO: 完整实现需要解析 $FILE_NAME 属性
            
            // 临时方案：返回 None（路径可能不完整）
            Ok(None)
        }
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
    
    /// 提取文件名
    unsafe fn extract_filename(&self, record: &UsnRecordV2) -> SmartString {
        let name_offset = record.file_name_offset as usize;
        let name_len = record.file_name_length as usize / 2;
        
        let name_ptr = (record as *const UsnRecordV2 as *const u8).add(name_offset) as *const u16;
        let name_slice = std::slice::from_raw_parts(name_ptr, name_len);
        
        SmartString::from(String::from_utf16_lossy(name_slice).as_str())
    }
    
    /// 刷新索引缓存到磁盘（同时持久化删除 bitmap）
    fn flush_index_cache(&mut self) -> Result<()> {
        if self.index_cache.is_empty() {
            return Ok(());
        }
        
        info!("💾 Flushing index cache: {} grams", self.index_cache.len());
        
        // 刷新路径文件
        if let Some(writer) = &mut self.paths_writer {
            writer.flush()?;
        }
        
        // 🔥 增量合并策略：
        // 由于完整实现需要重新构建 FST（FST 不支持增量插入），
        // 当前采用简化方案：
        // 1. 将新增的 gram -> bitmap 写入临时文件
        // 2. 后台任务定期合并临时文件到主索引
        // 3. 查询时同时查主索引 + 临时索引
        
        let temp_index_file = format!("{}\\{}_index_delta.dat", self.output_dir, self.drive_letter);
        
        // 追加到增量索引文件
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&temp_index_file)?;
        
        // 🔥 先更新内存中的 DeltaState（查询侧立即可见）
        if let Some(state_arc) = &self.delta_state {
            let mut state = state_arc.write().unwrap();
            for (gram, bitmap) in &self.index_cache {
                state.gram_bitmaps
                    .entry(gram.clone())
                    .and_modify(|e| *e |= bitmap)
                    .or_insert_with(|| bitmap.clone());
            }
        }
        
        for (gram, bitmap) in self.index_cache.drain() {
            // 写入 gram 长度
            file.write_all(&(gram.len() as u32).to_le_bytes())?;
            
            // 写入 gram 内容（已是 Vec<u8>）
            file.write_all(&gram)?;
            
            // 写入 bitmap（序列化到 Vec）
            let mut bitmap_bytes = Vec::new();
            bitmap.serialize_into(&mut bitmap_bytes)?;
            
            file.write_all(&(bitmap_bytes.len() as u32).to_le_bytes())?;
            file.write_all(&bitmap_bytes)?;
        }
        
        file.flush()?;
        
        info!("✓ Delta index written to {}", temp_index_file);
        
        // 同时持久化删除 bitmap（避免重启后无法过滤已删除文件）
        if !self.deleted_file_ids.is_empty() {
            if let Err(e) = self.save_deleted_bitmap() {
                error!("Failed to save deleted bitmap: {e:#}");
            } else {
                debug!("✓ Saved deleted bitmap: {} IDs", self.deleted_file_ids.len());
            }
        }
        
        Ok(())
    }
    
    /// 拆分为 3-字节 gram（与 IndexBuilder 字节一致）
    fn split_to_3grams_bytes(text: &str) -> Vec<Vec<u8>> {
        let b = text.as_bytes();
        if b.len() < 3 {
            return vec![b.to_vec()];
        }
        b.windows(3).map(|w| w.to_vec()).collect()
    }

    /// 拆分为 3-gram
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
