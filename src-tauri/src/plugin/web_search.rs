// Web 搜索插件

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;

pub struct WebSearchPlugin {
    metadata: PluginMetadata,
    search_engines: Vec<SearchEngine>,
}

#[derive(Clone)]
struct SearchEngine {
    name: String,
    keyword: String,
    url_template: String,
    icon: String,
}

impl WebSearchPlugin {
    pub fn new() -> Self {
        let search_engines = vec![
            SearchEngine {
                name: "Google".to_string(),
                keyword: "g".to_string(),
                url_template: "https://www.google.com/search?q={query}".to_string(),
                icon: "🔍".to_string(),
            },
            SearchEngine {
                name: "Bing".to_string(),
                keyword: "b".to_string(),
                url_template: "https://www.bing.com/search?q={query}".to_string(),
                icon: "🔎".to_string(),
            },
            SearchEngine {
                name: "Baidu".to_string(),
                keyword: "bd".to_string(),
                url_template: "https://www.baidu.com/s?wd={query}".to_string(),
                icon: "🐻".to_string(),
            },
            SearchEngine {
                name: "GitHub".to_string(),
                keyword: "gh".to_string(),
                url_template: "https://github.com/search?q={query}".to_string(),
                icon: "😺".to_string(),
            },
            SearchEngine {
                name: "Stack Overflow".to_string(),
                keyword: "so".to_string(),
                url_template: "https://stackoverflow.com/search?q={query}".to_string(),
                icon: "📚".to_string(),
            },
            SearchEngine {
                name: "YouTube".to_string(),
                keyword: "yt".to_string(),
                url_template: "https://www.youtube.com/results?search_query={query}".to_string(),
                icon: "📺".to_string(),
            },
            SearchEngine {
                name: "Wikipedia".to_string(),
                keyword: "wiki".to_string(),
                url_template: "https://en.wikipedia.org/wiki/Special:Search?search={query}".to_string(),
                icon: "📖".to_string(),
            },
            SearchEngine {
                name: "淘宝".to_string(),
                keyword: "tb".to_string(),
                url_template: "https://s.taobao.com/search?q={query}".to_string(),
                icon: "🛒".to_string(),
            },
            SearchEngine {
                name: "知乎".to_string(),
                keyword: "zh".to_string(),
                url_template: "https://www.zhihu.com/search?q={query}".to_string(),
                icon: "💡".to_string(),
            },
        ];

        Self {
            metadata: PluginMetadata {
                id: "web_search".to_string(),
                name: "Web Search".to_string(),
                description: "Search the web with multiple search engines".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("🌐"),
                trigger_keywords: vec![],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: PluginType::Native,
            },
            search_engines,
        }
    }

    /// 打开 URL
    async fn open_url(url: &str) -> Result<()> {
        let url = url.to_string();
        
        tokio::task::spawn_blocking(move || {
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("cmd")
                    .args(["/C", "start", "", &url])
                    .spawn()?;
            }
            
            #[cfg(target_os = "macos")]
            {
                std::process::Command::new("open")
                    .arg(&url)
                    .spawn()?;
            }
            
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("xdg-open")
                    .arg(&url)
                    .spawn()?;
            }
            
            tracing::info!("Opened URL: {}", url);
            Ok(())
        })
        .await?
    }
}

#[async_trait]
impl Plugin for WebSearchPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let search = ctx.search.trim();
        
        // 至少输入1个字符
        if search.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut results = Vec::new();
        
        // 检查是否是特定搜索引擎的关键词
        for engine in &self.search_engines {
            // 格式: "g rust" 或 "gh tauri"
            if search.starts_with(&format!("{} ", engine.keyword)) {
                let query = search[engine.keyword.len() + 1..].trim();
                if !query.is_empty() {
                    let url = engine.url_template.replace("{query}", &urlencoding::encode(query));
                    
                    results.push(QueryResult {
                        id: url.clone(),
                        title: format!("Search '{}' on {}", query, engine.name),
                        subtitle: url.clone(),
                        icon: WoxImage::emoji(&engine.icon),
                        preview: None,
                        score: 100,
                        context_data: serde_json::Value::Null,
                        group: Some("Web Search".to_string()),
                        plugin_id: self.metadata.id.clone(),
                        refreshable: false,
                        actions: vec![
                            Action {
                                id: "open".to_string(),
                                name: format!("Search on {}", engine.name),
                                icon: None,
                                is_default: true,
                                prevent_hide: false,
                                hotkey: None,
                            },
                        ],
                    });
                }
                
                // 只返回匹配的搜索引擎结果
                return Ok(results);
            }
        }
        
        // 如果没有特定关键词，但输入长度 >= 3，显示所有搜索引擎选项
        if search.len() >= 3 {
            for (idx, engine) in self.search_engines.iter().enumerate() {
                let url = engine.url_template.replace("{query}", &urlencoding::encode(search));
                
                results.push(QueryResult {
                    id: url.clone(),
                    title: format!("Search '{}' on {}", search, engine.name),
                    subtitle: format!("Keyword: {} | {}", engine.keyword, url),
                    icon: WoxImage::emoji(&engine.icon),
                    preview: None,
                    score: 90 - idx as i32, // 按顺序降低分数
                    context_data: serde_json::Value::Null,
                    group: Some("Web Search".to_string()),
                    plugin_id: self.metadata.id.clone(),
                    refreshable: false,
                    actions: vec![
                        Action {
                            id: "open".to_string(),
                            name: format!("Search on {}", engine.name),
                            icon: None,
                            is_default: idx == 0, // 第一个是默认
                            prevent_hide: false,
                            hotkey: None,
                        },
                    ],
                });
            }
        }
        
        Ok(results)
    }
    
    async fn execute(&self, result_id: &str, _action_id: &str) -> Result<()> {
        Self::open_url(result_id).await
    }
}
