// Delta 索引合并器 - 后台任务定期合并增量索引到主索引

use anyhow::Result;
use roaring::RoaringBitmap;
use fst::{Map, MapBuilder, Streamer};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write, BufWriter};
use std::path::Path;
use std::time::Duration;
use tracing::{info, error, debug};

/// Delta 索引合并器
pub struct DeltaMerger {
    drive_letter: char,
    output_dir: String,
    merge_threshold_mb: u64,  // Delta 文件超过此大小时触发合并
}

impl DeltaMerger {
    pub fn new(drive_letter: char, output_dir: String) -> Self {
        Self {
            drive_letter,
            output_dir,
            merge_threshold_mb: 50,  // 默认 50MB
        }
    }
    
    /// 检查是否需要合并
    pub fn should_merge(&self) -> bool {
        let delta_file = format!("{}\\{}_index_delta.dat", self.output_dir, self.drive_letter);
        
        if let Ok(metadata) = std::fs::metadata(&delta_file) {
            let size_mb = metadata.len() / 1024 / 1024;
            debug!("Delta index size: {} MB", size_mb);
            return size_mb >= self.merge_threshold_mb;
        }
        
        false
    }
    
    /// 执行合并（重建 FST + RoaringBitmap）
    pub fn merge(&self) -> Result<()> {
        info!("🔄 Starting delta index merge for drive {}...", self.drive_letter);
        let start = std::time::Instant::now();
        
        // 1. 加载现有主索引
        let mut main_index = self.load_main_index()?;
        
        // 2. 加载 delta 索引
        let delta_index = self.load_delta_index()?;
        
        // 3. 合并 bitmap
        for (gram, delta_bitmap) in delta_index {
            main_index.entry(gram)
                .and_modify(|existing| *existing |= delta_bitmap.clone())
                .or_insert(delta_bitmap);
        }
        
        // 4. 重建 FST + Bitmap 文件
        self.rebuild_index(&main_index)?;
        
        // 5. 删除 delta 文件
        self.cleanup_delta()?;
        
        let elapsed = start.elapsed();
        info!("✓ Delta merge completed in {:.2}s", elapsed.as_secs_f64());
        
        Ok(())
    }
    
    /// 加载主索引
    fn load_main_index(&self) -> Result<HashMap<String, RoaringBitmap>> {
        let fst_file = format!("{}\\{}_index.fst", self.output_dir, self.drive_letter);
        let bitmap_file = format!("{}\\{}_bitmaps.dat", self.output_dir, self.drive_letter);
        
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
        
        // 重建 HashMap
        let mut index = HashMap::new();
        let mut stream = fst_map.stream();
        
        while let Some((gram_bytes, offset)) = stream.next() {
            let gram = String::from_utf8(gram_bytes.to_vec())?;
            
            // 读取 bitmap
            if let Some(bitmap) = self.read_bitmap_at_offset(&bitmap_mmap, offset)? {
                index.insert(gram, bitmap);
            }
        }
        
        info!("✓ Loaded main index: {} grams", index.len());
        
        Ok(index)
    }
    
    /// 从偏移量读取 bitmap
    fn read_bitmap_at_offset(&self, mmap: &memmap2::Mmap, offset: u64) -> Result<Option<RoaringBitmap>> {
        let offset = offset as usize;
        
        if offset + 4 > mmap.len() {
            return Ok(None);
        }
        
        // 读取长度
        let len = u32::from_le_bytes([
            mmap[offset],
            mmap[offset + 1],
            mmap[offset + 2],
            mmap[offset + 3],
        ]) as usize;
        
        if offset + 4 + len > mmap.len() {
            return Ok(None);
        }
        
        // 反序列化 bitmap
        let bitmap = RoaringBitmap::deserialize_from(&mmap[offset + 4..offset + 4 + len])?;
        
        Ok(Some(bitmap))
    }
    
    /// 加载 delta 索引
    fn load_delta_index(&self) -> Result<HashMap<String, RoaringBitmap>> {
        let delta_file = format!("{}\\{}_index_delta.dat", self.output_dir, self.drive_letter);
        
        let mut file = File::open(delta_file)?;
        let mut delta_index = HashMap::new();
        
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
            
            // 合并
            delta_index.entry(gram)
                .and_modify(|existing| *existing |= bitmap.clone())
                .or_insert(bitmap);
        }
        
        info!("✓ Loaded delta index: {} grams", delta_index.len());
        
        Ok(delta_index)
    }
    
    /// 重建索引文件
    fn rebuild_index(&self, index: &HashMap<String, RoaringBitmap>) -> Result<()> {
        info!("📝 Rebuilding index files...");
        
        let fst_file = format!("{}\\{}_index.fst", self.output_dir, self.drive_letter);
        let bitmap_file = format!("{}\\{}_bitmaps.dat", self.output_dir, self.drive_letter);
        
        // 排序所有 gram（FST 需要有序）
        let mut sorted_grams: Vec<_> = index.iter().collect();
        sorted_grams.sort_by(|a, b| a.0.cmp(b.0));
        
        // 构建 FST
        let mut fst_builder = MapBuilder::new(BufWriter::new(File::create(&fst_file)?))?;
        let mut bitmap_writer = BufWriter::new(File::create(&bitmap_file)?);
        
        let mut current_offset: u64 = 0;
        
        for (gram, bitmap) in sorted_grams {
            // 写入 FST 映射
            fst_builder.insert(gram.as_bytes(), current_offset)?;
            
            // 序列化 bitmap
            let mut bitmap_bytes = Vec::new();
            bitmap.serialize_into(&mut bitmap_bytes)?;
            
            // 写入长度前缀
            bitmap_writer.write_all(&(bitmap_bytes.len() as u32).to_le_bytes())?;
            
            // 写入 bitmap 数据
            bitmap_writer.write_all(&bitmap_bytes)?;
            
            current_offset += 4 + bitmap_bytes.len() as u64;
        }
        
        fst_builder.finish()?;
        bitmap_writer.flush()?;
        
        info!("✓ Index files rebuilt");
        
        Ok(())
    }
    
    /// 清理 delta 文件
    fn cleanup_delta(&self) -> Result<()> {
        let delta_file = format!("{}\\{}_index_delta.dat", self.output_dir, self.drive_letter);
        
        if Path::new(&delta_file).exists() {
            std::fs::remove_file(&delta_file)?;
            info!("✓ Delta file removed");
        }
        
        Ok(())
    }
    
    /// 后台定期检查并合并
    pub fn start_background_merge(drive_letter: char, output_dir: String) {
        std::thread::spawn(move || {
            let merger = DeltaMerger::new(drive_letter, output_dir);
            
            loop {
                // 每 5 分钟检查一次
                std::thread::sleep(Duration::from_secs(300));
                
                if merger.should_merge() {
                    info!("🔔 Delta index threshold reached, starting merge...");
                    
                    if let Err(e) = merger.merge() {
                        error!("❌ Delta merge failed: {:#}", e);
                    }
                }
            }
        });
    }
}
