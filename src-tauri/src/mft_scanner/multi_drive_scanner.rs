// 多盘符并行扫描器 - 基于 prompt.txt I/O 感知调度方案

use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::fs;
use std::io::Write;
use tracing::{info, warn};

use super::streaming_builder::StreamingBuilder;
use super::index_builder::IndexBuilder;
use super::types::ScanConfig;

// 🔥 当前数据格式版本（变更后需要重建）
const DATA_FORMAT_VERSION: u32 = 2;  // v1: SQLite, v2: FST+RoaringBitmap

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiskType {
    SSD,
    HDD,
    Unknown,
}

/// 多盘符扫描器
pub struct MultiDriveScanner {
    drives: Vec<char>,
    output_dir: String,
    disk_types: HashMap<char, DiskType>,
}

impl MultiDriveScanner {
    pub fn new(config: &ScanConfig) -> Self {
        let mut scanner = Self {
            drives: config.drives.clone(),
            output_dir: config.output_dir.clone(),
            disk_types: HashMap::new(),
        };
        
        // 检测每个盘符的磁盘类型
        for &drive in &scanner.drives {
            let disk_type = Self::detect_disk_type(drive);
            scanner.disk_types.insert(drive, disk_type);
            info!("📀 Drive {}: detected as {:?}", drive, disk_type);
        }
        
        scanner
    }
    
    /// 检测磁盘类型（SSD/HDD）
    #[cfg(target_os = "windows")]
    fn detect_disk_type(drive: char) -> DiskType {
        use windows::Win32::Storage::FileSystem::*;
        use windows::Win32::System::Ioctl::*;
        use windows::Win32::Foundation::*;
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        
        unsafe {
            let path = format!(r"\\.\{}:", drive);
            let wide: Vec<u16> = OsStr::new(&path)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            
            let handle = match CreateFileW(
                windows::core::PCWSTR(wide.as_ptr()),
                FILE_GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            ) {
                Ok(h) => h,
                Err(_) => return DiskType::Unknown,
            };
            
            // 查询存储设备属性
            #[repr(C)]
            struct StoragePropertyQuery {
                property_id: u32,
                query_type: u32,
                additional_parameters: [u8; 1],
            }
            
            let query = StoragePropertyQuery {
                property_id: 0, // StorageDeviceProperty
                query_type: 0,  // PropertyStandardQuery
                additional_parameters: [0],
            };
            
            let mut buffer = vec![0u8; 1024];
            let mut bytes_returned = 0u32;
            
            let result = windows::Win32::System::IO::DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                Some(&query as *const _ as *const std::ffi::c_void),
                std::mem::size_of_val(&query) as u32,
                Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
                buffer.len() as u32,
                Some(&mut bytes_returned),
                None,
            );
            
            let _ = CloseHandle(handle);
            
            if result.is_err() {
                return DiskType::Unknown;
            }
            
            // 解析返回数据（简化判断）
            // BusType: 0x0B = SATA, 0x11 = NVMe
            // 实际应该解析 STORAGE_DEVICE_DESCRIPTOR 结构
            if bytes_returned > 20 {
                let bus_type = buffer[17];
                match bus_type {
                    0x11 => DiskType::SSD,  // NVMe
                    0x0B => {
                        // SATA 可能是 SSD 或 HDD
                        // 简化：假设 C 盘是 SSD
                        if drive == 'C' {
                            DiskType::SSD
                        } else {
                            DiskType::HDD
                        }
                    }
                    _ => DiskType::HDD,
                }
            } else {
                DiskType::Unknown
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    fn detect_disk_type(_drive: char) -> DiskType {
        DiskType::Unknown
    }
    
    /// 计算并行度
    fn calculate_parallelism(&self) -> usize {
        let ssd_count = self.disk_types.values().filter(|&&t| t == DiskType::SSD).count();
        let hdd_count = self.disk_types.values().filter(|&&t| t == DiskType::HDD).count();
        
        // SSD 可并行，HDD 必须串行
        let parallelism = ssd_count.max(1) + if hdd_count > 0 { 1 } else { 0 };
        
        info!("💡 Parallelism: {} threads ({} SSD, {} HDD)", parallelism, ssd_count, hdd_count);
        
        parallelism
    }
    
    /// 扫描所有驱动器
    pub fn scan_all(&self) -> Result<()> {
        info!("╔═══════════════════════════════════════════╗");
        info!("║    Multi-Drive Parallel Scanner           ║");
        info!("╚═══════════════════════════════════════════╝");
        info!("");
        
        // 🔥 检查数据格式版本，如有变更则清理旧数据
        self.check_and_cleanup_old_data()?;
        
        let total_start = Instant::now();
        
        // 分组：SSD 并行，HDD 串行
        let (ssd_drives, hdd_drives): (Vec<_>, Vec<_>) = self.drives
            .iter()
            .partition(|&&d| self.disk_types.get(&d) == Some(&DiskType::SSD));
        
        info!("📊 Drive classification:");
        info!("   SSD drives: {:?} (will scan in parallel)", ssd_drives);
        info!("   HDD drives: {:?} (will scan serially)", hdd_drives);
        info!("");
        
        let scanned_drives = Arc::new(Mutex::new(Vec::new()));
        
        // 🔥 步骤 1: 并行扫描所有 SSD
        if !ssd_drives.is_empty() {
            info!("⚡ Phase 1: Scanning SSD drives in parallel...");
            
            let ssd_results: Vec<_> = ssd_drives
                .par_iter()
                .map(|&&drive| {
                    self.scan_single_drive(drive)
                })
                .collect();
            
            for (i, result) in ssd_results.into_iter().enumerate() {
                if result.is_ok() {
                    scanned_drives.lock().unwrap().push(*ssd_drives[i]);
                }
            }
        }
        
        // 🔥 步骤 2: 串行扫描所有 HDD
        if !hdd_drives.is_empty() {
            info!("💿 Phase 2: Scanning HDD drives serially...");
            
            for &&drive in &hdd_drives {
                if let Ok(_) = self.scan_single_drive(drive) {
                    scanned_drives.lock().unwrap().push(drive);
                }
            }
        }
        
        let total_elapsed = total_start.elapsed();
        
        info!("");
        info!("╔═══════════════════════════════════════════╗");
        info!("║    Scan Complete                          ║");
        info!("╚═══════════════════════════════════════════╝");
        info!("⏱️  Total time: {:.2}s", total_elapsed.as_secs_f32());
        info!("✓ Successfully scanned: {:?}", scanned_drives.lock().unwrap());
        info!("");
        
        Ok(())
    }
    
    /// 检查数据格式版本并清理旧数据
    fn check_and_cleanup_old_data(&self) -> Result<()> {
        let version_file = format!("{}\\version.txt", self.output_dir);
        
        // 读取现有版本
        let current_version = if std::path::Path::new(&version_file).exists() {
            fs::read_to_string(&version_file)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(1)  // 旧版本默认为 1
        } else {
            1  // 首次运行
        };
        
        // 如果版本不匹配，清理旧数据
        if current_version != DATA_FORMAT_VERSION {
            warn!("🔄 Data format changed (v{} -> v{}), cleaning old files...", 
                  current_version, DATA_FORMAT_VERSION);
            
            // 创建输出目录（如果不存在）
            fs::create_dir_all(&self.output_dir).ok();
            
            // 删除所有旧的索引文件
            if let Ok(entries) = fs::read_dir(&self.output_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        // 删除 .dat, .fst, .db, .tmp 文件
                        if ext == "dat" || ext == "fst" || ext == "db" || ext == "tmp" {
                            if let Some(name) = path.file_name() {
                                info!("   ❌ Removing old file: {:?}", name);
                                fs::remove_file(&path).ok();
                            }
                        }
                    }
                }
            }
            
            info!("✓ Old data cleaned, will rebuild from scratch");
        }
        
        // 写入当前版本
        fs::write(&version_file, DATA_FORMAT_VERSION.to_string())?;
        
        Ok(())
    }
    
    /// 扫描单个驱动器
    fn scan_single_drive(&self, drive: char) -> Result<()> {
        let drive_start = Instant::now();
        
        info!("🚀 Scanning drive {}:", drive);
        
        // 🔥 步骤 1: 流式构建（MFT -> 路径文件）
        let mut builder = StreamingBuilder::new(drive, &self.output_dir)?;
        builder.scan_mft_streaming()?;
        builder.finalize(&self.output_dir)?;
        
        let scan_elapsed = drive_start.elapsed();
        info!("   ✓ MFT scan: {:.2}s", scan_elapsed.as_secs_f32());
        
        // 🔥 步骤 2: 构建 3-gram 索引
        let index_start = Instant::now();
        let mut index_builder = IndexBuilder::new(drive);
        index_builder.build_from_paths(&self.output_dir)?;
        index_builder.save_index(&self.output_dir)?;
        
        let index_elapsed = index_start.elapsed();
        info!("   ✓ Index build: {:.2}s", index_elapsed.as_secs_f32());
        
        let total_elapsed = drive_start.elapsed();
        info!("✅ Drive {} completed in {:.2}s", drive, total_elapsed.as_secs_f32());
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[cfg(target_os = "windows")]
    fn test_detect_disk_type() {
        let disk_type = MultiDriveScanner::detect_disk_type('C');
        println!("C: drive type: {:?}", disk_type);
        assert_ne!(disk_type, DiskType::Unknown);
    }
    
    #[test]
    fn test_parallelism() {
        let mut config = ScanConfig::default();
        config.drives = vec!['C', 'D', 'E'];
        
        let scanner = MultiDriveScanner::new(&config);
        let parallelism = scanner.calculate_parallelism();
        
        println!("Parallelism: {}", parallelism);
        assert!(parallelism > 0);
    }
}
