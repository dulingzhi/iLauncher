// 剪贴板历史核心：条目、环形缓冲存储、JSONL 持久化。
// 纯内存/文件逻辑，无 UI、无系统调用，全部可单测。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;

/// 单条剪贴板历史
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardItem {
    /// 单调递增序号（新→旧按 id 降序展示）
    pub id: u64,
    /// 内容类型（一期仅 text；image 预留）
    pub kind: String,
    /// 文本内容（kind=text）；图片场景存文件路径
    pub content: String,
    /// 列表预览（长文本截断）
    pub preview: String,
    /// Unix 秒时间戳
    pub timestamp: u64,
    /// 收藏
    pub pinned: bool,
}

/// 新建文本条目（id/timestamp 由 store 注入）
impl ClipboardItem {
    fn new_text(id: u64, content: &str, timestamp: u64) -> Self {
        let preview = if content.chars().count() > PREVIEW_CHARS {
            content.chars().take(PREVIEW_CHARS).collect::<String>() + "…"
        } else {
            content.to_string()
        };
        Self {
            id,
            kind: "text".into(),
            content: content.to_string(),
            preview,
            timestamp,
            pinned: false,
        }
    }
}

/// 单条文本上限（与 Tauri 版一致，100KB）
pub const MAX_TEXT_LEN: usize = 100_000;
/// 预览字符数
pub const PREVIEW_CHARS: usize = 200;
/// 默认历史容量
pub const DEFAULT_CAPACITY: usize = 500;

/// 环形缓冲 + JSONL 持久化的剪贴板历史存储
pub struct ClipboardStore {
    /// 新→旧
    items: Vec<ClipboardItem>,
    capacity: usize,
    next_id: u64,
    persist_path: Option<PathBuf>,
    /// 最近一条文本内容，用于连续去重
    last_text: Option<String>,
}

impl ClipboardStore {
    /// 纯内存存储（测试/调试用）
    pub fn in_memory(capacity: usize) -> Self {
        Self { items: Vec::new(), capacity, next_id: 1, persist_path: None, last_text: None }
    }

    /// 带 JSONL 持久化的存储；文件存在则加载历史
    pub fn with_persist(path: impl Into<PathBuf>, capacity: usize) -> Result<Self> {
        let path = path.into();
        let mut store = Self::in_memory(capacity);
        if path.exists() {
            let file = std::fs::File::open(&path).context("open clipboard history")?;
            for line in std::io::BufReader::new(file).lines() {
                let Ok(line) = line else { continue };
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(item) = serde_json::from_str::<ClipboardItem>(&line) {
                    store.next_id = store.next_id.max(item.id + 1);
                    store.items.push(item);
                }
            }
            // 只保留 capacity 条
            if store.items.len() > capacity {
                let overflow = store.items.len() - capacity;
                store.items.drain(..overflow);
            }
            // 按 id 降序（新→旧）排序，容忍文件里乱序
            store.items.sort_by(|a, b| b.id.cmp(&a.id));
        }
        store.persist_path = Some(path);
        Ok(store)
    }

    /// 录入一条文本；返回是否真正录入（连续重复/超长返回 false）
    pub fn add_text(&mut self, text: &str, timestamp: u64) -> bool {
        let text = text.trim_end_matches(['\r', '\n']);
        if text.trim().is_empty() || text.len() > MAX_TEXT_LEN {
            return false;
        }
        // 连续去重：与最近一条完全相同则忽略
        if self.last_text.as_deref() == Some(text) {
            return false;
        }
        // 与最新一条内容相同也忽略（复制旧内容场景）
        if self.items.first().map(|it| it.content.as_str()) == Some(text) {
            self.last_text = Some(text.to_string());
            return false;
        }
        self.last_text = Some(text.to_string());
        let item = ClipboardItem::new_text(self.next_id, text, timestamp);
        self.next_id += 1;
        self.items.insert(0, item);
        self.items.truncate(self.capacity);
        self.persist_append();
        true
    }

    /// 列表（新→旧），offset/limit 分页
    pub fn list(&self, offset: usize, limit: usize) -> &[ClipboardItem] {
        let start = offset.min(self.items.len());
        let end = (start + limit).min(self.items.len());
        &self.items[start..end]
    }

    /// 子串搜索（大小写不敏感，新→旧）
    pub fn search(&self, query: &str, limit: usize) -> Vec<ClipboardItem> {
        let q = query.to_lowercase();
        self.items
            .iter()
            .filter(|it| it.content.to_lowercase().contains(&q))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 删除单条
    pub fn delete(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|it| it.id != id);
        let changed = self.items.len() != before;
        if changed {
            self.persist_rewrite();
        }
        changed
    }

    /// 切换收藏
    pub fn toggle_pinned(&mut self, id: u64) -> Option<bool> {
        let item = self.items.iter_mut().find(|it| it.id == id)?;
        item.pinned = !item.pinned;
        let v = item.pinned;
        self.persist_rewrite();
        Some(v)
    }

    /// 清空（连同持久化文件）
    pub fn clear(&mut self) {
        self.items.clear();
        self.last_text = None;
        if let Some(path) = &self.persist_path {
            let _ = std::fs::remove_file(path);
        }
    }

    /// 追加一行到 JSONL
    fn persist_append(&self) {
        let Some(path) = &self.persist_path else { return };
        let Some(first) = self.items.first() else { return };
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            if let Ok(line) = serde_json::to_string(first) {
                let _ = writeln!(f, "{line}");
            }
        }
    }

    /// 全量重写（删除/置顶后保持文件紧凑）
    fn persist_rewrite(&self) {
        let Some(path) = &self.persist_path else { return };
        if let Ok(mut f) = std::fs::File::create(path) {
            // 新→旧逐行写回
            for item in &self.items {
                if let Ok(line) = serde_json::to_string(item) {
                    let _ = writeln!(f, "{line}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn ts() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    fn temp_path(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ilauncher_clip_test_{}_{}.jsonl", tag, std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn add_and_list_newest_first() {
        let mut s = ClipboardStore::in_memory(10);
        assert!(s.add_text("first", ts()));
        assert!(s.add_text("second", ts()));
        assert!(s.add_text("third", ts()));
        assert_eq!(s.len(), 3);
        let all = s.list(0, 10);
        assert_eq!(all[0].content, "third");
        assert_eq!(all[2].content, "first");
        assert!(all[0].id > all[2].id);
    }

    #[test]
    fn consecutive_duplicate_ignored() {
        let mut s = ClipboardStore::in_memory(10);
        assert!(s.add_text("same", ts()));
        assert!(!s.add_text("same", ts()));
        assert_eq!(s.len(), 1);
        // 中间插了别的再复制相同内容 → 允许
        assert!(s.add_text("other", ts()));
        assert!(s.add_text("same", ts()));
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn empty_and_oversize_rejected() {
        let mut s = ClipboardStore::in_memory(10);
        assert!(!s.add_text("", ts()));
        assert!(!s.add_text("   \n", ts()));
        assert!(!s.add_text(&"x".repeat(MAX_TEXT_LEN + 1), ts()));
        assert!(s.is_empty());
    }

    #[test]
    fn capacity_truncates_oldest() {
        let mut s = ClipboardStore::in_memory(3);
        for i in 0..5 {
            assert!(s.add_text(&format!("item{i}"), ts()));
        }
        assert_eq!(s.len(), 3);
        let all = s.list(0, 10);
        assert_eq!(all[0].content, "item4");
        assert_eq!(all[2].content, "item2");
    }

    #[test]
    fn search_case_insensitive_substring() {
        let mut s = ClipboardStore::in_memory(10);
        s.add_text("Hello World", ts());
        s.add_text("rust code", ts());
        s.add_text("HELLO again", ts());
        let hits = s.search("hello", 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].content, "HELLO again"); // 新→旧
    }

    #[test]
    fn long_text_gets_preview() {
        let mut s = ClipboardStore::in_memory(10);
        let long = "汉".repeat(PREVIEW_CHARS + 50);
        s.add_text(&long, ts());
        let item = &s.list(0, 1)[0];
        assert!(item.preview.ends_with('…'));
        assert_eq!(item.preview.chars().count(), PREVIEW_CHARS + 1);
        assert_eq!(item.content, long); // 完整内容保留
    }

    #[test]
    fn persist_roundtrip() {
        let path = temp_path("roundtrip");
        {
            let mut s = ClipboardStore::with_persist(&path, 10).unwrap();
            s.add_text("alpha", ts());
            s.add_text("beta", ts());
            s.toggle_pinned(s.list(0, 1)[0].id);
        }
        {
            let s = ClipboardStore::with_persist(&path, 10).unwrap();
            assert_eq!(s.len(), 2);
            let all = s.list(0, 10);
            assert_eq!(all[0].content, "beta");
            assert!(all[0].pinned);
            assert!(!all[1].pinned);
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn persist_respects_capacity_on_load() {
        let path = temp_path("capacity");
        {
            let mut s = ClipboardStore::with_persist(&path, 10).unwrap();
            for i in 0..8 {
                s.add_text(&format!("c{i}"), ts());
            }
        }
        {
            let s = ClipboardStore::with_persist(&path, 5).unwrap();
            assert_eq!(s.len(), 5);
            assert_eq!(s.list(0, 1)[0].content, "c7");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn delete_and_clear() {
        let path = temp_path("delete");
        let mut s = ClipboardStore::with_persist(&path, 10).unwrap();
        s.add_text("a", ts());
        s.add_text("b", ts());
        let id = s.list(0, 1)[0].id;
        assert!(s.delete(id));
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn trailing_newline_stripped() {
        let mut s = ClipboardStore::in_memory(10);
        assert!(s.add_text("line\n", ts()));
        assert_eq!(s.list(0, 1)[0].content, "line");
    }
}
