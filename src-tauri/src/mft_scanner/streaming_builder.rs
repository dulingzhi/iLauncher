// 流式 MFT 扫描器 - 基于 prompt.txt 方案
// 核心优化：Arena 分配器 + 流式写入 + 延迟路径构建

use anyhow::Result;
use bumpalo::Bump;
use rustc_hash::FxHashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use tracing::{info, debug};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::IO::DeviceIoControl;

use super::types::*;

/// FileRecord - 不存储完整路径，只存储文件名引用和父目录ID
struct FileRecord {
    name: String,        // 文件名（从 Arena 复制）
    parent_frn: u64,     // 父目录 FRN
    size: u64,
    is_dir: bool,
}

/// 流式构建器 - 内存占用极低
pub struct StreamingBuilder {
    drive_letter: char,
    arena: Bump,                                // 内存池（分块释放）
    temp_records: Vec<FileRecord>,              // 临时记录（批量处理）
    parent_cache: FxHashMap<u64, String>,       // FRN -> 完整路径缓存
    path_writer: BufWriter<File>,               // 流式写入路径
    index_writer: BufWriter<File>,              // 流式写入索引
    current_path_id: u32,
    total_files: u64,
}

impl StreamingBuilder {
    /// 创建流式构建器
    pub fn new(drive_letter: char, output_dir: &str) -> Result<Self> {
        // 删除旧的临时文件
        let _ = std::fs::remove_file(format!("{}\\{}_paths.tmp", output_dir, drive_letter));
        let _ = std::fs::remove_file(format!("{}\\{}_index.tmp", output_dir, drive_letter));
        
        // 确保目录存在
        std::fs::create_dir_all(output_dir)?;
        
        Ok(Self {
            drive_letter,
            arena: Bump::with_capacity(256 * 1024 * 1024), // 预分配 256MB
            temp_records: Vec::with_capacity(100_000),     // 10万条批量
            parent_cache: FxHashMap::default(),
            path_writer: BufWriter::with_capacity(
                32 * 1024 * 1024,
                File::create(format!("{}\\{}_paths.tmp", output_dir, drive_letter))?,
            ),
            index_writer: BufWriter::with_capacity(
                32 * 1024 * 1024,
                File::create(format!("{}\\{}_index.tmp", output_dir, drive_letter))?,
            ),
            current_path_id: 0,
            total_files: 0,
        })
    }
    
    /// 从 MFT 流式读取（内存占用稳定）
    pub fn scan_mft_streaming(&mut self) -> Result<()> {
        info!("🚀 Starting streaming scan for drive {}:", self.drive_letter);
        
        // 打开卷句柄
        let volume_handle = self.open_volume()?;
        info!("✓ Volume handle opened");
        
        // 查询 USN Journal
        let journal_data = self.query_usn_journal(volume_handle)?;
        info!("✓ USN Journal ID: {:016X}", journal_data.usn_journal_id);
        
        // 🔥 阶段 1：构建 FRN Map（最小化内存）
        info!("📍 Phase 1: Building FRN map...");
        let frn_map = self.build_frn_map(volume_handle, &journal_data)?;
        info!("✓ FRN map built: {} entries", frn_map.len());
        
        // 🔥 阶段 2：流式重建路径 + 批量写入
        info!("📝 Phase 2: Streaming path reconstruction...");
        self.stream_paths_to_disk(&frn_map)?;
        
        unsafe { let _ = CloseHandle(volume_handle); }
        
        info!("✅ Streaming scan completed: {} files", self.total_files);
        Ok(())
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
    
    /// 🔥 构建 FRN Map（只存储映射，不重建路径）
    /// ⚡ 优化版本：增大缓冲区 + 预分配 HashMap + 进度报告
    fn build_frn_map(
        &mut self,
        volume_handle: HANDLE,
        journal_data: &UsnJournalData,
    ) -> Result<FxHashMap<u64, ParentInfo>> {
        // 预分配 HashMap 容量（减少 rehashing）
        let mut frn_map = FxHashMap::with_capacity_and_hasher(2_500_000, Default::default());
        
        let mut enum_data = MftEnumData {
            start_file_reference_number: 0,
            low_usn: 0,
            high_usn: journal_data.next_usn,
        };
        
        // ⚡ 优化1: 增大缓冲区到 4MB（减少 IO 调用）
        const BUFFER_SIZE: usize = 4 * 1024 * 1024;
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut bytes_returned: u32 = 0;
        
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
                
                // ⚠️ USN 记录是**变长**的，必须串行解析！
                let mut offset = 8usize;
                while offset < bytes_returned as usize {
                    let record = &*(buffer.as_ptr().add(offset) as *const UsnRecordV2);
                    
                    if record.record_length == 0 {
                        break;
                    }
                    
                    let frn = record.file_reference_number;
                    let parent_frn = record.parent_file_reference_number;
                    let filename = self.extract_filename(record);
                    
                    frn_map.insert(frn, ParentInfo { parent_frn, filename });
                    
                    offset += record.record_length as usize;
                    
                    // ⚡ 优化2: 每 100K 条记录输出进度
                    if frn_map.len() % 100_000 == 0 {
                        debug!("   Progress: {} entries", frn_map.len());
                    }
                }
            }
        }
        
        debug!("   Total entries: {}", frn_map.len());
        Ok(frn_map)
    }
    
    /// 提取文件名
    unsafe fn extract_filename(&self, record: &UsnRecordV2) -> String {
        let name_offset = record.file_name_offset as usize;
        let name_len = record.file_name_length as usize / 2;
        
        let name_ptr = (record as *const UsnRecordV2 as *const u8).add(name_offset) as *const u16;
        let name_slice = std::slice::from_raw_parts(name_ptr, name_len);
        
        String::from_utf16_lossy(name_slice)
    }
    
    /// 🔥 流式重建路径并写入磁盘（内存占用极低）
    fn stream_paths_to_disk(&mut self, frn_map: &FxHashMap<u64, ParentInfo>) -> Result<()> {
        const BATCH_SIZE: usize = 10_000;  // 🔥 从 50K 提升到 100K
        
        // 重用 buffer
        let mut path_buffer = String::with_capacity(512);
        
        for (frn, parent_info) in frn_map.iter() {
            path_buffer.clear();
            
            // 🔹 延迟构建完整路径
            if let Ok(full_path) = self.build_path_recursive(*frn, frn_map, &mut path_buffer) {
                // 过滤系统路径
                if self.should_ignore(&full_path) {
                    continue;
                }
                
                // 计算优先级
                let priority = self.calculate_priority(&full_path, parent_info);
                
                // 流式写入路径
                self.write_path_entry(&full_path, priority)?;
                
                self.total_files += 1;
                
                // 批量刷新
                if self.total_files % BATCH_SIZE as u64 == 0 {
                    self.flush_buffers()?;
                    
                    // 🔥 减少日志频率（从 50K 提升到 200K）
                    if self.total_files % 200_000 == 0 {
                        info!("   Progress: {} files written", self.total_files);
                    }
                }
            }
        }
        
        // 刷新剩余数据
        self.flush_buffers()?;
        
        Ok(())
    }
    
    /// 🔥 递归构建完整路径（重用 buffer）
    fn build_path_recursive(
        &mut self,
        frn: u64,
        frn_map: &FxHashMap<u64, ParentInfo>,
        path_buffer: &mut String,
    ) -> Result<String> {
        // 检查缓存
        if let Some(cached_path) = self.parent_cache.get(&frn) {
            return Ok(cached_path.clone());
        }
        
        let mut components = Vec::with_capacity(20);
        let mut current_frn = frn;
        
        // 向上遍历父目录
        for _ in 0..50 {  // 最大深度 50
            if current_frn == 0 || current_frn == 5 {  // 根目录
                break;
            }
            
            if let Some(parent_info) = frn_map.get(&current_frn) {
                components.push(parent_info.filename.as_str());
                current_frn = parent_info.parent_frn;
            } else {
                break;
            }
        }
        
        // 反转拼接
        path_buffer.clear();
        path_buffer.push_str(&format!("{}:", self.drive_letter));
        
        for component in components.iter().rev() {
            path_buffer.push('\\');
            path_buffer.push_str(component);
        }
        
        // 缓存路径（父目录）
        if components.len() <= 5 {  // 只缓存浅层路径
            self.parent_cache.insert(frn, path_buffer.clone());
        }
        
        Ok(path_buffer.clone())
    }
    
    /// 计算优先级
    fn calculate_priority(&self, path: &str, parent_info: &ParentInfo) -> i32 {
        // 根据扩展名计算优先级
        if parent_info.filename.ends_with(".exe") {
            100
        } else if parent_info.filename.ends_with(".lnk") {
            90
        } else if parent_info.filename.ends_with(".bat") || parent_info.filename.ends_with(".cmd") {
            80
        } else if path.contains("\\Program Files") || path.contains("\\Windows") {
            70
        } else {
            50
        }
    }
    
    /// 检查是否应该忽略
    fn should_ignore(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();
        
        path_lower.contains("$recycle.bin") ||
        path_lower.contains("system volume information") ||
        path_lower.contains("\\winsxs\\") ||
        path_lower.contains("\\temp\\")
    }
    
    /// 写入路径条目
    fn write_path_entry(&mut self, path: &str, priority: i32) -> Result<()> {
        // 写入路径长度（4字节）
        let path_bytes = path.as_bytes();
        let len = (path_bytes.len() as u32).to_le_bytes();
        self.path_writer.write_all(&len)?;
        
        // 写入路径内容
        self.path_writer.write_all(path_bytes)?;
        
        // 写入优先级（4字节）
        let priority_bytes = priority.to_le_bytes();
        self.index_writer.write_all(&priority_bytes)?;
        
        self.current_path_id += 1;
        
        Ok(())
    }
    
    /// 刷新缓冲区
    fn flush_buffers(&mut self) -> Result<()> {
        self.path_writer.flush()?;
        self.index_writer.flush()?;
        
        // 🔥 释放 Arena 内存
        if self.arena.allocated_bytes() > 128 * 1024 * 1024 {  // 超过 128MB
            self.arena.reset();
            debug!("   Arena reset: freed memory");
        }
        
        Ok(())
    }
    
    /// 完成构建，生成最终文件
    pub fn finalize(mut self, output_dir: &str) -> Result<()> {
        info!("🔧 Finalizing database...");
        
        // 刷新所有缓冲区
        self.flush_buffers()?;
        
        // 关闭文件
        drop(self.path_writer);
        drop(self.index_writer);
        
        // 重命名临时文件为最终文件
        let temp_paths = format!("{}\\{}_paths.tmp", output_dir, self.drive_letter);
        let temp_index = format!("{}\\{}_index.tmp", output_dir, self.drive_letter);
        
        let final_paths = format!("{}\\{}_paths.dat", output_dir, self.drive_letter);
        let final_index = format!("{}\\{}_index.dat", output_dir, self.drive_letter);
        
        std::fs::rename(temp_paths, final_paths)?;
        std::fs::rename(temp_index, final_index)?;
        
        info!("✅ Database finalized: {} files", self.total_files);
        
        Ok(())
    }
}
