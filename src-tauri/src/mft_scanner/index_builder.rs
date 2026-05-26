// 3-Gram 倒排索引构建器 - 基于 prompt.txt 方案
// 使用 FST + RoaringBitmap 实现极致压缩

use anyhow::Result;
use fst::{Map, MapBuilder};
use rayon::prelude::*;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::{Arc, RwLock};
use tracing::info;

/// 3-Gram 索引构建器
pub struct IndexBuilder {
    drive_letter: char,
    /// gram key: 3字节滑动窗口的原始字节（UTF-8）
    gram_index: FxHashMap<Vec<u8>, RoaringBitmap>,
    total_grams: usize,
}

impl IndexBuilder {
    pub fn new(drive_letter: char) -> Self {
        Self {
            drive_letter,
            gram_index: FxHashMap::with_capacity_and_hasher(100_000, Default::default()),
            total_grams: 0,
        }
    }
    
    /// 从路径文件构建索引（fallback，优先使用 build_from_entries）
    pub fn build_from_paths(&mut self, output_dir: &str) -> Result<()> {
        info!("🔍 Building 3-gram index for drive {}:", self.drive_letter);
        let paths_file = format!("{}\\{}_paths.dat", output_dir, self.drive_letter);
        let mut reader = BufReader::with_capacity(32 * 1024 * 1024, File::open(paths_file)?);

        let mut len_buf = [0u8; 4];
        let mut current_offset = 0usize;
        let mut offset_index: Vec<usize> = Vec::with_capacity(2_200_000);
        let mut filename_entries: Vec<Vec<u8>> = Vec::with_capacity(2_200_000);

        while reader.read_exact(&mut len_buf).is_ok() {
            offset_index.push(current_offset);
            let path_len = u32::from_le_bytes(len_buf) as usize;
            let mut path_bytes = vec![0u8; path_len];
            reader.read_exact(&mut path_bytes)?;
            let path = String::from_utf8_lossy(&path_bytes);
            let filename = path.rsplit('\\').next().unwrap_or(&path);
            filename_entries.push(filename.to_lowercase().into_bytes());
            current_offset += 4 + path_len;
        }

        info!("   Read {} file names, building gram index...", filename_entries.len());
        self.build_gram_index_parallel(&filename_entries);
        self.total_grams = self.gram_index.len();
        info!("✓ Index built: {} files, {} unique 3-grams", filename_entries.len(), self.total_grams);

        Self::write_offset_index(&offset_index, output_dir, self.drive_letter)?;
        Ok(())
    }

    /// 从内存文件名条目构建索引（pipeline 路径，无 paths.dat 二次读取）
    pub fn build_from_entries(
        &mut self,
        filename_entries: &[Vec<u8>],
        offset_index: &[usize],
        output_dir: &str,
    ) -> Result<()> {
        info!("⚡ Building 3-gram index from {} entries (pipeline mode)...", filename_entries.len());
        self.build_gram_index_parallel(filename_entries);
        self.total_grams = self.gram_index.len();
        info!("✓ Index built: {} files, {} unique 3-grams", filename_entries.len(), self.total_grams);
        Self::write_offset_index(offset_index, output_dir, self.drive_letter)?;
        Ok(())
    }

    /// Rayon 并行 3-gram 索引构建
    fn build_gram_index_parallel(&mut self, filename_entries: &[Vec<u8>]) {
        if filename_entries.is_empty() {
            return;
        }
        let num_threads = rayon::current_num_threads().max(1);
        let chunk_size = (filename_entries.len() + num_threads - 1) / num_threads;

        let local_maps: Vec<FxHashMap<Vec<u8>, RoaringBitmap>> = filename_entries
            .par_chunks(chunk_size)
            .enumerate()
            .map(|(chunk_idx, chunk)| {
                let file_id_start = chunk_idx * chunk_size;
                let mut local: FxHashMap<Vec<u8>, RoaringBitmap> =
                    FxHashMap::with_capacity_and_hasher(50_000, Default::default());
                for (offset, name_bytes) in chunk.iter().enumerate() {
                    let file_id = (file_id_start + offset) as u32;
                    Self::add_3grams_into(name_bytes, file_id, &mut local);
                }
                local
            })
            .collect();

        for local_map in local_maps {
            for (key, bitmap) in local_map {
                *self.gram_index.entry(key).or_default() |= bitmap;
            }
        }
    }

    /// 静态辅助：向 map 插入 name_bytes 对应的所有 3-字节 gram
    pub fn add_3grams_into(
        name_bytes: &[u8],
        file_id: u32,
        map: &mut FxHashMap<Vec<u8>, RoaringBitmap>,
    ) {
        if name_bytes.is_empty() {
            return;
        }
        if name_bytes.len() < 3 {
            map.entry(name_bytes.to_vec()).or_default().insert(file_id);
            return;
        }
        for window in name_bytes.windows(3) {
            map.entry(window.to_vec()).or_default().insert(file_id);
        }
    }

    /// 写入 offset index 到磁盘
    fn write_offset_index(offset_index: &[usize], output_dir: &str, drive_letter: char) -> Result<()> {
        let offset_file = format!("{}\\{}_offsets.dat", output_dir, drive_letter);
        let mut writer = BufWriter::new(File::create(offset_file)?);
        writer.write_all(&(offset_index.len() as u32).to_le_bytes())?;
        for offset in offset_index {
            writer.write_all(&(*offset as u64).to_le_bytes())?;
        }
        writer.flush()?;
        Ok(())
    }
    
    /// 保存索引到 FST + RoaringBitmap 文件
    pub fn save_index(&self, output_dir: &str) -> Result<()> {
        info!("💾 Saving compressed index...");
        
        // 🔥 步骤 1: 构建 FST（3-gram -> offset 映射）
        let fst_file = format!("{}\\{}_index.fst", output_dir, self.drive_letter);
        let bitmap_file = format!("{}\\{}_bitmaps.dat", output_dir, self.drive_letter);
        
        // 排序所有 3-gram（FST 需要有序）
        let mut sorted_grams: Vec<_> = self.gram_index.iter().collect();
        sorted_grams.sort_by(|a, b| a.0.cmp(b.0));
        
        // 构建 FST
        let mut fst_builder = MapBuilder::new(BufWriter::new(File::create(&fst_file)?))?;
        let mut bitmap_writer = BufWriter::new(File::create(&bitmap_file)?);
        
        let mut current_offset: u64 = 0;
        
        for (gram, bitmap) in sorted_grams {
            // FST 记录：gram 字节切片 -> bitmap 在文件中的偏移量
            fst_builder.insert(gram.as_slice(), current_offset)?;
            
            // 序列化 RoaringBitmap
            let bitmap_bytes = self.serialize_bitmap(bitmap)?;
            
            // 写入长度（4字节）+ 数据
            let len = (bitmap_bytes.len() as u32).to_le_bytes();
            bitmap_writer.write_all(&len)?;
            bitmap_writer.write_all(&bitmap_bytes)?;
            
            current_offset += 4 + bitmap_bytes.len() as u64;
        }
        
        fst_builder.finish()?;
        bitmap_writer.flush()?;
        
        // 计算压缩率
        let fst_size = std::fs::metadata(&fst_file)?.len();
        let bitmap_size = std::fs::metadata(&bitmap_file)?.len();
        let total_size = fst_size + bitmap_size;
        
        info!("✓ Index saved:");
        info!("   FST: {:.2} MB", fst_size as f64 / 1024.0 / 1024.0);
        info!("   Bitmaps: {:.2} MB", bitmap_size as f64 / 1024.0 / 1024.0);
        info!("   Total: {:.2} MB", total_size as f64 / 1024.0 / 1024.0);
        
        Ok(())
    }
    
    /// 序列化 RoaringBitmap（使用内置压缩）
    fn serialize_bitmap(&self, bitmap: &RoaringBitmap) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        bitmap.serialize_into(&mut buffer)?;
        Ok(buffer)
    }
}

/// Delta 状态（增量新增 gram + 已删除 bitmap），由查询器和 USN 更新器共享
/// 通过 Arc<RwLock<_>> 实现无锁并发读、写时短暂独占
pub struct DeltaState {
    pub gram_bitmaps: FxHashMap<Vec<u8>, RoaringBitmap>,
    pub deleted_bitmap: RoaringBitmap,
}

impl DeltaState {
    pub fn new() -> Self {
        Self {
            gram_bitmaps: FxHashMap::default(),
            deleted_bitmap: RoaringBitmap::new(),
        }
    }
}

/// 索引查询器（零拷贝，内存映射）
pub struct IndexQuery {
    drive_letter: char,
    output_dir: String,
    fst_map: Map<memmap2::Mmap>,
    bitmap_mmap: memmap2::Mmap,
    /// 热更新 delta 状态：与 UsnIncrementalUpdater 共享同一 Arc
    delta_state: Arc<RwLock<DeltaState>>,
    loaded_version: u64,  // 主索引（FST+Bitmap）版本号
}

impl IndexQuery {
    /// 打开索引（零拷贝加载）
    pub fn open(drive_letter: char, output_dir: &str) -> Result<Self> {
        let open_start = std::time::Instant::now();
        
        let fst_file = format!("{}\\{}_index.fst", output_dir, drive_letter);
        let bitmap_file = format!("{}\\{}_bitmaps.dat", output_dir, drive_letter);
        
        // 内存映射 FST
        let fst_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&File::open(fst_file)?)?
        };
        let fst_map = Map::new(fst_mmap)?;
        
        // 内存映射 Bitmap 文件
        let bitmap_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&File::open(bitmap_file)?)?
        };
        
        // 加载 delta 索引和删除 bitmap，构建初始 DeltaState
        let delta_grams = Self::load_delta_index(drive_letter, output_dir)
            .map(|d| d.gram_bitmaps)
            .unwrap_or_default();
        let deleted_bitmap = Self::load_deleted_bitmap(drive_letter, output_dir)
            .unwrap_or_default();
        let delta_state = Arc::new(RwLock::new(DeltaState {
            gram_bitmaps: delta_grams,
            deleted_bitmap,
        }));
        
        // 读取当前版本号
        let loaded_version = Self::read_version(drive_letter, output_dir);
        
        let query = Self {
            drive_letter,
            output_dir: output_dir.to_string(),
            fst_map,
            bitmap_mmap,
            delta_state,
            loaded_version,
        };
        
        // 🔥 预热 mmap 数据 (触发 OS 加载页表到物理内存)
        query.warmup_mmap()?;
        
        tracing::info!("✓ Index opened for drive {} in {:.2}ms", drive_letter, open_start.elapsed().as_secs_f64() * 1000.0);
        
        Ok(query)
    }

    /// 加载删除 bitmap（{drive}_deleted.dat）
    fn load_deleted_bitmap(drive_letter: char, output_dir: &str) -> Option<RoaringBitmap> {
        let deleted_file = format!("{}\\{}_deleted.dat", output_dir, drive_letter);
        let bytes = std::fs::read(&deleted_file).ok()?;
        RoaringBitmap::deserialize_from(&bytes[..]).ok()
    }

    /// 返回共享 DeltaState 句柄（供 UsnIncrementalUpdater 持有）
    ///
    /// 用法：
    ///   let handle = index_query.delta_state_handle();
    ///   usn_updater.attach_index(handle, path_reader.delta_paths_handle());
    pub fn delta_state_handle(&self) -> Arc<RwLock<DeltaState>> {
        Arc::clone(&self.delta_state)
    }

    /// 热重载 delta（从磁盘重读，用于重启后患复场景）
    /// 正常运行时用 UsnIncrementalUpdater.attach_index() 即可，无需此方法
    pub fn hot_reload_delta(&self) -> Result<()> {
        let new_grams = Self::load_delta_index(self.drive_letter, &self.output_dir)
            .map(|d| d.gram_bitmaps)
            .unwrap_or_default();
        let new_deleted = Self::load_deleted_bitmap(self.drive_letter, &self.output_dir)
            .unwrap_or_default();
        let mut state = self.delta_state.write().unwrap();
        state.gram_bitmaps = new_grams;
        state.deleted_bitmap = new_deleted;
        tracing::info!("✓ Delta hot-reloaded from disk for drive {}", self.drive_letter);
        Ok(())
    }
    
    /// 预热 mmap 映射的数据（强制 OS 加载到物理内存）
    fn warmup_mmap(&self) -> Result<()> {
        let warmup_start = std::time::Instant::now();
        
        // 🔥 方法 1: 顺序访问 mmap 数据 (每 4KB 读取一个字节)
        // 这会触发页表加载，避免首次查询时的缺页中断
        
        // 预热 FST (通常 < 10MB)
        let fst_bytes = self.fst_map.as_fst().as_bytes();
        let fst_len = fst_bytes.len();
        let mut fst_sum: u64 = 0;
        
        // 每隔 4KB (页大小) 访问一次
        const PAGE_SIZE: usize = 4096;
        for offset in (0..fst_len).step_by(PAGE_SIZE) {
            fst_sum = fst_sum.wrapping_add(fst_bytes[offset] as u64);
        }
        
        // 预热 Bitmap (可能较大，采样访问避免过慢)
        let bitmap_len = self.bitmap_mmap.len();
        let mut bitmap_sum: u64 = 0;
        
        // 🔥 优化：大文件只采样前 50MB（避免启动时过慢）
        const MAX_WARMUP_SIZE: usize = 50 * 1024 * 1024; // 50MB
        let warmup_len = bitmap_len.min(MAX_WARMUP_SIZE);
        
        for offset in (0..warmup_len).step_by(PAGE_SIZE) {
            bitmap_sum = bitmap_sum.wrapping_add(self.bitmap_mmap[offset] as u64);
        }
        
        // 防止编译器优化掉这些访问
        std::hint::black_box(fst_sum);
        std::hint::black_box(bitmap_sum);
        
        let warmup_elapsed = warmup_start.elapsed().as_secs_f64() * 1000.0;
        
        if warmup_elapsed > 100.0 {
            tracing::info!(
                "🔥 Warmup for drive {}: FST={:.2}MB, Bitmap={:.2}MB (sampled {:.2}MB) in {:.2}ms",
                self.drive_letter,
                fst_len as f64 / 1_048_576.0,
                bitmap_len as f64 / 1_048_576.0,
                warmup_len as f64 / 1_048_576.0,
                warmup_elapsed
            );
        }
        
        Ok(())
    }
    
    /// 读取索引版本号
    fn read_version(drive_letter: char, output_dir: &str) -> u64 {
        let version_file = format!("{}\\{}_index.version", output_dir, drive_letter);
        
        if let Ok(content) = std::fs::read_to_string(&version_file) {
            content.trim().parse::<u64>().unwrap_or(0)
        } else {
            0
        }
    }
    
    /// 检查索引是否需要重新加载（版本号已变化）
    pub fn needs_reload(&self) -> bool {
        let current_version = Self::read_version(self.drive_letter, &self.output_dir);
        current_version > self.loaded_version
    }
    
    /// 重新加载索引（热重载）
    pub fn reload(&mut self) -> Result<()> {
        tracing::info!("🔄 Reloading index for drive {} (version changed)...", self.drive_letter);
        
        let fst_file = format!("{}\\{}_index.fst", self.output_dir, self.drive_letter);
        let bitmap_file = format!("{}\\{}_bitmaps.dat", self.output_dir, self.drive_letter);
        
        // 重新映射 FST
        let fst_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&File::open(fst_file)?)?
        };
        self.fst_map = Map::new(fst_mmap)?;
        
        // 重新映射 Bitmap
        self.bitmap_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&File::open(bitmap_file)?)?
        };
        
        // 重新加载 delta 索引（更新共享状态）
        let new_grams = Self::load_delta_index(self.drive_letter, &self.output_dir)
            .map(|d| d.gram_bitmaps)
            .unwrap_or_default();
        let new_deleted = Self::load_deleted_bitmap(self.drive_letter, &self.output_dir)
            .unwrap_or_default();
        {
            let mut state = self.delta_state.write().unwrap();
            state.gram_bitmaps = new_grams;
            state.deleted_bitmap = new_deleted;
        }
        
        // 更新版本号
        self.loaded_version = Self::read_version(self.drive_letter, &self.output_dir);
        
        // 🔥 预热新加载的 mmap 数据
        self.warmup_mmap()?;
        
        tracing::info!("✓ Index reloaded (version: {})", self.loaded_version);
        
        Ok(())
    }
    
    /// 加载 delta 索引文件
    fn load_delta_index(drive_letter: char, output_dir: &str) -> Result<DeltaState> {
        let delta_file = format!("{}\\{}_index_delta.dat", output_dir, drive_letter);
        
        if !std::path::Path::new(&delta_file).exists() {
            return Err(anyhow::anyhow!("Delta index not found"));
        }
        
        let mut file = std::fs::File::open(delta_file)?;
        let mut gram_bitmaps: FxHashMap<Vec<u8>, RoaringBitmap> = FxHashMap::default();
        
        use std::io::Read;
        
        loop {
            // 读取 gram 长度
            let mut len_buf = [0u8; 4];
            if file.read_exact(&mut len_buf).is_err() {
                break; // EOF
            }
            let gram_len = u32::from_le_bytes(len_buf) as usize;
            
            // 读取 gram 内容
            let mut gram_bytes = vec![0u8; gram_len];
            file.read_exact(&mut gram_bytes)?;
            // gram_bytes 直接用作 Vec<u8> key（与 USN updater 写入的 UTF-8 bytes 完全相同）
            
            // 读取 bitmap 长度
            let mut bitmap_len_buf = [0u8; 4];
            file.read_exact(&mut bitmap_len_buf)?;
            let bitmap_len = u32::from_le_bytes(bitmap_len_buf) as usize;
            
            // 读取 bitmap 数据
            let mut bitmap_bytes = vec![0u8; bitmap_len];
            file.read_exact(&mut bitmap_bytes)?;
            
            // 反序列化 bitmap
            let bitmap = RoaringBitmap::deserialize_from(&bitmap_bytes[..])?;
            
            // 合并到 delta 索引（如果已存在则并集）
            gram_bitmaps.entry(gram_bytes)
                .and_modify(|existing| *existing |= bitmap.clone())
                .or_insert(bitmap);
        }
        
        // 🔥 降低日志级别，避免每次查询都输出（仅在首次加载时输出）
        tracing::debug!("✓ Loaded delta index: {} unique grams", gram_bitmaps.len());
        
        Ok(DeltaState { gram_bitmaps, deleted_bitmap: RoaringBitmap::new() })
    }
    
    /// 查询关键词（< 30ms，支持 delta）
    pub fn search(&self, keyword: &str, limit: usize) -> Result<Vec<u32>> {
        let query_start = std::time::Instant::now();
        
        let keyword_lower = keyword.to_lowercase();
        let query_bytes = keyword_lower.as_bytes();

        if query_bytes.is_empty() {
            return Ok(Vec::new());
        }

        let result_bitmap = if query_bytes.len() < 3 {
            // ── Short query (1-2 bytes): FST prefix search ──────────────────
            // Union all bitmaps whose gram starts with the keyword
            let state = self.delta_state.read().unwrap();
            self.prefix_search_bitmap(&keyword_lower, &state)?
        } else {
            // ── Normal query (≥3 bytes): 3-gram intersection ─────────────────
            let query_grams = Self::split_to_3grams_bytes(&keyword_lower);
            let state = self.delta_state.read().unwrap();

            let mut bitmaps = Vec::with_capacity(query_grams.len());
            for gram in &query_grams {
                // 从主索引查询（gram: &Vec<u8>，传入 as_slice() 展开 AsRef<[u8]>）
                let mut bitmap = if let Some(offset) = self.fst_map.get(gram.as_slice()) {
                    self.load_bitmap(offset)?.unwrap_or_else(RoaringBitmap::new)
                } else {
                    RoaringBitmap::new()
                };

                // 从共享 delta 状态查询并合并
                if let Some(delta_bitmap) = state.gram_bitmaps.get(gram.as_slice()) {
                    bitmap |= delta_bitmap;
                }
                
                if bitmap.is_empty() {
                    return Ok(Vec::new());
                }
                
                bitmaps.push(bitmap);
            }
            
            if bitmaps.len() == 1 {
                bitmaps.into_iter().next().unwrap()
            } else {
                bitmaps.into_iter().reduce(|a, b| a & b).unwrap()
            }
        };

        // ── 过滤已删除文件 ────────────────────────────────────────────────────
        let result_bitmap = {
            let state = self.delta_state.read().unwrap();
            if !state.deleted_bitmap.is_empty() {
                &result_bitmap - &state.deleted_bitmap
            } else {
                result_bitmap
            }
        };
        
        let results: Vec<u32> = result_bitmap.iter().take(limit).collect();
        
        let elapsed = query_start.elapsed();
        tracing::debug!(
            "search: '{}' ({} bytes) -> {} results in {:.2}ms",
            keyword,
            query_bytes.len(),
            results.len(),
            elapsed.as_secs_f64() * 1000.0,
        );
        
        Ok(results)
    }

    /// FST 前缀搜索（1-2 字符查询用）：union 所有以 prefix 开头的 gram 的 bitmap
    fn prefix_search_bitmap(&self, prefix: &str, state: &DeltaState) -> Result<RoaringBitmap> {
        use fst::automaton::Str;
        use fst::{Automaton, IntoStreamer};

        let auto = Str::new(prefix).starts_with();
        let mut stream = self.fst_map.search(&auto).into_stream();
        
        let mut union_bitmap = RoaringBitmap::new();
        
        use fst::Streamer;
        while let Some((_key, offset)) = stream.next() {
            if let Some(bmp) = self.load_bitmap(offset)? {
                union_bitmap |= bmp;
            }
        }

        // 同时查 delta
        let prefix_bytes = prefix.as_bytes();
        for (gram_bytes, bmp) in &state.gram_bitmaps {
            if gram_bytes.starts_with(prefix_bytes) {
                union_bitmap |= bmp;
            }
        }

        Ok(union_bitmap)
    }
    
    /// 拆分为 3-字节 gram（基于 UTF-8 字节窗口）
    fn split_to_3grams_bytes(text: &str) -> Vec<Vec<u8>> {
        let b = text.as_bytes();
        if b.len() < 3 {
            return vec![b.to_vec()];
        }
        b.windows(3).map(|w| w.to_vec()).collect()
    }
    
    /// 从内存映射加载 bitmap
    fn load_bitmap(&self, offset: u64) -> Result<Option<RoaringBitmap>> {
        let offset = offset as usize;
        
        if offset + 4 > self.bitmap_mmap.len() {
            return Ok(None);
        }
        
        // 读取长度
        let len_bytes: [u8; 4] = self.bitmap_mmap[offset..offset + 4].try_into()?;
        let len = u32::from_le_bytes(len_bytes) as usize;
        
        if offset + 4 + len > self.bitmap_mmap.len() {
            return Ok(None);
        }
        
        // 反序列化 bitmap
        let bitmap_bytes = &self.bitmap_mmap[offset + 4..offset + 4 + len];
        let bitmap = RoaringBitmap::deserialize_from(bitmap_bytes)?;
        
        Ok(Some(bitmap))
    }
}

/// 路径读取器（从 .dat 文件读取路径）
pub struct PathReader {
    #[allow(dead_code)]
    drive_letter: char,
    paths_mmap: memmap2::Mmap,
    offset_index: Vec<usize>,  // 🔥 新增: 文件ID -> 偏移量索引
    /// USN 新增文件的路径（主索引固定了，这些路径内存持有）
    pub delta_paths: Arc<RwLock<HashMap<u32, String>>>,
}

impl PathReader {
    pub fn open(drive_letter: char, output_dir: &str) -> Result<Self> {
        let paths_file = format!("{}\\{}_paths.dat", output_dir, drive_letter);
        let offset_file = format!("{}\\{}_offsets.dat", output_dir, drive_letter);
        
        let paths_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&File::open(&paths_file)?)?
        };
        
        // 🔥 从文件加载偏移量索引（避免重复扫描）
        let start = std::time::Instant::now();
        
        let offset_index = if std::path::Path::new(&offset_file).exists() {
            // 优先从文件加载
            Self::load_offset_index(&offset_file)?
        } else {
            // 降级：现场构建（向后兼容）
            tracing::warn!("⚠️  Offset file not found, building on-the-fly (slower)");
            Self::build_offset_index(&paths_mmap)?
        };
        
        let elapsed = start.elapsed();
        tracing::debug!(
            "✓ Loaded offset index for drive {}: {} entries in {:.2}ms",
            drive_letter,
            offset_index.len(),
            elapsed.as_secs_f64() * 1000.0
        );
        
        Ok(Self {
            drive_letter,
            paths_mmap,
            offset_index,
            delta_paths: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// 从文件加载偏移量索引
    fn load_offset_index(offset_file: &str) -> Result<Vec<usize>> {
        let mut reader = BufReader::new(File::open(offset_file)?);
        
        // 读取文件数量
        let mut count_buf = [0u8; 4];
        reader.read_exact(&mut count_buf)?;
        let count = u32::from_le_bytes(count_buf) as usize;
        
        // 读取所有偏移量
        let mut index = Vec::with_capacity(count);
        let mut offset_buf = [0u8; 8];
        
        for _ in 0..count {
            reader.read_exact(&mut offset_buf)?;
            let offset = u64::from_le_bytes(offset_buf) as usize;
            index.push(offset);
        }
        
        Ok(index)
    }
    
    /// 构建偏移量索引
    fn build_offset_index(mmap: &memmap2::Mmap) -> Result<Vec<usize>> {
        let mut index = Vec::new();
        let mut offset = 0usize;
        
        while offset + 4 <= mmap.len() {
            // 记录当前文件的起始偏移
            index.push(offset);
            
            // 读取路径长度
            let len_bytes: [u8; 4] = mmap[offset..offset + 4].try_into()?;
            let path_len = u32::from_le_bytes(len_bytes) as usize;
            
            // 跳到下一个文件
            offset += 4 + path_len;
        }
        
        Ok(index)
    }
    
    /// 返回共享 delta_paths 句柄（供 UsnIncrementalUpdater 持有）
    pub fn delta_paths_handle(&self) -> Arc<RwLock<HashMap<u32, String>>> {
        Arc::clone(&self.delta_paths)
    }

    /// 根据文件ID读取路径（O(1) 访问）— 主索引优先，则 fallback 到 delta_paths
    pub fn get_path(&self, file_id: u32) -> Result<String> {
        let idx = file_id as usize;
        
        if idx < self.offset_index.len() {
            // 主索引 O(1)
            let offset = self.offset_index[idx];
            
            if offset + 4 > self.paths_mmap.len() {
                return Err(anyhow::anyhow!("Invalid offset"));
            }
            
            let len_bytes: [u8; 4] = self.paths_mmap[offset..offset + 4].try_into()?;
            let path_len = u32::from_le_bytes(len_bytes) as usize;
            
            let data_offset = offset + 4;
            if data_offset + path_len > self.paths_mmap.len() {
                return Err(anyhow::anyhow!("Invalid path length"));
            }
            
            let path_bytes = &self.paths_mmap[data_offset..data_offset + path_len];
            Ok(String::from_utf8_lossy(path_bytes).to_string())
        } else {
            // USN 新增路径 fallback
            self.delta_paths
                .read()
                .unwrap()
                .get(&file_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File ID {} not found", file_id))
        }
    }
    
    /// 批量读取路径（性能优化）
    pub fn get_paths(&self, file_ids: &[u32]) -> Result<Vec<String>> {
        let mut results = Vec::with_capacity(file_ids.len());
        
        for &id in file_ids {
            if let Ok(path) = self.get_path(id) {
                results.push(path);
            }
        }
        
        Ok(results)
    }
}
