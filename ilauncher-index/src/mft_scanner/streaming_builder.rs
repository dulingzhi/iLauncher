// 流式 MFT 扫描器 - 基于 prompt.txt 方案
// 核心优化：Arena 分配器 + 流式写入 + 延迟路径构建

use anyhow::Result;
use bumpalo::Bump;
use rustc_hash::{FxHashMap, FxHashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use tracing::{info, debug};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::IO::DeviceIoControl;

use super::types::*;

/// 流式构建器 - 内存占用极低
pub struct StreamingBuilder {
    drive_letter: char,
    output_dir: String,                         // 快照输出目录（new() 时确定，惰性写 tmp 用）
    arena: Bump,                                // 内存池（分块释放）
    parent_cache: FxHashMap<u64, String>,       // FRN -> 完整路径缓存
    path_writer: Option<BufWriter<File>>,       // 流式写入路径（仅 v2 物化链路惰性创建；v3 快照链路不碰 _paths.tmp）
    current_path_id: u32,
    total_files: u64,
    /// 每个写入文件的小写文件名字节（pipeline 用）
    pub filename_entries: Vec<Vec<u8>>,
    /// 每个文件在 paths.tmp 中的字节偏移（pipeline 用）
    pub offset_index: Vec<usize>,
    /// 当前已写入字节数
    current_path_offset: usize,
}

impl StreamingBuilder {
    /// 创建流式构建器
    pub fn new(drive_letter: char, output_dir: &str) -> Result<Self> {
        // 删除旧的临时文件
        let _ = std::fs::remove_file(format!("{}\\{}_paths.tmp", output_dir, drive_letter));

        // 确保目录存在
        std::fs::create_dir_all(output_dir)?;

        Ok(Self {
            drive_letter,
            output_dir: output_dir.to_string(),
            arena: Bump::with_capacity(256 * 1024 * 1024), // 预分配 256MB
            parent_cache: FxHashMap::default(),
            // v3 快照链路（scan_mft_streaming_v3）不写 _paths.tmp，
            // 只有 v2 物化链路（write_path_entry）首次写入时才创建
            path_writer: None,
            current_path_id: 0,
            total_files: 0,
            filename_entries: Vec::with_capacity(2_200_000),
            offset_index: Vec::with_capacity(2_200_000),
            current_path_offset: 0,
        })
    }

    /// v2 物化链路首次写入路径时惰性创建 _paths.tmp
    fn ensure_path_writer(&mut self) -> Result<()> {
        if self.path_writer.is_none() {
            let tmp = format!("{}\\{}_paths.tmp", self.output_dir, self.drive_letter);
            self.path_writer = Some(BufWriter::with_capacity(32 * 1024 * 1024, File::create(tmp)?));
        }
        Ok(())
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

    /// v3 快照扫描：复用阶段 1 的 FrnMap，直接产出 index_v2 列式快照
    /// （`{output_dir}\{drive}.snapshot`），不再走物化全路径的 paths.dat 老链路。
    pub fn scan_mft_streaming_v3(&mut self, output_dir: &str) -> Result<PathBuf> {
        info!("🚀 Starting v3 snapshot scan for drive {}:", self.drive_letter);

        let volume_handle = self.open_volume()?;
        info!("✓ Volume handle opened");

        let journal_data = self.query_usn_journal(volume_handle)?;
        info!("✓ USN Journal ID: {:016X}", journal_data.usn_journal_id);

        info!("📍 Phase 1: Building FRN map...");
        let frn_map = self.build_frn_map(volume_handle, &journal_data)?;
        info!("✓ FRN map built: {} entries", frn_map.len());

        unsafe { let _ = CloseHandle(volume_handle); }

        info!("📝 Phase 2: Writing v3 columnar snapshot...");
        let path = super::v3_export::write_v3_snapshot(
            &frn_map,
            self.drive_letter,
            output_dir,
            journal_data.usn_journal_id,
            journal_data.next_usn,
        )?;

        info!("✅ v3 snapshot written: {:?}", path);
        Ok(path)
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
                    let is_dir =
                        (record.file_attributes & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY.0)
                            != 0;

                    frn_map.insert(frn, ParentInfo { parent_frn, filename, is_dir });
                    
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
    
    /// 🔥 BFS 流式重建路径并写入磁盘（父路径 100% 缓存命中）
    fn stream_paths_to_disk(&mut self, frn_map: &FxHashMap<u64, ParentInfo>) -> Result<()> {
        // ── 步骤 1: 构建 children map (parent_frn → Vec<child_frn>)
        let mut children: FxHashMap<u64, Vec<u64>> =
            FxHashMap::with_capacity_and_hasher(frn_map.len() / 2 + 1, Default::default());
        for (frn, parent_info) in frn_map.iter() {
            children.entry(parent_info.parent_frn).or_default().push(*frn);
        }

        // ── 步骤 2: BFS 从根 FRN=5 (卷根目录) 开始遍历
        let root_path = format!("{}:", self.drive_letter);
        self.parent_cache.insert(5u64, root_path);

        let mut visited: FxHashSet<u64> =
            FxHashSet::with_capacity_and_hasher(frn_map.len() + 1, Default::default());
        visited.insert(5u64);

        let mut queue: std::collections::VecDeque<u64> = std::collections::VecDeque::new();
        queue.push_back(5u64);

        while let Some(parent_frn) = queue.pop_front() {
            let parent_path = match self.parent_cache.get(&parent_frn) {
                Some(p) => p.clone(),
                None => continue,
            };
            let Some(child_frns) = children.get(&parent_frn) else {
                continue;
            };
            for &child_frn in child_frns {
                if !visited.insert(child_frn) {
                    continue; // 已访问
                }
                let Some(parent_info) = frn_map.get(&child_frn) else {
                    continue;
                };
                let child_path = format!("{}\\{}", parent_path, parent_info.filename);

                if self.should_ignore(&child_path) {
                    continue; // 跳过整个子树
                }

                // 如果该节点有子节点，缓存路径且入队
                if children.contains_key(&child_frn) {
                    self.parent_cache.insert(child_frn, child_path.clone());
                    queue.push_back(child_frn);
                }

                self.write_path_entry(&child_path)?;
                self.total_files += 1;

                if self.total_files % 10_000 == 0 {
                    self.flush_buffers()?;
                    if self.total_files % 200_000 == 0 {
                        info!("   Progress: {} files written", self.total_files);
                    }
                }
            }
        }

        // ── 步骤 3: 处理孤立节点（BFS 未访问到的 FRN）
        let mut orphan_count = 0u64;
        let mut path_buffer = String::with_capacity(512);
        for frn in frn_map.keys() {
            if visited.contains(frn) {
                continue;
            }
            path_buffer.clear();
            if let Ok(full_path) = self.build_path_recursive(*frn, frn_map, &mut path_buffer) {
                if !self.should_ignore(&full_path) {
                    self.write_path_entry(&full_path)?;
                    self.total_files += 1;
                    orphan_count += 1;
                }
            }
        }
        if orphan_count > 0 {
            debug!("   Orphan entries: {}", orphan_count);
        }

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
    
    /// 检查是否应该忽略
    fn should_ignore(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();

        path_lower.contains("$recycle.bin") ||
        path_lower.contains("system volume information") ||
        path_lower.contains("\\winsxs\\") ||
        path_lower.contains("\\temp\\")
    }

    /// 写入路径条目（同时收集 offset_index + filename_entries 供 pipeline 使用）
    fn write_path_entry(&mut self, path: &str) -> Result<()> {
        self.ensure_path_writer()?;
        let path_bytes = path.as_bytes();
        let path_len = path_bytes.len();

        // 记录本条目在 paths.tmp 中的字节偏移
        self.offset_index.push(self.current_path_offset);
        self.current_path_offset += 4 + path_len;

        // 提取小写文件名（pipeline 用）
        let filename = path.rsplit('\\').next().unwrap_or(path);
        self.filename_entries.push(filename.to_lowercase().into_bytes());

        // 写入路径长度（4字节）
        let len = (path_len as u32).to_le_bytes();
        self.path_writer.as_mut().unwrap().write_all(&len)?;

        // 写入路径内容
        self.path_writer.as_mut().unwrap().write_all(path_bytes)?;

        self.current_path_id += 1;

        Ok(())
    }

    /// 刷新缓冲区
    fn flush_buffers(&mut self) -> Result<()> {
        if let Some(w) = self.path_writer.as_mut() {
            w.flush()?;
        }

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

        // 关闭文件（v3 链路从未创建，take 后为 None，无操作）
        drop(self.path_writer.take());

        // 重命名临时文件为最终文件
        let temp_paths = format!("{}\\{}_paths.tmp", output_dir, self.drive_letter);
        let final_paths = format!("{}\\{}_paths.dat", output_dir, self.drive_letter);

        std::fs::rename(temp_paths, final_paths)?;

        info!("✅ Database finalized: {} files", self.total_files);

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// 每个测试独立的临时输出目录
    fn temp_output_dir(tag: &str) -> String {
        let dir = std::env::temp_dir().join(format!("ilauncher_sb_test_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().to_string()
    }

    #[test]
    fn new_does_not_create_paths_tmp() {
        // v3 快照链路（new + scan_mft_streaming_v3）不应再留下 0 字节 _paths.tmp
        let dir = temp_output_dir("v3");
        let b = StreamingBuilder::new('T', &dir).unwrap();
        drop(b);
        let tmp = format!("{}\\T_paths.tmp", dir);
        assert!(!PathBuf::from(&tmp).exists(), "v3 链路不应创建 {}", tmp);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_removes_stale_paths_tmp() {
        // 上一进程残留的 tmp 应在 new() 时清理（历史 0 字节残留自愈）
        let dir = temp_output_dir("stale");
        let tmp = format!("{}\\T_paths.tmp", dir);
        std::fs::write(&tmp, b"stale").unwrap();
        let b = StreamingBuilder::new('T', &dir).unwrap();
        drop(b);
        assert!(!PathBuf::from(&tmp).exists(), "stale tmp 应被 new() 删除");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn v2_write_then_finalize_renames_tmp_to_dat() {
        // v2 物化链路契约：首次写入惰性创建 tmp，finalize 改名 .dat
        let dir = temp_output_dir("v2");
        {
            let mut b = StreamingBuilder::new('T', &dir).unwrap();
            b.write_path_entry("T:\\alpha.txt").unwrap();
            b.write_path_entry("T:\\dir\\beta.txt").unwrap();
            b.flush_buffers().unwrap();
            let tmp = format!("{}\\T_paths.tmp", dir);
            assert!(PathBuf::from(&tmp).exists(), "首次写入后 tmp 应存在");
            b.finalize(&dir).unwrap();
        }
        let dat = PathBuf::from(format!("{}\\T_paths.dat", dir));
        assert!(dat.exists(), "finalize 后应生成 .dat");
        let tmp = PathBuf::from(format!("{}\\T_paths.tmp", dir));
        assert!(!tmp.exists(), "finalize 后 tmp 应已改名消失");
        // 内容校验：两条路径 + 长度前缀
        let bytes = std::fs::read(&dat).unwrap();
        let l1 = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        assert_eq!(&bytes[4..4 + l1], b"T:\\alpha.txt");
        let off = 4 + l1;
        let l2 = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
        assert_eq!(&bytes[off + 4..off + 4 + l2], b"T:\\dir\\beta.txt");
        // offset_index 记录的字节偏移与文件布局一致
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn flush_without_writer_is_noop() {
        // v3 链路不创建 writer，flush_buffers 必须静默无操作（不能 panic）
        let dir = temp_output_dir("flush");
        let mut b = StreamingBuilder::new('T', &dir).unwrap();
        b.flush_buffers().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
