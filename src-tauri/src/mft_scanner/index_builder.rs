// 3-Gram 倒排索引构建器 - 基于 prompt.txt 方案
// 使用 FST + RoaringBitmap 实现极致压缩

use anyhow::Result;
use fst::{Map, MapBuilder};
use roaring::RoaringBitmap;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use tracing::info;

/// 3-Gram 索引构建器
pub struct IndexBuilder {
    drive_letter: char,
    gram_index: HashMap<String, RoaringBitmap>,  // 3-gram -> 文件ID位图
    total_grams: usize,
}

impl IndexBuilder {
    pub fn new(drive_letter: char) -> Self {
        Self {
            drive_letter,
            gram_index: HashMap::with_capacity(1_000_000),  // 预分配 100万 3-gram
            total_grams: 0,
        }
    }
    
    /// 从路径文件构建索引
    pub fn build_from_paths(&mut self, output_dir: &str) -> Result<()> {
        info!("🔍 Building 3-gram index for drive {}:", self.drive_letter);
        
        let paths_file = format!("{}\\{}_paths.dat", output_dir, self.drive_letter);
        let mut reader = BufReader::with_capacity(
            32 * 1024 * 1024,
            File::open(paths_file)?,
        );
        
        let mut path_id: u32 = 0;
        let mut len_buf = [0u8; 4];
        
        // 🔥 同时构建 offset index（避免后续重复扫描）
        let mut offset_index = Vec::new();
        let mut current_offset = 0usize;
        
        // 流式读取路径并构建 3-gram
        while reader.read_exact(&mut len_buf).is_ok() {
            // 记录当前文件的起始偏移
            offset_index.push(current_offset);
            
            let path_len = u32::from_le_bytes(len_buf) as usize;
            
            let mut path_bytes = vec![0u8; path_len];
            reader.read_exact(&mut path_bytes)?;
            
            let path = String::from_utf8_lossy(&path_bytes);
            
            // 提取文件名（最后一个 \ 之后）
            let filename = path.rsplit('\\').next().unwrap_or(&path);
            let filename_lower = filename.to_lowercase();
            
            // 生成 3-gram
            self.add_3grams(&filename_lower, path_id);
            
            // 更新偏移量
            current_offset += 4 + path_len;
            path_id += 1;
            
            if path_id % 100_000 == 0 {
                info!("   Progress: {} files processed, {} unique grams", path_id, self.gram_index.len());
            }
        }
        
        self.total_grams = self.gram_index.len();
        info!("✓ Index built: {} files, {} unique 3-grams", path_id, self.total_grams);
        
        // 🔥 保存 offset index 到文件
        let offset_file = format!("{}\\{}_offsets.dat", output_dir, self.drive_letter);
        let mut offset_writer = BufWriter::new(File::create(offset_file)?);
        
        // 写入文件数量
        offset_writer.write_all(&(offset_index.len() as u32).to_le_bytes())?;
        
        // 写入所有偏移量
        for offset in &offset_index {
            offset_writer.write_all(&(*offset as u64).to_le_bytes())?;
        }
        offset_writer.flush()?;
        
        Ok(())
    }
    
    /// 添加 3-gram
    fn add_3grams(&mut self, text: &str, file_id: u32) {
        // 🔥 关键优化：使用滑动窗口生成 3-gram
        if text.len() < 3 {
            // 对于短文件名，直接用完整名称
            self.gram_index
                .entry(text.to_string())
                .or_insert_with(RoaringBitmap::new)
                .insert(file_id);
            return;
        }
        
        // 生成所有 3-gram
        let chars: Vec<char> = text.chars().collect();
        for window in chars.windows(3) {
            let gram: String = window.iter().collect();
            
            self.gram_index
                .entry(gram)
                .or_insert_with(RoaringBitmap::new)
                .insert(file_id);
        }
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
            // FST 记录：gram -> bitmap在文件中的偏移量
            fst_builder.insert(gram, current_offset)?;
            
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

/// 索引查询器（零拷贝，内存映射）
pub struct IndexQuery {
    #[allow(dead_code)]
    drive_letter: char,
    fst_map: Map<memmap2::Mmap>,
    bitmap_mmap: memmap2::Mmap,
    delta_index: Option<DeltaIndex>,  // 增量索引
}

/// Delta 索引（内存中的增量更新）
struct DeltaIndex {
    gram_bitmaps: HashMap<String, RoaringBitmap>,
}

impl IndexQuery {
    /// 打开索引（零拷贝加载）
    pub fn open(drive_letter: char, output_dir: &str) -> Result<Self> {
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
        
        // 加载 delta 索引（如果存在）
        let delta_index = Self::load_delta_index(drive_letter, output_dir).ok();
        
        Ok(Self {
            drive_letter,
            fst_map,
            bitmap_mmap,
            delta_index,
        })
    }
    
    /// 加载 delta 索引文件
    fn load_delta_index(drive_letter: char, output_dir: &str) -> Result<DeltaIndex> {
        let delta_file = format!("{}\\{}_index_delta.dat", output_dir, drive_letter);
        
        if !std::path::Path::new(&delta_file).exists() {
            return Err(anyhow::anyhow!("Delta index not found"));
        }
        
        let mut file = std::fs::File::open(delta_file)?;
        let mut gram_bitmaps = HashMap::new();
        
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
            let gram = String::from_utf8(gram_bytes)?;
            
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
            gram_bitmaps.entry(gram)
                .and_modify(|existing| *existing |= bitmap.clone())
                .or_insert(bitmap);
        }
        
        // 🔥 降低日志级别，避免每次查询都输出（仅在首次加载时输出）
        tracing::debug!("✓ Loaded delta index: {} unique grams", gram_bitmaps.len());
        
        Ok(DeltaIndex { gram_bitmaps })
    }
    
    /// 查询关键词（< 30ms，支持 delta）
    pub fn search(&self, keyword: &str, limit: usize) -> Result<Vec<u32>> {
        let query_start = std::time::Instant::now();
        
        let keyword_lower = keyword.to_lowercase();
        
        // 🔥 步骤 1: 将查询拆分为 3-gram（约 0.1ms）
        let query_grams = self.split_to_3grams(&keyword_lower);
        
        if query_grams.is_empty() {
            return Ok(Vec::new());
        }
        
        // 🔥 步骤 2: 查找每个 gram 的 bitmap（约 1-2ms）
        let mut bitmaps = Vec::new();
        for gram in &query_grams {
            // 从主索引查询
            let mut bitmap = if let Some(offset) = self.fst_map.get(gram) {
                self.load_bitmap(offset)?.unwrap_or_else(RoaringBitmap::new)
            } else {
                RoaringBitmap::new()
            };
            
            // 🔥 从 delta 索引查询并合并
            if let Some(delta) = &self.delta_index {
                if let Some(delta_bitmap) = delta.gram_bitmaps.get(gram) {
                    bitmap |= delta_bitmap;
                }
            }
            
            // 如果合并后仍为空，说明没有结果
            if bitmap.is_empty() {
                return Ok(Vec::new());
            }
            
            bitmaps.push(bitmap);
        }
        
        // 🔥 步骤 3: 快速交集运算（约 1-5ms）
        let result_bitmap = if bitmaps.len() == 1 {
            bitmaps.into_iter().next().unwrap()
        } else {
            // 多个 bitmap 交集
            bitmaps.into_iter().reduce(|a, b| a & b).unwrap()
        };
        
        // 🔥 步骤 4: 转换为 Vec（约 1-2ms）
        let results: Vec<u32> = result_bitmap.iter().take(limit).collect();
        
        let elapsed = query_start.elapsed();
        tracing::debug!(
            "3-gram search: '{}' -> {} results in {:.2}ms (delta: {})",
            keyword,
            results.len(),
            elapsed.as_secs_f64() * 1000.0,
            self.delta_index.is_some()
        );
        
        Ok(results)
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
    
    /// 根据文件ID读取路径（O(1) 访问）
    pub fn get_path(&self, file_id: u32) -> Result<String> {
        let file_id = file_id as usize;
        
        if file_id >= self.offset_index.len() {
            return Err(anyhow::anyhow!("File ID {} out of range", file_id));
        }
        
        let offset = self.offset_index[file_id];
        
        if offset + 4 > self.paths_mmap.len() {
            return Err(anyhow::anyhow!("Invalid offset"));
        }
        
        // 读取路径长度
        let len_bytes: [u8; 4] = self.paths_mmap[offset..offset + 4].try_into()?;
        let path_len = u32::from_le_bytes(len_bytes) as usize;
        
        let data_offset = offset + 4;
        if data_offset + path_len > self.paths_mmap.len() {
            return Err(anyhow::anyhow!("Invalid path length"));
        }
        
        // 读取路径
        let path_bytes = &self.paths_mmap[data_offset..data_offset + path_len];
        Ok(String::from_utf8_lossy(path_bytes).to_string())
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
