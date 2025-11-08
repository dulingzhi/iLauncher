// MFT 扫描器模块 - Windows NTFS 加速

#[cfg(target_os = "windows")]
pub mod types;

#[cfg(target_os = "windows")]
pub mod scanner;

#[cfg(target_os = "windows")]
pub mod monitor;

#[cfg(target_os = "windows")]
pub mod database;

#[cfg(target_os = "windows")]
pub mod config;

// 重新导出核心类型
#[cfg(target_os = "windows")]
pub use types::{MftFileEntry, ScanConfig, FrnMap, ParentInfo};

#[cfg(target_os = "windows")]
pub use scanner::UsnScanner;

#[cfg(target_os = "windows")]
pub use monitor::UsnMonitor;

#[cfg(target_os = "windows")]
pub use database::Database;

#[cfg(target_os = "windows")]
pub use config::load_config;

