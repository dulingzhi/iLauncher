// MFT 扫描器模块 - Windows NTFS 加速

#[cfg(target_os = "windows")]
pub mod scanner;

#[cfg(target_os = "windows")]
pub mod ipc;

#[cfg(target_os = "windows")]
pub mod launcher;

// 使用 USN Journal 扫描器
#[cfg(target_os = "windows")]
pub use scanner::{UsnScanner as MftScanner, MftFileEntry};

#[cfg(target_os = "windows")]
pub use ipc::{ScannerServer, ScannerClient};

#[cfg(target_os = "windows")]
pub use launcher::ScannerLauncher;
