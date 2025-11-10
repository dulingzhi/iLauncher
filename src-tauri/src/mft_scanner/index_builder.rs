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
        
        // 流式读取路径并构建 3-gram
        while reader.read_exact(&mut len_buf).is_ok() {
            let path_len = u32::from_le_bytes(len_buf) as usize;
            
            let mut path_bytes = vec![0u8; path_len];
            reader.read_exact(&mut path_bytes)?;
            
            let path = String::from_utf8_lossy(&path_bytes);
            
            // 提取文件名（最后一个 \ 之后）
            let filename = path.rsplit('\\').next().unwrap_or(&path);
            let filename_lower = filename.to_lowercase();
            
            // 生成 3-gram
            self.add_3grams(&filename_lower, path_id);
            
            path_id += 1;
            
            if path_id % 100_000 == 0 {
                info!("   Progress: {} files processed, {} unique grams", path_id, self.gram_index.len());
            }
        }
        
        self.total_grams = self.gram_index.len();
        info!("✓ Index built: {} files, {} unique 3-grams", path_id, self.total_grams);
        
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
    drive_letter: char,
    fst_map: Map<memmap2::Mmap>,
    bitmap_mmap: memmap2::Mmap,
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
        
        Ok(Self {
            drive_letter,
            fst_map,
            bitmap_mmap,
        })
    }
    
    /// 查询关键词（< 30ms）
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
            if let Some(offset) = self.fst_map.get(gram) {
                if let Some(bitmap) = self.load_bitmap(offset)? {
                    bitmaps.push(bitmap);
                }
            } else {
                // 任意一个 gram 不存在，直接返回空
                return Ok(Vec::new());
            }
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
            "3-gram search: '{}' -> {} results in {:.2}ms",
            keyword,
            results.len(),
            elapsed.as_secs_f64() * 1000.0
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
    drive_letter: char,
    paths_mmap: memmap2::Mmap,
}

impl PathReader {
    pub fn open(drive_letter: char, output_dir: &str) -> Result<Self> {
        let paths_file = format!("{}\\{}_paths.dat", output_dir, drive_letter);
        
        let paths_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&File::open(paths_file)?)?
        };
        
        Ok(Self {
            drive_letter,
            paths_mmap,
        })
    }
    
    /// 根据文件ID读取路径
    pub fn get_path(&self, file_id: u32) -> Result<String> {
        let mut offset = 0usize;
        let mut current_id = 0u32;
        
        // 🔥 优化：如果有索引文件，可以直接跳转
        // 这里简化实现，顺序查找
        while offset < self.paths_mmap.len() {
            if offset + 4 > self.paths_mmap.len() {
                break;
            }
            
            // 读取路径长度
            let len_bytes: [u8; 4] = self.paths_mmap[offset..offset + 4].try_into()?;
            let path_len = u32::from_le_bytes(len_bytes) as usize;
            
            offset += 4;
            
            if current_id == file_id {
                // 找到目标路径
                if offset + path_len > self.paths_mmap.len() {
                    break;
                }
                
                let path_bytes = &self.paths_mmap[offset..offset + path_len];
                return Ok(String::from_utf8_lossy(path_bytes).to_string());
            }
            
            offset += path_len;
            current_id += 1;
        }
        
        Err(anyhow::anyhow!("File ID {} not found", file_id))
    }
    
    /// 批量读取路径（性能优化）
    pub fn get_paths(&self, file_ids: &[u32]) -> Result<Vec<String>> {
        // 🔥 TODO: 优化为跳表或索引查找
        // 当前简化实现：顺序扫描
        let mut results = Vec::with_capacity(file_ids.len());
        
        for &id in file_ids {
            if let Ok(path) = self.get_path(id) {
                results.push(path);
            }
        }
        
        Ok(results)
    }
}
