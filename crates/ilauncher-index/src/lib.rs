// ilauncher-index：iLauncher 核心索引 crate
//
// 从 src-tauri 拆出（无 Tauri/UI 依赖），由 GPUI 前端以库调用直接消费：
// - index_v2:   列式快照索引（v3 磁盘格式：charmask 预过滤 + mmap 懒加载 + USN 覆盖层）
// - mft_scanner: Windows NTFS MFT 扫描器（流式构建 v3 快照、常驻服务 catch-up/compact）
//
// 拆分约定：本 crate 不得依赖 tauri/wry/tokio 等 UI 运行时；
// 新增跨模块依赖前请先确认它能保持 UI 无关。

pub mod index_v2;

#[cfg(target_os = "windows")]
pub mod mft_scanner;

pub mod paths;
