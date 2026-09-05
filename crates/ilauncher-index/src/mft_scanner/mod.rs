// MFT 扫描器模块 - Windows NTFS 加速（v3 链路）
//
// 现役链路：StreamingBuilder（MFT 流式扫描）→ v3_export（写 v3 列式快照）
// → v3_service（常驻服务：启动决策 + USN catch-up + compact，读写经 index_v2）。
// v2 链路（IndexBuilder/IndexQuery/PathReader + UsnIncrementalUpdater +
// DeltaMerger + QueryCache + MultiDriveScanner，{D}_delta.* 增量协议）随
// 唯一消费者 src-tauri 已退役删除。

#[cfg(target_os = "windows")]
pub mod types;

#[cfg(target_os = "windows")]
pub mod streaming_builder;

#[cfg(target_os = "windows")]
pub mod v3_export;

#[cfg(target_os = "windows")]
pub mod v3_service;

// 重新导出核心类型
#[cfg(target_os = "windows")]
pub use types::{FrnMap, ParentInfo, ScanConfig};

#[cfg(target_os = "windows")]
pub use streaming_builder::StreamingBuilder;

#[cfg(target_os = "windows")]
pub use v3_service::V3DriveService;
