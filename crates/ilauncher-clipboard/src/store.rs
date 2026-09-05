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
    /// 内容类型（text / image）
    pub kind: String,
    /// 文本内容（kind=text）；图片场景存 PNG 文件路径
    pub content: String,
    /// 列表预览（长文本截断；图片为 "图片 1920x1080"）
    pub preview: String,
    /// Unix 秒时间戳
    pub timestamp: u64,
    /// 收藏
    pub pinned: bool,
    /// 图片内容哈希（去重用；文本为 None）
    #[serde(default)]
    pub image_hash: Option<u64>,
}

/// 新建条目（id/timestamp 由 store 注入）
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
            image_hash: None,
        }
    }

    fn new_image(id: u64, file_path: &str, width: usize, height: usize, hash: u64, timestamp: u64) -> Self {
        Self {
            id,
            kind: "image".into(),
            content: file_path.to_string(),
            preview: format!("图片 {}x{}", width, height),
            timestamp,
            pinned: false,
            image_hash: Some(hash),
        }
    }

    fn is_image(&self) -> bool {
        self.kind == "image"
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
            store.items.sort_by_key(|a| std::cmp::Reverse(a.id));
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

    /// 录入一张图片（PNG 已落盘，传文件路径 + 尺寸 + 内容哈希）
    /// 返回是否真正录入（与最新图片哈希相同 → 去重拒绝）
    pub fn add_image(
        &mut self,
        file_path: &str,
        width: usize,
        height: usize,
        hash: u64,
        timestamp: u64,
    ) -> bool {
        // 与历史任一图片哈希相同则跳过（同一张图再复制一次没有新信息）
        if self.items.iter().any(|it| it.image_hash == Some(hash)) {
            return false;
        }
        // 文本连续去重状态被图片打断
        self.last_text = None;
        let item = ClipboardItem::new_image(self.next_id, file_path, width, height, hash, timestamp);
        self.next_id += 1;
        self.items.insert(0, item);
        self.items.truncate(self.capacity);
        self.persist_append();
        true
    }

    /// 只读文本条目（图片不进搜索/纯文本列表）
    pub fn text_items(&self, offset: usize, limit: usize) -> Vec<ClipboardItem> {
        self.items
            .iter()
            .filter(|it| !it.is_image())
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 列表（新→旧），offset/limit 分页
    pub fn list(&self, offset: usize, limit: usize) -> &[ClipboardItem] {
        let start = offset.min(self.items.len());
        let end = (start + limit).min(self.items.len());
        &self.items[start..end]
    }

    /// 子串搜索（大小写不敏感，新→旧）；只匹配文本条目
    pub fn search(&self, query: &str, limit: usize) -> Vec<ClipboardItem> {
        let q = query.to_lowercase();
        self.items
            .iter()
            .filter(|it| !it.is_image() && it.content.to_lowercase().contains(&q))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 当前容量上限（设置页可调，见 set_capacity）
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 删除单条（图片条目连同 PNG 文件一起删）
    pub fn delete(&mut self, id: u64) -> bool {
        let removed_file = self
            .items
            .iter()
            .find(|it| it.id == id && it.is_image())
            .map(|it| it.content.clone());
        let before = self.items.len();
        self.items.retain(|it| it.id != id);
        let changed = self.items.len() != before;
        if changed {
            if let Some(path) = removed_file {
                let _ = std::fs::remove_file(&path);
            }
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

    /// 清空（连同持久化文件；图片文件一并删除）
    pub fn clear(&mut self) {
        for it in &self.items {
            if it.is_image() {
                let _ = std::fs::remove_file(&it.content);
            }
        }
        self.items.clear();
        self.last_text = None;
        if let Some(path) = &self.persist_path {
            let _ = std::fs::remove_file(path);
        }
    }

    /// 调整容量上限：溢出部分按最旧截断（图片条目连同文件删除），并持久化
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.max(1);
        while self.items.len() > self.capacity {
            if let Some(old) = self.items.pop() {
                if old.is_image() {
                    let _ = std::fs::remove_file(&old.content);
                }
            }
        }
        self.persist_rewrite();
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
    fn set_capacity_truncates_and_persists() {
        let path = temp_path("setcap");
        {
            let mut s = ClipboardStore::with_persist(&path, 10).unwrap();
            for i in 0..5 {
                assert!(s.add_text(&format!("cap{i}"), ts()));
            }
            s.set_capacity(2);
            assert_eq!(s.len(), 2);
            assert_eq!(s.capacity(), 2);
            let all = s.list(0, 10);
            assert_eq!(all[0].content, "cap4");
            assert_eq!(all[1].content, "cap3");
        }
        // 重载：容量是应用级设置（注册表），由 with_persist 参数注入；
        // 注入 2 后溢出条目不回魂
        let mut s = ClipboardStore::with_persist(&path, 2).unwrap();
        assert_eq!(s.capacity(), 2);
        assert_eq!(s.len(), 2);
        assert!(s.add_text("cap5", ts()));
        assert_eq!(s.len(), 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn set_capacity_drops_oldest_image_file() {
        let mut s = ClipboardStore::in_memory(10);
        let dir = std::env::temp_dir().join(format!("ilauncher_clip_setcap_img_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let png = dir.join("old_1.png");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&png, b"fake-png").unwrap();
        assert!(s.add_image(png.to_str().unwrap(), 2, 2, 1, ts()));
        for i in 0..3 {
            assert!(s.add_text(&format!("t{i}"), ts()));
        }
        s.set_capacity(3);
        // 最旧的图片条目被截断，文件一并删除
        assert!(!png.exists());
        assert!(s.list(0, 10).iter().all(|it| !it.is_image()));
        let _ = std::fs::remove_dir_all(&dir);
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

    #[test]
    fn add_image_and_dedup_by_hash() {
        let mut s = ClipboardStore::in_memory(10);
        assert!(s.add_image("T:\\img\\a.png", 640, 480, 111, ts()));
        // 连续相同哈希去重
        assert!(!s.add_image("T:\\img\\b.png", 640, 480, 111, ts()));
        // 不同哈希允许
        assert!(s.add_image("T:\\img\\c.png", 800, 600, 222, ts()));
        assert_eq!(s.len(), 2);
        let latest = &s.list(0, 1)[0];
        assert_eq!(latest.kind, "image");
        assert_eq!(latest.preview, "图片 800x600");
        assert_eq!(latest.image_hash, Some(222));
    }

    #[test]
    fn image_between_same_text_allowed() {
        let mut s = ClipboardStore::in_memory(10);
        assert!(s.add_text("same", ts()));
        assert!(s.add_image("T:\\a.png", 10, 10, 1, ts()));
        // 图片打断了文本连续去重：再复制 "same" 允许
        assert!(s.add_text("same", ts()));
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn search_excludes_images() {
        let mut s = ClipboardStore::in_memory(10);
        s.add_text("hello world", ts());
        s.add_image("T:\\hello.png", 10, 10, 7, ts());
        // 图片路径里含 "hello" 也不该被文本搜索命中
        assert!(s.search("hello", 10).iter().all(|it| it.kind == "text"));
        assert_eq!(s.search("hello", 10).len(), 1);
    }

    #[test]
    fn delete_image_removes_file_and_clear_removes_all() {
        let dir = std::env::temp_dir().join(format!("ilauncher_clip_img_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p1 = dir.join("a.png");
        let p2 = dir.join("b.png");
        std::fs::write(&p1, b"fakepng").unwrap();
        std::fs::write(&p2, b"fakepng2").unwrap();

        let path = temp_path("imgfiles");
        let mut s = ClipboardStore::with_persist(&path, 10).unwrap();
        s.add_image(p1.to_str().unwrap(), 1, 1, 1, ts());
        s.add_image(p2.to_str().unwrap(), 2, 2, 2, ts());
        s.add_text("keep", ts());
        assert_eq!(s.len(), 3);

        // 删除图片条目 → 文件随之删除，其余保留
        let img_id = s.list(1, 1)[0].id;
        assert!(s.delete(img_id));
        assert!(!p2.exists());
        assert!(p1.exists());
        assert_eq!(s.len(), 2);

        // 清空 → 剩余图片文件也删除
        s.clear();
        assert!(!p1.exists());
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn image_roundtrip_persist() {
        let path = temp_path("imgpersist");
        {
            let mut s = ClipboardStore::with_persist(&path, 10).unwrap();
            s.add_image("T:\\x.png", 320, 200, 42, ts());
            s.add_text("text", ts());
        }
        {
            let mut s = ClipboardStore::with_persist(&path, 10).unwrap();
            assert_eq!(s.len(), 2);
            let img = s.list(0, 2).iter().find(|it| it.kind == "image").cloned().expect("图片条目应持久化");
            assert_eq!(img.image_hash, Some(42));
            assert_eq!(img.preview, "图片 320x200");
            // 重载后相同哈希仍然去重（持久化字段完整）
            assert!(!s.add_image("T:\\y.png", 320, 200, 42, ts()));
            assert_eq!(s.len(), 2);
        }
        let _ = std::fs::remove_file(&path);
    }
}
