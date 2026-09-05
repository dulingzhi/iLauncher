//! 网页搜索插件：多搜索引擎关键词触发 + `? ` 前缀全网搜索
//! （对齐 src-tauri/src/plugin/web_search.rs 行为）。
//! 无 gpui 依赖，可单元测试。
//!
//! 相对 Tauri 版的偏离：
//!   - URL 编码用本地 encode_query（不引入 urlencoding 依赖；同 %20 语义，单测锚定）
//!   - execute 返回 ExecuteOutcome::Open（opener 上移 Launcher 层），且先走沙盒
//!     NetworkAccess 域名校验——检查事件落审计管道（Tauri 版无此检查）

use std::sync::Arc;

use anyhow::{Result, anyhow};

use crate::plugin::sandbox::SandboxManager;
use crate::plugin::{ExecuteOutcome, Plugin, PluginAction, PluginMetadata, QueryContext, QueryResult};

#[derive(Clone)]
struct SearchEngine {
    name: String,
    keyword: String,
    url_template: String,
    icon: String,
}

impl SearchEngine {
    fn build_url(&self, query: &str) -> String {
        self.url_template.replace("{query}", &encode_query(query))
    }
}

/// application/x-www-form-urlencoded 百分号编码（对齐 Tauri urlencoding::encode 语义：
/// 非 [A-Za-z0-9-_.~] 一律 %XX，空格 %20 非 +）
fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 从 URL 提取域名（"https://host/path?q=1" → "host"；失败 None）
fn extract_domain(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1)?;
    let host = rest.split('/').next()?;
    if host.is_empty() { None } else { Some(host.to_string()) }
}

pub struct WebSearchPlugin {
    metadata: PluginMetadata,
    engines: Vec<SearchEngine>,
    sandbox: Arc<SandboxManager>,
}

impl WebSearchPlugin {
    pub fn new(sandbox: Arc<SandboxManager>) -> Self {
        // 引擎表与 Tauri 版一致（名称/关键词/URL 模板/图标）
        let engines = [
            ("Google", "g", "https://www.google.com/search?q={query}", "🔍"),
            ("Bing", "b", "https://www.bing.com/search?q={query}", "🔎"),
            ("Baidu", "bd", "https://www.baidu.com/s?wd={query}", "🐻"),
            ("GitHub", "gh", "https://github.com/search?q={query}", "😺"),
            ("Stack Overflow", "so", "https://stackoverflow.com/search?q={query}", "📚"),
            ("YouTube", "yt", "https://www.youtube.com/results?search_query={query}", "📺"),
            ("Wikipedia", "wiki", "https://en.wikipedia.org/wiki/Special:Search?search={query}", "📖"),
            ("淘宝", "tb", "https://s.taobao.com/search?q={query}", "🛒"),
            ("知乎", "zh", "https://www.zhihu.com/search?q={query}", "💡"),
        ]
        .into_iter()
        .map(|(name, keyword, url_template, icon)| SearchEngine {
            name: name.to_string(),
            keyword: keyword.to_string(),
            url_template: url_template.to_string(),
            icon: icon.to_string(),
        })
        .collect();

        Self {
            metadata: PluginMetadata::new("web_search", "Web Search")
                .with_description("多搜索引擎网页搜索（g/b/bd/gh/so/yt/wiki/tb/zh 前缀，或 ? 前缀全网）")
                .with_icon("🌐"),
            engines,
            sandbox,
        }
    }

    /// 关键词前缀命中的引擎（"g rust" → google）
    fn match_engine<'a>(&'a self, search: &'a str) -> Option<(&'a SearchEngine, &'a str)> {
        for engine in &self.engines {
            let prefix = format!("{} ", engine.keyword);
            if let Some(rest) = search.strip_prefix(&prefix) {
                let query = rest.trim();
                if !query.is_empty() {
                    return Some((engine, query));
                }
            }
        }
        None
    }

    fn result(&self, engine: &SearchEngine, query: &str, score: i32, default: bool) -> QueryResult {
        let url = engine.build_url(query);
        QueryResult::new(url.clone(), format!("Search '{}' on {}", query, engine.name))
            .with_subtitle(url)
            .with_icon(engine.icon.clone())
            .with_score(score)
            .with_action(PluginAction {
                id: "open".to_string(),
                name: format!("Search on {}", engine.name),
                is_default: default,
            })
    }
}

impl Plugin for WebSearchPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let search = ctx.search.trim();
        if search.is_empty() {
            return Ok(Vec::new());
        }

        // 1. 引擎关键词："g rust" → 只返回该引擎
        if let Some((engine, query)) = self.match_engine(search) {
            return Ok(vec![self.result(engine, query, 100, true)]);
        }

        // 2. "? " 前缀：全部引擎（分数按表序递减，首个为默认）
        if let Some(query) = search.strip_prefix("? ") {
            let query = query.trim();
            if !query.is_empty() {
                return Ok(self
                    .engines
                    .iter()
                    .enumerate()
                    .map(|(ix, engine)| self.result(engine, query, 90 - ix as i32, ix == 0))
                    .collect());
            }
        }

        Ok(Vec::new())
    }

    fn execute(&self, result_id: &str, action_id: &str) -> Result<ExecuteOutcome> {
        if action_id != "open" {
            return Err(anyhow!("Unknown action: {action_id}"));
        }
        // 网络权限检查（事件落审计管道）；拒绝时 Err 上抛，URL 不会被打开
        let domain = extract_domain(result_id)
            .ok_or_else(|| anyhow!("无法解析 URL 域名: {result_id}"))?;
        self.sandbox.validate_network_access(&self.metadata.id, &domain)?;
        Ok(ExecuteOutcome::Open(result_id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditLogger;
    use crate::plugin::sandbox::{NetworkScope, PluginPermission, SandboxConfig, SecurityLevel};

    fn plugin() -> WebSearchPlugin {
        WebSearchPlugin::new(Arc::new(SandboxManager::default()))
    }

    #[test]
    fn encode_query_percent_encoding() {
        assert_eq!(encode_query("rust lang"), "rust%20lang");
        assert_eq!(encode_query("a&b=c"), "a%26b%3Dc");
        assert_eq!(encode_query("中文"), "%E4%B8%AD%E6%96%87");
        assert_eq!(encode_query("a-z_A.Z~0"), "a-z_A.Z~0");
    }

    #[test]
    fn extract_domain_from_url() {
        assert_eq!(extract_domain("https://github.com/search?q=1"), Some("github.com".to_string()));
        assert_eq!(extract_domain("https://www.baidu.com/s?wd=2"), Some("www.baidu.com".to_string()));
        assert_eq!(extract_domain("not-a-url"), None);
    }

    #[test]
    fn keyword_prefix_single_engine() {
        let p = plugin();
        let rs = p.query(&QueryContext::new("g rust lang")).unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].title, "Search 'rust lang' on Google");
        assert!(rs[0].id.contains("q=rust%20lang"));
        assert_eq!(rs[0].score, 100);

        // bd 命中百度（验证多字符关键词）
        let rs = p.query(&QueryContext::new("bd 天气")).unwrap();
        assert_eq!(rs.len(), 1);
        assert!(rs[0].id.starts_with("https://www.baidu.com/"));
    }

    #[test]
    fn keyword_without_space_is_plain_text() {
        let p = plugin();
        // "google" 不是 "g " 前缀 → 无结果（对齐 Tauri：不会误触发）
        assert!(p.query(&QueryContext::new("google")).unwrap().is_empty());
    }

    #[test]
    fn question_prefix_all_engines() {
        let p = plugin();
        let rs = p.query(&QueryContext::new("? hello")).unwrap();
        assert_eq!(rs.len(), 9);
        assert_eq!(rs[0].score, 90);
        assert_eq!(rs[8].score, 82);
        // 首个结果为默认动作
        assert_eq!(rs[0].default_action_id(), Some("open"));
    }

    #[test]
    fn empty_and_plain_query() {
        let p = plugin();
        assert!(p.query(&QueryContext::new("")).unwrap().is_empty());
        assert!(p.query(&QueryContext::new("? ")).unwrap().is_empty());
        assert!(p.query(&QueryContext::new("plain words")).unwrap().is_empty());
    }

    #[test]
    fn open_action_validates_network_permission() {
        let logger = Arc::new(parking_lot::Mutex::new(AuditLogger::in_memory(50)));
        let sandbox = Arc::new(SandboxManager::new(logger.clone()));
        sandbox.register(SandboxConfig {
            plugin_id: "web_search".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some(
                [PluginPermission::NetworkAccess(NetworkScope::All)].into_iter().collect(),
            ),
            enabled: true,
            timeout_ms: None,
            max_memory_mb: None,
        });
        let p = WebSearchPlugin::new(sandbox.clone());
        let url = "https://www.google.com/search?q=x";
        assert_eq!(p.execute(url, "open").unwrap(), ExecuteOutcome::Open(url.to_string()));
        // 审计管道收到 NetworkAccess 允许事件
        assert!(logger.lock().len() >= 1);

        // 拒绝路径：Restricted 默认权限无网络访问 → Err 且不打开
        sandbox.register(SandboxConfig {
            plugin_id: "web_search".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: None,
            enabled: true,
            timeout_ms: None,
            max_memory_mb: None,
        });
        let p = WebSearchPlugin::new(sandbox);
        assert!(p.execute(url, "open").is_err());
        assert!(p.execute(url, "bogus").is_err());
    }
}
