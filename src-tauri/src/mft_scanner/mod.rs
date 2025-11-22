// MFT 扫描器模块 - Windows NTFS 加速
// 🔥 基于 prompt.txt 完整技术方案

#[cfg(target_os = "windows")]
pub mod types;

#[cfg(target_os = "windows")]
pub mod config;

// 🔥 新模块：基于 prompt.txt 的完整实现
#[cfg(target_os = "windows")]
pub mod streaming_builder;

#[cfg(target_os = "windows")]
pub mod index_builder;

#[cfg(target_os = "windows")]
pub mod multi_drive_scanner;

#[cfg(target_os = "windows")]
pub mod usn_incremental_updater;

#[cfg(target_os = "windows")]
pub mod delta_merger;

// 重新导出核心类型
#[cfg(target_os = "windows")]
pub use types::{MftFileEntry, ScanConfig, FrnMap, ParentInfo};

#[cfg(target_os = "windows")]
pub use config::load_config;

// 🔥 导出：流式构建和索引
#[cfg(target_os = "windows")]
pub use streaming_builder::StreamingBuilder;

#[cfg(target_os = "windows")]
pub use index_builder::{IndexBuilder, IndexQuery, PathReader};

#[cfg(target_os = "windows")]
pub use multi_drive_scanner::{MultiDriveScanner, DiskType};

#[cfg(target_os = "windows")]
pub use usn_incremental_updater::UsnIncrementalUpdater;

#[cfg(target_os = "windows")]
pub use delta_merger::DeltaMerger;
