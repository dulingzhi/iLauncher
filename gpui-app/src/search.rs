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
}

impl Entry {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self { name: name.into(), path: path.into() }
    }
}

/// 搜索数据源
pub enum SearchSource {
    #[cfg(feature = "ilauncher")]
    Live(ilauncher_index::index_v2::LiveIndex),
    Demo(Vec<Entry>),
}

impl SearchSource {
    /// 按环境构造：优先 ILAUNCHER_SNAPSHOT 指定的真实快照（需 feature ilauncher）
    pub fn from_env_or_demo(demo_entries: Vec<Entry>) -> Self {
        #[cfg(feature = "ilauncher")]
        if let Ok(path) = std::env::var("ILAUNCHER_SNAPSHOT") {
            match ilauncher_index::index_v2::LiveIndex::open(std::path::Path::new(&path)) {
                Ok(idx) => {
                    eprintln!("✓ LiveIndex 已加载: {}（{} 行）", path, idx.snapshot().row_count());
                    return Self::Live(idx);
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
            Self::Live(idx) => idx
                .search(q, limit)
                .unwrap_or_default()
                .into_iter()
                .map(|h| Entry::new(h.name, h.path))
                .collect(),
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
        let Ok(path) = std::env::var("ILAUNCHER_SNAPSHOT") else { return };
        let src = SearchSource::from_env_or_demo(vec![]);
        let hits = src.search("report", 50);
        assert!(!hits.is_empty(), "真实快照搜 report 应有结果");
        assert!(hits.iter().all(|h| !h.name.is_empty() && !h.path.is_empty()));
    }
}
