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
use std::fs::{File, OpenOptions};
use std::io::{Write, Read, Seek, SeekFrom, BufWriter, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, debug, error, warn};

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
///
/// 跨进程同步协议（与 UI 进程的约定）：
/// - 每次 flush 后递增 `{drive}_delta.version`，UI 查询前比对版本号；
/// - 新路径追加到 `{drive}_paths.dat`，其偏移量记录在 `{drive}_offsets_delta.dat`
///   （追加条目的 file_id 从 offsets.dat 的条目数起连续编号），UI 据此扩展偏移索引；
/// - 删除集合持久化在 `{drive}_deleted.dat`；
/// - DeltaMerger 合并主索引后重建 offsets.dat、删除 offsets_delta.dat 并递增
///   `{drive}_index.version`（UI 全量重载）。
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

    // 🔥 USN 追加路径的偏移量（file_id = delta_base + 下标，隐式连续）
    delta_base: u32,
    offsets_delta: Vec<u64>,
    delta_version: u64,
    pending_changes: bool,
    last_flush: Instant,

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
            delta_base: 0,
            offsets_delta: Vec::new(),
            delta_version: 0,
            pending_changes: false,
            last_flush: Instant::now(),
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
        
        // 2. 从持久化文件恢复 FRN Map（如果存在）
        self.load_frn_map_from_index()?;
        
        // 3. 加载删除 bitmap
        self.load_deleted_bitmap()?;
        
        // 4. 打开路径文件用于追加
        self.open_paths_file_for_append()?;
        
        // 5. 恢复 USN 追加偏移量与 delta 版本号（跨重启续接 file_id 编号）
        self.load_offsets_delta()?;
        self.delta_version = Self::read_delta_version_file(self.drive_letter, &self.output_dir);
        
        info!("✓ USN updater initialized: {} FRNs cached, {} deleted, {} appended offsets (delta v{})",
              self.frn_map.len(), self.deleted_file_ids.len(), self.offsets_delta.len(), self.delta_version);
        
        Ok(())
    }

    /// 读取 delta 版本号文件（{drive}_delta.version）
    fn read_delta_version_file(drive_letter: char, output_dir: &str) -> u64 {
        let version_file = format!("{}\\{}_delta.version", output_dir, drive_letter);
        std::fs::read_to_string(&version_file)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0)
    }

    /// 递增并写回 delta 版本号（通知 UI 进程重读增量索引与路径）
    fn bump_delta_version(&mut self) -> Result<()> {
        self.delta_version += 1;
        let version_file = format!("{}\\{}_delta.version", self.output_dir, self.drive_letter);
        std::fs::write(&version_file, self.delta_version.to_string())?;
        debug!("✓ Delta version bumped to {} for drive {}", self.delta_version, self.drive_letter);
        Ok(())
    }

    /// 加载 USN 追加的偏移量列表（{drive}_offsets_delta.dat）
    ///
    /// 追加条目的 file_id 从 offsets.dat（基线）的条目数起连续编号：
    /// delta_base = 当前总路径数 - 已追加条目数。
    /// 正常运行时每次启动都是全量重建（offsets_delta 清零），此恢复路径
    /// 为 Phase 2「冷启动 + USN 追赶」预留。
    fn load_offsets_delta(&mut self) -> Result<()> {
        let delta_file = format!("{}\\{}_offsets_delta.dat", self.output_dir, self.drive_letter);
        if !std::path::Path::new(&delta_file).exists() {
            self.delta_base = self.file_id_counter;
            return Ok(());
        }

        let bytes = std::fs::read(&delta_file)?;
        if bytes.len() < 4 {
            self.delta_base = self.file_id_counter;
            return Ok(());
        }

        let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let mut offsets = Vec::with_capacity(count);
        let mut pos = 4usize;
        for _ in 0..count {
            if pos + 8 > bytes.len() {
                break; // 截断文件：采用已读到的部分
            }
            offsets.push(u64::from_le_bytes(bytes[pos..pos + 8].try_into().unwrap()));
            pos += 8;
        }

        // file_id_counter 是 paths.dat 全量计数（基线 + 已追加），由此反推基线
        self.delta_base = self.file_id_counter.saturating_sub(offsets.len() as u32);
        self.offsets_delta = offsets;

        info!("✓ Loaded offsets delta: {} appended entries (base file_id = {})",
              self.offsets_delta.len(), self.delta_base);
        Ok(())
    }

    /// 将 USN 追加的偏移量列表写入磁盘（{drive}_offsets_delta.dat）
    fn save_offsets_delta(&self) -> Result<()> {
        let delta_file = format!("{}\\{}_offsets_delta.dat", self.output_dir, self.drive_letter);
        let mut file = BufWriter::new(File::create(&delta_file)?);
        file.write_all(&(self.offsets_delta.len() as u32).to_le_bytes())?;
        for &offset in &self.offsets_delta {
            file.write_all(&offset.to_le_bytes())?;
        }
        file.flush()?;
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

            // 🔥 定期 flush： gram 缓存超过 1000 条立即刷；否则每 30 秒把
            // 积累的变更（新增路径 + 删除标记）落盘并 bump delta 版本号，
            // 保证 UI 进程最多 30 秒即可见 USN 变更
            if self.pending_changes
                && (self.index_cache.len() > 1000 || self.last_flush.elapsed() >= Duration::from_secs(30))
            {
                if let Err(e) = self.flush_index_cache() {
                    error!("Failed to flush index cache: {:#}", e);
                }
            }

            // 每 100ms 轮询一次
            std::thread::sleep(Duration::from_millis(100));
        }

        // 🔥 退出前把剩余变更（< 1000 条的尾量）全部落盘，避免丢失
        if let Err(e) = self.finalize() {
            error!("Failed to finalize USN updater: {:#}", e);
        }

        info!("USN monitoring stopped for drive {}", self.drive_letter);

        Ok(())
    }

    /// 收尾：把所有未落盘的变更（gram 缓存、删除 bitmap、追加偏移量）写入磁盘
    /// 并递增 delta 版本号。退出监控前必须调用，否则最后一批变更会丢失。
    pub fn finalize(&mut self) -> Result<()> {
        if self.index_cache.is_empty() && !self.pending_changes {
            return Ok(());
        }

        info!("🏁 Finalizing USN updater for drive {} ({} grams, {} deleted)",
              self.drive_letter, self.index_cache.len(), self.deleted_file_ids.len());

        self.flush_index_cache()?;

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
                    self.pending_changes = true;
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
                debug!("   🗑  Marked file_id {} as deleted (FRN={})", file_id, frn);
            }
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

        // 4. 追加到 _paths.dat，并记录条目偏移量（供 UI 侧 offsets_delta 寻址）
        let entry_offset = self.paths_offset;
        self.append_path_to_file(&full_path)?;
        debug_assert_eq!(self.delta_base as usize + self.offsets_delta.len(), file_id as usize);
        self.offsets_delta.push(entry_offset);

        // 5. 生成 3-gram 字节 key 并更新内存缓存
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
    
    /// 刷新索引缓存到磁盘（同时持久化删除 bitmap、追加偏移量，并 bump delta 版本号）
    ///
    /// 跨进程可见性协议（与 UI 进程的约定）：
    /// 1. gram -> bitmap 追加到 {drive}_index_delta.dat
    /// 2. 删除集合写入 {drive}_deleted.dat
    /// 3. 追加路径偏移量重写 {drive}_offsets_delta.dat
    /// 4. 递增 {drive}_delta.version —— UI 查询前比对版本号，
    ///    变化则调用 IndexQuery::hot_reload_delta() + PathReader::reload_paths()
    ///
    /// 持 INDEX_IO_LOCK 与 DeltaMerger::merge 互斥（同一 Service 进程内）。
    fn flush_index_cache(&mut self) -> Result<()> {
        let _io_guard = super::delta_merger::INDEX_IO_LOCK.lock().unwrap();

        if self.index_cache.is_empty() && self.deleted_file_ids.is_empty() {
            self.pending_changes = false;
            self.last_flush = Instant::now();
            return Ok(());
        }

        info!("💾 Flushing index cache: {} grams", self.index_cache.len());

        // 刷新路径文件（确保 offsets_delta 指向的数据已落盘）
        if let Some(writer) = &mut self.paths_writer {
            writer.flush()?;
        }

        // gram -> bitmap 追加到增量索引文件
        if !self.index_cache.is_empty() {
            let temp_index_file = format!("{}\\{}_index_delta.dat", self.output_dir, self.drive_letter);

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&temp_index_file)?;

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
        }

        // 持久化删除 bitmap（避免重启后无法过滤已删除文件）
        if !self.deleted_file_ids.is_empty() {
            if let Err(e) = self.save_deleted_bitmap() {
                error!("Failed to save deleted bitmap: {e:#}");
            } else {
                debug!("✓ Saved deleted bitmap: {} IDs", self.deleted_file_ids.len());
            }
        }

        // 持久化追加偏移量（UI 据此扩展偏移索引，读取新增路径）
        if let Err(e) = self.save_offsets_delta() {
            error!("Failed to save offsets delta: {e:#}");
        }

        // 🔥 最后递增版本号：以上所有文件均已落盘，UI 读到新版本号时数据一定完整
        if let Err(e) = self.bump_delta_version() {
            error!("Failed to bump delta version: {e:#}");
        }

        self.pending_changes = false;
        self.last_flush = Instant::now();

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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::mft_scanner::{DeltaMerger, IndexBuilder, IndexQuery, PathReader};

    const TEST_DRIVE: char = 'T';

    fn temp_dir(tag: &str) -> String {
        let dir = std::env::temp_dir()
            .join(format!("ilauncher_usn_test_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().to_string()
    }

    /// 手工构建一份"全量扫描完成"的基线索引（paths.dat + offsets.dat + fst/bitmaps）
    fn build_base_index(output_dir: &str, paths: &[&str]) {
        let paths_file = format!("{}\\{}_paths.dat", output_dir, TEST_DRIVE);
        let mut writer = BufWriter::new(File::create(&paths_file).unwrap());
        let mut offset_index: Vec<usize> = Vec::with_capacity(paths.len());
        let mut offset = 0usize;
        for p in paths {
            offset_index.push(offset);
            writer.write_all(&(p.len() as u32).to_le_bytes()).unwrap();
            writer.write_all(p.as_bytes()).unwrap();
            offset += 4 + p.len();
        }
        writer.flush().unwrap();
        drop(writer);

        let filename_entries: Vec<Vec<u8>> = paths
            .iter()
            .map(|p| p.rsplit('\\').next().unwrap_or(p).to_lowercase().into_bytes())
            .collect();

        let mut builder = IndexBuilder::new(TEST_DRIVE);
        builder.build_from_entries(&filename_entries, &offset_index, output_dir).unwrap();
        builder.save_index(output_dir).unwrap();
    }

    /// 构造 updater 并初始化到"基线索引刚建好"的状态
    /// （手动复刻 initialize() 中不依赖卷句柄的部分）
    fn make_updater(output_dir: &str) -> UsnIncrementalUpdater {
        let mut updater = UsnIncrementalUpdater::new(TEST_DRIVE, output_dir.to_string());
        updater.open_paths_file_for_append().unwrap();
        updater.load_offsets_delta().unwrap();
        updater.delta_version =
            UsnIncrementalUpdater::read_delta_version_file(TEST_DRIVE, output_dir);
        updater
    }

    fn add_dir(updater: &mut UsnIncrementalUpdater, frn: u64, name: &str, parent_frn: u64) {
        updater.frn_map.insert(
            frn,
            ParentInfo {
                parent_frn,
                filename: SmartString::from(name),
            },
        );
    }

    /// 端到端：Service flush → UI 热重载 → 新增可见 / 删除被过滤 / 版本号递增
    #[test]
    fn test_delta_sync_end_to_end() {
        let dir = temp_dir("sync");
        let base_paths = ["T:\\Users\\alpha.txt", "T:\\Users\\beta.txt"];
        build_base_index(&dir, &base_paths);

        // ── UI 侧：先加载缓存（模拟进程内已打开的查询器） ──
        let mut reader = PathReader::open(TEST_DRIVE, &dir).unwrap();
        let query = IndexQuery::open(TEST_DRIVE, &dir).unwrap();
        assert_eq!(IndexQuery::read_delta_version(TEST_DRIVE, &dir), 0);

        // ── Service 侧：模拟 USN 新增 gamma.txt + 删除 alpha.txt，然后 flush ──
        let mut updater = make_updater(&dir);
        add_dir(&mut updater, 5, "Users", 0);
        add_dir(&mut updater, 100, "gamma.txt", 5);
        updater.add_file_to_index("gamma.txt", 100).unwrap();
        updater.deleted_file_ids.insert(0u32); // 删除 alpha.txt
        updater.flush_index_cache().unwrap();

        // 版本号已递增
        assert_eq!(IndexQuery::read_delta_version(TEST_DRIVE, &dir), 1);

        // 热重载前：新 file_id(2) 不可解析，delta 搜索未命中
        assert!(reader.get_path(2).is_err());
        assert!(query.search("gamma", 10).unwrap().is_empty());

        // ── UI 热重载（file_search 检测到 delta 版本变化后执行的操作） ──
        query.hot_reload_delta().unwrap();
        reader.reload_paths().unwrap();

        // 新路径可见且内容正确
        assert_eq!(reader.get_path(2).unwrap(), "T:\\Users\\gamma.txt");

        // delta 索引命中新增文件
        let ids = query.search("gamma", 10).unwrap();
        assert!(ids.contains(&2), "delta 索引应命中新增文件 gamma.txt: {:?}", ids);

        // 已删除文件被 deleted bitmap 过滤
        let ids = query.search("alpha", 10).unwrap();
        assert!(!ids.contains(&0), "已删除的 alpha.txt 不应出现: {:?}", ids);
        // 未删除的基线文件不受影响
        let ids = query.search("beta", 10).unwrap();
        assert!(ids.contains(&1), "beta.txt 应仍可搜索: {:?}", ids);

        // ── 第二轮变更：验证版本号继续递增、热重载可重复执行（幂等） ──
        add_dir(&mut updater, 101, "delta.txt", 5);
        updater.add_file_to_index("delta.txt", 101).unwrap();
        updater.flush_index_cache().unwrap();
        assert_eq!(IndexQuery::read_delta_version(TEST_DRIVE, &dir), 2);

        query.hot_reload_delta().unwrap();
        reader.reload_paths().unwrap();
        assert_eq!(reader.get_path(3).unwrap(), "T:\\Users\\delta.txt");
        let ids = query.search("delta", 10).unwrap();
        assert!(ids.contains(&3), "第二轮新增应可搜索: {:?}", ids);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Merger 合并：delta 折入主索引、offsets.dat 覆盖追加条目、delta 文件清理
    #[test]
    fn test_merger_folds_delta_and_rebuilds_offsets() {
        let dir = temp_dir("merge");
        let base_paths = ["T:\\Users\\alpha.txt"];
        build_base_index(&dir, &base_paths);

        // Service 侧：追加一个文件并 flush（产生 delta 文件与 offsets_delta）
        let mut updater = make_updater(&dir);
        add_dir(&mut updater, 5, "Users", 0);
        add_dir(&mut updater, 100, "gamma.txt", 5);
        updater.add_file_to_index("gamma.txt", 100).unwrap();
        updater.flush_index_cache().unwrap();
        assert!(std::path::Path::new(&format!("{}\\{}_index_delta.dat", dir, TEST_DRIVE)).exists());
        assert!(std::path::Path::new(&format!("{}\\{}_offsets_delta.dat", dir, TEST_DRIVE)).exists());

        // 执行合并
        let merger = DeltaMerger::new(TEST_DRIVE, dir.clone());
        merger.merge().unwrap();

        // delta 文件已清理
        assert!(!std::path::Path::new(&format!("{}\\{}_index_delta.dat", dir, TEST_DRIVE)).exists());
        assert!(!std::path::Path::new(&format!("{}\\{}_offsets_delta.dat", dir, TEST_DRIVE)).exists());

        // 主索引版本已递增（UI 的 needs_reload 会感知）
        let version = std::fs::read_to_string(format!("{}\\{}_index.version", dir, TEST_DRIVE))
            .unwrap()
            .trim()
            .parse::<u64>()
            .unwrap();
        assert_eq!(version, 1);

        // 合并后全新打开：gamma 已在主索引中直接命中（无 delta 文件），
        // offsets.dat 覆盖全部 2 条路径
        let query = IndexQuery::open(TEST_DRIVE, &dir).unwrap();
        let reader = PathReader::open(TEST_DRIVE, &dir).unwrap();
        let ids = query.search("gamma", 10).unwrap();
        assert!(ids.contains(&1), "合并后 gamma 应从主索引命中: {:?}", ids);
        assert_eq!(reader.get_path(1).unwrap(), "T:\\Users\\gamma.txt");
        // offsets.dat 已折入追加条目：全新打开的读取器无需 offsets_delta 即可解析 id 1，
        // 且不存在 id 2
        assert!(reader.get_path(2).is_err(), "offsets.dat 应恰好覆盖 2 条路径");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// finalize：不足 1000 条的尾量变更也必须落盘（此前直接丢失）
    #[test]
    fn test_finalize_flushes_tail_changes() {
        let dir = temp_dir("finalize");
        let base_paths = ["T:\\Users\\alpha.txt"];
        build_base_index(&dir, &base_paths);

        let mut updater = make_updater(&dir);
        add_dir(&mut updater, 5, "Users", 0);
        add_dir(&mut updater, 100, "gamma.txt", 5);
        updater.add_file_to_index("gamma.txt", 100).unwrap();
        updater.deleted_file_ids.insert(0u32);

        // 不调用 flush_index_cache，直接 finalize（模拟退出前收尾）
        updater.finalize().unwrap();

        assert_eq!(IndexQuery::read_delta_version(TEST_DRIVE, &dir), 1);

        let query = IndexQuery::open(TEST_DRIVE, &dir).unwrap();
        let ids = query.search("gamma", 10).unwrap();
        assert!(ids.contains(&1), "finalize 应落盘新增文件: {:?}", ids);
        let ids = query.search("alpha", 10).unwrap();
        assert!(!ids.contains(&0), "finalize 应落盘删除标记: {:?}", ids);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
