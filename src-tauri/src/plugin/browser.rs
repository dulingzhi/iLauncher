// 浏览器书签和历史记录搜索插件

use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Bookmark {
    title: String,
    url: String,
    folder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEntry {
    title: String,
    url: String,
    visit_count: i32,
    last_visit_time: i64,
}

pub struct BrowserPlugin {
    metadata: PluginMetadata,
    bookmarks: Arc<RwLock<Vec<Bookmark>>>,
    history: Arc<RwLock<Vec<HistoryEntry>>>,
}

impl BrowserPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "browser".to_string(),
                name: "浏览器".to_string(),
                description: "搜索浏览器书签和历史记录".to_string(),
                icon: WoxImage::Emoji("🌐".to_string()),
                version: "1.0.0".to_string(),
                author: "iLauncher".to_string(),
                trigger_keywords: vec!["bm".to_string(), "his".to_string()],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string()],
                plugin_type: PluginType::Native,
            },
            bookmarks: Arc::new(RwLock::new(Vec::new())),
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn init(&self) {
        tracing::info!("Initializing browser plugin...");
        
        // 加载书签
        if let Err(e) = self.load_bookmarks().await {
            tracing::warn!("Failed to load bookmarks: {}", e);
        }
        
        // 加载历史记录
        if let Err(e) = self.load_history().await {
            tracing::warn!("Failed to load history: {}", e);
        }
        
        let bookmark_count = self.bookmarks.read().await.len();
        let history_count = self.history.read().await.len();
        tracing::info!("Browser plugin initialized: {} bookmarks, {} history entries", bookmark_count, history_count);
    }

    async fn load_bookmarks(&self) -> Result<()> {
        let mut all_bookmarks = Vec::new();

        // Chrome 书签
        if let Ok(bookmarks) = self.load_chrome_bookmarks().await {
            all_bookmarks.extend(bookmarks);
        }

        // Edge 书签
        if let Ok(bookmarks) = self.load_edge_bookmarks().await {
            all_bookmarks.extend(bookmarks);
        }

        *self.bookmarks.write().await = all_bookmarks;
        Ok(())
    }

    async fn load_chrome_bookmarks(&self) -> Result<Vec<Bookmark>> {
        let local_data = dirs::data_local_dir().ok_or_else(|| anyhow::anyhow!("No local data dir"))?;
        let bookmark_path = local_data.join("Google\\Chrome\\User Data\\Default\\Bookmarks");
        
        if !bookmark_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&bookmark_path)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        
        let mut bookmarks = Vec::new();
        self.parse_bookmark_folder(&json["roots"]["bookmark_bar"], "书签栏", &mut bookmarks);
        self.parse_bookmark_folder(&json["roots"]["other"], "其他书签", &mut bookmarks);
        
        tracing::info!("Loaded {} Chrome bookmarks", bookmarks.len());
        Ok(bookmarks)
    }

    async fn load_edge_bookmarks(&self) -> Result<Vec<Bookmark>> {
        let local_data = dirs::data_local_dir().ok_or_else(|| anyhow::anyhow!("No local data dir"))?;
        let bookmark_path = local_data.join("Microsoft\\Edge\\User Data\\Default\\Bookmarks");
        
        if !bookmark_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&bookmark_path)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        
        let mut bookmarks = Vec::new();
        self.parse_bookmark_folder(&json["roots"]["bookmark_bar"], "书签栏", &mut bookmarks);
        self.parse_bookmark_folder(&json["roots"]["other"], "其他书签", &mut bookmarks);
        
        tracing::info!("Loaded {} Edge bookmarks", bookmarks.len());
        Ok(bookmarks)
    }

    fn parse_bookmark_folder(&self, node: &serde_json::Value, folder: &str, bookmarks: &mut Vec<Bookmark>) {
        if let Some(node_type) = node["type"].as_str() {
            if node_type == "url" {
                if let (Some(title), Some(url)) = (node["name"].as_str(), node["url"].as_str()) {
                    bookmarks.push(Bookmark {
                        title: title.to_string(),
                        url: url.to_string(),
                        folder: folder.to_string(),
                    });
                }
            } else if node_type == "folder" {
                if let Some(children) = node["children"].as_array() {
                    let folder_name = node["name"].as_str().unwrap_or(folder);
                    for child in children {
                        self.parse_bookmark_folder(child, folder_name, bookmarks);
                    }
                }
            }
        }
    }

    async fn load_history(&self) -> Result<()> {
        let mut all_history = Vec::new();

        // Chrome 历史
        if let Ok(history) = self.load_chrome_history().await {
            all_history.extend(history);
        }

        // Edge 历史
        if let Ok(history) = self.load_edge_history().await {
            all_history.extend(history);
        }

        // 按访问次数和时间排序
        all_history.sort_by(|a, b| {
            b.visit_count.cmp(&a.visit_count)
                .then(b.last_visit_time.cmp(&a.last_visit_time))
        });

        // 只保留前 1000 条
        all_history.truncate(1000);

        *self.history.write().await = all_history;
        Ok(())
    }

    async fn load_chrome_history(&self) -> Result<Vec<HistoryEntry>> {
        let local_data = dirs::data_local_dir().ok_or_else(|| anyhow::anyhow!("No local data dir"))?;
        let history_path = local_data.join("Google\\Chrome\\User Data\\Default\\History");
        
        self.load_history_from_db(&history_path).await
    }

    async fn load_edge_history(&self) -> Result<Vec<HistoryEntry>> {
        let local_data = dirs::data_local_dir().ok_or_else(|| anyhow::anyhow!("No local data dir"))?;
        let history_path = local_data.join("Microsoft\\Edge\\User Data\\Default\\History");
        
        self.load_history_from_db(&history_path).await
    }

    async fn load_history_from_db(&self, db_path: &PathBuf) -> Result<Vec<HistoryEntry>> {
        if !db_path.exists() {
            return Ok(Vec::new());
        }

        // 复制数据库到临时文件（避免锁定）
        let temp_path = std::env::temp_dir().join(format!("ilauncher_history_{}.db", uuid::Uuid::new_v4()));
        std::fs::copy(db_path, &temp_path)?;

        let conn = Connection::open(&temp_path)?;
        let mut stmt = conn.prepare(
            "SELECT title, url, visit_count, last_visit_time 
             FROM urls 
             WHERE visit_count > 0 
             ORDER BY visit_count DESC, last_visit_time DESC 
             LIMIT 500"
        )?;

        let history_iter = stmt.query_map([], |row| {
            Ok(HistoryEntry {
                title: row.get(0).unwrap_or_default(),
                url: row.get(1)?,
                visit_count: row.get(2)?,
                last_visit_time: row.get(3)?,
            })
        })?;

        let mut history = Vec::new();
        for entry in history_iter {
            if let Ok(entry) = entry {
                history.push(entry);
            }
        }

        // 清理临时文件
        let _ = std::fs::remove_file(&temp_path);

        tracing::info!("Loaded {} history entries from {:?}", history.len(), db_path);
        Ok(history)
    }
}

#[async_trait]
impl crate::plugin::Plugin for BrowserPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim();
        
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let matcher = SkimMatcherV2::default();
        let mut results = Vec::new();

        // 检查是否有触发词
        let (search_bookmarks, search_history, search_term) = if query.starts_with("bm ") {
            (true, false, &query[3..])
        } else if query.starts_with("his ") {
            (false, true, &query[4..])
        } else {
            (true, true, query)
        };

        // 搜索书签
        if search_bookmarks {
            let bookmarks = self.bookmarks.read().await;
            for bookmark in bookmarks.iter().take(100) {
                let title_score = matcher.fuzzy_match(&bookmark.title, search_term).unwrap_or(0);
                let url_score = matcher.fuzzy_match(&bookmark.url, search_term).unwrap_or(0);
                let score = title_score.max(url_score);

                if score > 30 {
                    results.push(QueryResult {
                        id: bookmark.url.clone(),
                        plugin_id: self.metadata.id.clone(),
                        title: bookmark.title.clone(),
                        subtitle: format!("📁 {} | {}", bookmark.folder, bookmark.url),
                        icon: WoxImage::emoji("🔖".to_string()),
                        score: score as i32,
                        context_data: serde_json::to_value(&bookmark)?,
                        actions: vec![
                            Action {
                                id: "open".to_string(),
                                name: "打开".to_string(),
                                icon: None,
                                is_default: true,
                                hotkey: None,
                                prevent_hide: false,
                            },
                            Action {
                                id: "copy".to_string(),
                                name: "复制链接".to_string(),
                                icon: None,
                                is_default: false,
                                hotkey: None,
                                prevent_hide: false,
                            },
                        ],
                        preview: None,
                        refreshable: false,
                        group: None,
                    });
                }
            }
        }

        // 搜索历史记录
        if search_history {
            let history = self.history.read().await;
            for entry in history.iter().take(100) {
                let title_score = matcher.fuzzy_match(&entry.title, search_term).unwrap_or(0);
                let url_score = matcher.fuzzy_match(&entry.url, search_term).unwrap_or(0);
                let score = title_score.max(url_score);

                if score > 30 {
                    results.push(QueryResult {
                        id: entry.url.clone(),
                        plugin_id: self.metadata.id.clone(),
                        title: if entry.title.is_empty() { entry.url.clone() } else { entry.title.clone() },
                        subtitle: format!("🕒 访问 {} 次 | {}", entry.visit_count, entry.url),
                        icon: WoxImage::emoji("📜".to_string()),
                        score: (score as i32) + (entry.visit_count / 10),
                        context_data: serde_json::to_value(&entry)?,
                        actions: vec![
                            Action {
                                id: "open".to_string(),
                                name: "打开".to_string(),
                                icon: None,
                                is_default: true,
                                hotkey: None,
                                prevent_hide: false,
                            },
                            Action {
                                id: "copy".to_string(),
                                name: "复制链接".to_string(),
                                icon: None,
                                is_default: false,
                                hotkey: None,
                                prevent_hide: false,
                            },
                        ],
                        preview: None,
                        refreshable: false,
                        group: None,
                    });
                }
            }
        }

        // 按分数排序
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(20);

        Ok(results)
    }

    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        match action_id {
            "open" => {
                // 在默认浏览器中打开
                #[cfg(target_os = "windows")]
                {
                    std::process::Command::new("cmd")
                        .args(&["/C", "start", result_id])
                        .spawn()?;
                }
                
                #[cfg(not(target_os = "windows"))]
                {
                    std::process::Command::new("xdg-open")
                        .arg(result_id)
                        .spawn()?;
                }
                
                Ok(())
            }
            "copy" => {
                // 复制到剪贴板
                use arboard::Clipboard;
                let mut clipboard = Clipboard::new()?;
                clipboard.set_text(result_id)?;
                tracing::info!("Copied URL to clipboard: {}", result_id);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unknown action: {}", action_id)),
        }
    }
}
