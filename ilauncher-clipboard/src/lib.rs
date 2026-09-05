// ilauncher-clipboard：剪贴板历史（store 可单测 + Windows 事件驱动监听）
//
// 与 Tauri 版差异（刻意精简）：
//   - 持久化用 JSONL 追加代替 SQLite（启动器历史场景读多写少，零 schema 负担）
//   - 监听用 WM_CLIPBOARDUPDATE 事件代替 500ms 轮询
//   - 一期仅文本；图片/富文本在 store 的 kind 字段上预留

pub mod store;

#[cfg(target_os = "windows")]
pub mod monitor;

pub use store::{ClipboardItem, ClipboardStore, DEFAULT_CAPACITY, MAX_TEXT_LEN, PREVIEW_CHARS};
