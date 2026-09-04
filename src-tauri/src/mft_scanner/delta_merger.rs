// Delta 索引合并器 - 后台任务定期合并增量索引到主索引

use anyhow::Result;
use roaring::RoaringBitmap;
use fst::{Map, MapBuilder, Streamer};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, BufWriter, SeekFrom};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tracing::{info, error, debug};

/// 全局 flush/merge 互斥锁（Service 进程内）
///
/// UsnIncrementalUpdater::flush_index_cache（监控线程，每 30s/1000gram 触发）与
/// DeltaMerger::merge（后台合并线程，每 5min 检查）必须互斥，否则：
/// - merge 读 delta 文件时可能读到写了一半的记录；
/// - merge 重建 offsets.dat 后删除 offsets_delta.dat，会吞掉重建期间
///   updater 追加的路径偏移量，导致这些 file_id 在 UI 侧无法解析。
pub static INDEX_IO_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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
        // 🔥 与 USN updater 的 flush 互斥（见 INDEX_IO_LOCK 说明）
        let _io_guard = INDEX_IO_LOCK.lock().unwrap();

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

        // 4. 重建 FST + Bitmap 文件（使用临时文件避免文件锁）
        self.rebuild_index(&main_index)?;

        // 5. 把 USN 追加的路径偏移量折入主 offsets.dat，并删除 offsets_delta
        //    （必须在 bump 版本号之前完成，UI 全量重载时两者需一致）
        self.rebuild_offsets_index()?;
        self.cleanup_offsets_delta()?;

        // 6. 删除 delta 文件
        self.cleanup_delta()?;

        // 7. 更新版本号（通知 UI 重新加载）
        self.increment_version()?;

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
    
    /// 重建索引文件（直接覆盖，Windows 允许 rename 覆盖被 mmap 的文件）
    fn rebuild_index(&self, index: &HashMap<String, RoaringBitmap>) -> Result<()> {
        info!("📝 Rebuilding index files...");
        
        // 使用 .new 后缀的临时文件
        let fst_file_new = format!("{}\\{}_index.fst.new", self.output_dir, self.drive_letter);
        let bitmap_file_new = format!("{}\\{}_bitmaps.dat.new", self.output_dir, self.drive_letter);
        
        // 最终目标文件
        let fst_file = format!("{}\\{}_index.fst", self.output_dir, self.drive_letter);
        let bitmap_file = format!("{}\\{}_bitmaps.dat", self.output_dir, self.drive_letter);
        
        // 排序所有 gram（FST 需要有序）
        let mut sorted_grams: Vec<_> = index.iter().collect();
        sorted_grams.sort_by(|a, b| a.0.cmp(b.0));
        
        // 构建 FST 到临时文件
        let mut fst_builder = MapBuilder::new(BufWriter::new(File::create(&fst_file_new)?))?;
        let mut bitmap_writer = BufWriter::new(File::create(&bitmap_file_new)?);
        
        let mut current_offset: u64 = 0;
        
        for (gram, bitmap) in sorted_grams {
            fst_builder.insert(gram.as_bytes(), current_offset)?;
            
            let mut bitmap_bytes = Vec::new();
            bitmap.serialize_into(&mut bitmap_bytes)?;
            
            bitmap_writer.write_all(&(bitmap_bytes.len() as u32).to_le_bytes())?;
            bitmap_writer.write_all(&bitmap_bytes)?;
            
            current_offset += 4 + bitmap_bytes.len() as u64;
        }
        
        fst_builder.finish()?;
        bitmap_writer.flush()?;
        drop(bitmap_writer);
        
        info!("✓ New index files written");
        
        // 🔥 Windows 特性：rename 可以覆盖被 mmap 的文件
        // UI 进程的 mmap 不会失效，但下次 reload 会看到新内容
        std::fs::rename(&fst_file_new, &fst_file)?;
        std::fs::rename(&bitmap_file_new, &bitmap_file)?;
        
        info!("✓ Index files replaced via rename");
        
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

    /// 全量扫描 paths.dat，把主索引 + USN 追加的所有条目偏移量重建成 offsets.dat
    ///
    /// 合并后主索引已包含 delta grams，追加路径的 file_id 仍在 bitmap 中被引用，
    /// 因此 offsets.dat 必须覆盖它们；此后 offsets_delta.dat 删除，
    /// 新的追加从新的基线继续编号。
    fn rebuild_offsets_index(&self) -> Result<()> {
        let paths_file = format!("{}\\{}_paths.dat", self.output_dir, self.drive_letter);
        let offsets_file = format!("{}\\{}_offsets.dat", self.output_dir, self.drive_letter);
        let tmp_file = format!("{}.new", offsets_file);

        let mut reader = std::io::BufReader::new(File::open(&paths_file)?);
        let mut writer = BufWriter::new(File::create(&tmp_file)?);

        let mut count: u32 = 0;
        let mut offset: u64 = 0;
        let mut len_buf = [0u8; 4];

        writer.write_all(&count.to_le_bytes())?; // 占位，扫描完成后回填

        while reader.read_exact(&mut len_buf).is_ok() {
            writer.write_all(&offset.to_le_bytes())?;
            let path_len = u32::from_le_bytes(len_buf) as u64;
            count += 1;
            offset += 4 + path_len;
            // 跳过路径内容
            std::io::Seek::seek(&mut reader, SeekFrom::Current(path_len as i64))?;
        }

        writer.flush()?;
        drop(writer);

        // 回填数量
        let mut file = OpenOptions::new().write(true).open(&tmp_file)?;
        file.write_all(&count.to_le_bytes())?;
        drop(file);

        // 原子替换（offsets.dat 只被 UI 在打开/热重载时短读，rename 即可保证一致性）
        std::fs::rename(&tmp_file, &offsets_file)?;

        info!("✓ Offsets index rebuilt: {} entries", count);
        Ok(())
    }

    /// 删除 offsets_delta 文件（其内容已折入 offsets.dat）
    fn cleanup_offsets_delta(&self) -> Result<()> {
        let delta_file = format!("{}\\{}_offsets_delta.dat", self.output_dir, self.drive_letter);
        if Path::new(&delta_file).exists() {
            std::fs::remove_file(&delta_file)?;
            info!("✓ Offsets delta removed");
        }
        Ok(())
    }

    /// 递增主索引版本号（独立函数，供全量重建完成后复用）
    pub fn bump_index_version(drive_letter: char, output_dir: &str) -> Result<()> {
        let version_file = format!("{}\\{}_index.version", output_dir, drive_letter);
        Self::bump_version_file(&version_file)
    }

    fn bump_version_file(version_file: &str) -> Result<()> {
        let current_version = if Path::new(version_file).exists() {
            std::fs::read_to_string(version_file)?
                .trim()
                .parse::<u64>()
                .unwrap_or(0)
        } else {
            0
        };

        let new_version = current_version + 1;
        std::fs::write(version_file, new_version.to_string())?;
        info!("✓ Index version updated: {} → {}", current_version, new_version);
        Ok(())
    }

    /// 递增版本号（通知 UI 重新加载索引）
    fn increment_version(&self) -> Result<()> {
        let version_file = format!("{}\\{}_index.version", self.output_dir, self.drive_letter);
        Self::bump_version_file(&version_file)
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
