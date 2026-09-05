// 搜索门面：UI 与索引实现之间的唯一接口（本模块不依赖 gpui，可单元测试）
//
// 两种数据源：
//   - Live：ilauncher_lib 的 LiveIndex（真实 MFT 快照，feature ilauncher）
//   - Demo：内存假数据（无 feature 或开发调试）
//
// 语义约定（与现行 Tauri 版启动器一致）：
//   - 空查询返回空结果（启动器惯例：不打断用户前先给全量列表）
//   - 非空查询最多返回 limit 条
//   - Demo 源为大小写不敏感子串匹配；Live 源走 LiveIndex 的模糊搜索

/// 一条搜索结果（UI 只消费这个结构）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    /// 索引 fzf 匹配分（Demo 源恒 0；Live 源用于跨盘合并排序）
    pub score: i64,
}

impl Entry {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self { name: name.into(), path: path.into(), score: 0 }
    }

    #[cfg(feature = "ilauncher")]
    pub fn with_score(name: impl Into<String>, path: impl Into<String>, score: i64) -> Self {
        Self { name: name.into(), path: path.into(), score }
    }
}

/// feature 开启时为真实 LiveIndex 的多盘集合；关闭时为占位单元结构
#[cfg(feature = "ilauncher")]
#[derive(Clone)]
pub struct LiveSet(std::sync::Arc<std::sync::RwLock<Vec<ilauncher_index::index_v2::LiveIndex>>>);

#[cfg(not(feature = "ilauncher"))]
#[derive(Clone)]
pub struct LiveSet;

impl LiveSet {
    pub fn empty() -> Self {
        #[cfg(feature = "ilauncher")]
        {
            Self(std::sync::Arc::new(std::sync::RwLock::new(Vec::new())))
        }
        #[cfg(not(feature = "ilauncher"))]
        {
            Self
        }
    }

    /// 当前已加载的盘数（UI 状态栏用，非阻塞）
    #[cfg(feature = "ilauncher")]
    pub fn drive_count(&self) -> usize {
        self.0.try_read().map(|g| g.len()).unwrap_or(0)
    }

    /// 后台加载线程追加一盘（增量加载）
    #[cfg(feature = "ilauncher")]
    pub fn push_index(&self, idx: ilauncher_index::index_v2::LiveIndex) {
        self.0.write().unwrap().push(idx);
    }

    /// 清空全部索引（重建前释放 mmap 文件占用）
    #[cfg(feature = "ilauncher")]
    pub fn clear(&self) {
        self.0.write().unwrap().clear();
    }
}

/// 搜索数据源
pub enum SearchSource {
    #[cfg(feature = "ilauncher")]
    Live(LiveSet),
    Demo(Vec<Entry>),
}

impl SearchSource {
    /// 按环境构造：优先 ILAUNCHER_SNAPSHOT 指定的真实快照（需 feature ilauncher）
    #[cfg(feature = "ilauncher")]
    pub fn from_env_single(demo_entries: Vec<Entry>) -> Self {
        if let Ok(path) = std::env::var("ILAUNCHER_SNAPSHOT") {
            match ilauncher_index::index_v2::LiveIndex::open(std::path::Path::new(&path)) {
                Ok(idx) => {
                    eprintln!("✓ LiveIndex 已加载: {}（{} 行）", path, idx.snapshot().row_count());
                    let set = LiveSet::empty();
                    set.0.write().unwrap().push(idx);
                    return Self::Live(set);
                }
                Err(e) => eprintln!("⚠ 打开快照 {} 失败（{}），回退 Demo 数据", path, e),
            }
        }
        Self::Demo(demo_entries)
    }

    /// 搜索：空查询返回空；非空最多 limit 条
    pub fn search(&self, query: &str, limit: usize) -> Vec<Entry> {
        let q = query.trim();
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }
        match self {
            #[cfg(feature = "ilauncher")]
            Self::Live(set) => {
                let guard = match set.0.read() {
                    Ok(g) => g,
                    Err(_) => return Vec::new(),
                };
                // 多盘合并：每盘多取（4×，封顶 200）保证高质量命中不被截断，
                // 统一按索引 fzf 分降序排后取 limit（SearchHit.score 无需自研 ranking）
                let per_drive = limit.saturating_mul(4).min(200);
                let mut merged: Vec<Entry> = guard
                    .iter()
                    .flat_map(|idx| idx.search(q, per_drive).unwrap_or_default())
                    .map(|h| Entry::with_score(h.name, h.path, h.score))
                    .collect();
                merged.sort_by(|a, b| b.score.cmp(&a.score));
                merged.truncate(limit);
                merged
            }
            Self::Demo(items) => {
                let lower = q.to_lowercase();
                items
                    .iter()
                    .filter(|e| e.name.to_lowercase().contains(&lower))
                    .take(limit)
                    .cloned()
                    .collect()
            }
        }
    }

    /// 状态栏文本（非阻塞）
    pub fn status_text(&self) -> String {
        #[cfg(feature = "ilauncher")]
        if let Self::Live(set) = self {
            let n = set.drive_count();
            return if n == 0 { "索引加载中…".into() } else { format!("实时索引 {} 盘", n) };
        }
        let _ = self;
        "演示数据".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Vec<Entry> {
        (0..1000)
            .map(|i| Entry::new(format!("file_{:04}.txt", i), format!("C:\\demo\\file_{:04}.txt", i)))
            .collect()
    }

    #[test]
    fn empty_query_returns_empty() {
        let src = SearchSource::Demo(demo());
        assert!(src.search("", 50).is_empty());
        assert!(src.search("   ", 50).is_empty());
    }

    #[test]
    fn zero_limit_returns_empty() {
        let src = SearchSource::Demo(demo());
        assert!(src.search("file", 0).is_empty());
    }

    #[test]
    fn substring_case_insensitive() {
        let src = SearchSource::Demo(demo());
        let hits = src.search("FILE_0001", 50);
        assert_eq!(hits.len(), 1);
        assert!(hits.iter().all(|h| h.name.contains("file_0001")));
    }

    #[test]
    fn limit_is_respected() {
        let src = SearchSource::Demo(demo());
        assert_eq!(src.search("file", 7).len(), 7);
    }

    #[test]
    fn chinese_substring() {
        let mut items = demo();
        items.push(Entry::new("年度总结报告.docx", "C:\\docs\\年度总结报告.docx"));
        let src = SearchSource::Demo(items);
        let hits = src.search("报告", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "年度总结报告.docx");
    }

    #[test]
    fn no_match() {
        let src = SearchSource::Demo(demo());
        assert!(src.search("不存在的文件xyz", 50).is_empty());
    }

    // feature ilauncher 下的真实快照冒烟（设置 ILAUNCHER_SNAPSHOT 时启用）
    #[cfg(feature = "ilauncher")]
    #[test]
    fn live_snapshot_smoke() {
        let Ok(_path) = std::env::var("ILAUNCHER_SNAPSHOT") else { return };
        let src = SearchSource::from_env_single(vec![]);
        let hits = src.search("report", 50);
        assert!(!hits.is_empty(), "真实快照搜 report 应有结果");
        assert!(hits.iter().all(|h| !h.name.is_empty() && !h.path.is_empty()));
    }
}
