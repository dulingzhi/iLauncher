// IndexV2 - 列式快照索引（v3 磁盘格式）
//
// 模块布局：
// - format:   快照二进制格式（header 元信息、section 布局、charmask 预过滤）
// - writer:   从记录列表构建快照（唯一名池化、父引用解析、CSR 构建、原子替换）
// - snapshot: mmap 读取器（O(1) 打开、typed section 切片、路径沿父链重建）
//
// 与旧 mft_scanner（v2：物化全路径 + FST 3-gram）的差异与动机见
// docs/FILE_INDEX_OPTIMIZATION_PLAN.md Phase 1。

pub mod format;
pub mod search;
pub mod snapshot;
pub mod writer;

pub use format::{charmask_of, required_mask_of, SnapshotMeta};
pub use snapshot::Snapshot;
pub use writer::{write_snapshot, IndexRecord};
