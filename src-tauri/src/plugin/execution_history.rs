// 运行历史插件 - 记录和快速重启应用

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

const MAX_HISTORY: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: WoxImage,
    pub plugin_id: String,
    pub action_id: String,
    pub execution_count: usize,
    pub last_executed: DateTime<Utc>,
}

pub struct ExecutionHistoryPlugin {
    metadata: PluginMetadata,
    history: Arc<RwLock<Vec<ExecutionRecord>>>,
    storage_path: String,
}

impl ExecutionHistoryPlugin {
    pub fn new(storage_path: String) -> Self {
        let plugin = Self {
            metadata: PluginMetadata {
                id: "execution-history".to_string(),
                name: "运行历史".to_string(),
                description: "显示最近运行的应用程序".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("🕐"),
                trigger_keywords: vec!["history".to_string(), "recent".to_string(), "lishi".to_string()],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: PluginType::Native,
            },
            history: Arc::new(RwLock::new(Vec::new())),
            storage_path: storage_path.clone(),
        };
        
        // 异步加载历史记录
        let history_clone = plugin.history.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::load_async(&storage_path, history_clone).await {
                tracing::warn!("Failed to load execution history: {}", e);
            }
        });
        
        plugin
    }
    
    /// 异步加载历史记录
    async fn load_async(storage_path: &str, history: Arc<RwLock<Vec<ExecutionRecord>>>) -> Result<()> {
        if !std::path::Path::new(storage_path).exists() {
            return Ok(());
        }
        
        let content = tokio::fs::read_to_string(storage_path).await?;
        let records: Vec<ExecutionRecord> = serde_json::from_str(&content)?;
        
        *history.write().await = records;
        
        Ok(())
    }
    
    /// 记录执行
    pub async fn record_execution(
        &self,
        id: String,
        title: String,
        subtitle: String,
        icon: WoxImage,
        plugin_id: String,
        action_id: String,
    ) -> Result<()> {
        let mut history = self.history.write().await;
        
        // 查找是否已存在
        if let Some(pos) = history.iter().position(|r| r.id == id && r.action_id == action_id) {
            let mut record = history.remove(pos);
            record.execution_count += 1;
            record.last_executed = Utc::now();
            history.insert(0, record);
        } else {
            // 新记录
            history.insert(0, ExecutionRecord {
                id,
                title,
                subtitle,
                icon,
                plugin_id,
                action_id,
                execution_count: 1,
                last_executed: Utc::now(),
            });
        }
        
        // 限制数量
        if history.len() > MAX_HISTORY {
            history.truncate(MAX_HISTORY);
        }
        
        drop(history);
        
        // 异步保存
        self.save().await?;
        
        Ok(())
    }
    
    /// 获取历史记录
    pub async fn get_history(&self) -> Vec<ExecutionRecord> {
        self.history.read().await.clone()
    }
    
    /// 清空历史
    pub async fn clear(&self) -> Result<()> {
        self.history.write().await.clear();
        self.save().await?;
        Ok(())
    }
    
    /// 删除指定记录
    pub async fn remove(&self, id: &str, action_id: &str) -> Result<()> {
        let mut history = self.history.write().await;
        history.retain(|r| !(r.id == id && r.action_id == action_id));
        drop(history);
        self.save().await?;
        Ok(())
    }
    
    /// 保存历史
    async fn save(&self) -> Result<()> {
        let history = self.history.read().await.clone();
        let storage_path = self.storage_path.clone();
        
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = std::path::Path::new(&storage_path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_string_pretty(&history)?;
            std::fs::write(&storage_path, json)?;
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        
        Ok(())
    }
}

#[async_trait]
impl Plugin for ExecutionHistoryPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        // 加载配置以获取禁用的插件列表
        let disabled_plugins = match crate::storage::StorageManager::new() {
            Ok(storage) => {
                match storage.load_config().await {
                    Ok(config) => config.plugins.disabled_plugins,
                    Err(_) => Vec::new(),
                }
            }
            Err(_) => Vec::new(),
        };
        
        let search = ctx.search.trim().to_lowercase();
        let history = self.history.read().await;
        
        let mut results = Vec::new();
        
        for record in history.iter() {
            // 跳过禁用插件的历史记录
            if disabled_plugins.contains(&record.plugin_id) {
                continue;
            }
            
            // 如果有搜索词，进行过滤
            if !search.is_empty() {
                let title_lower = record.title.to_lowercase();
                let subtitle_lower = record.subtitle.to_lowercase();
                
                if !title_lower.contains(&search) && !subtitle_lower.contains(&search) {
                    continue;
                }
            }
            
            // 计算相对时间
            let duration = Utc::now().signed_duration_since(record.last_executed);
            let time_str = if duration.num_minutes() < 1 {
                "刚刚".to_string()
            } else if duration.num_minutes() < 60 {
                format!("{} 分钟前", duration.num_minutes())
            } else if duration.num_hours() < 24 {
                format!("{} 小时前", duration.num_hours())
            } else {
                format!("{} 天前", duration.num_days())
            };
            
            results.push(QueryResult {
                id: format!("{}:{}", record.id, record.action_id),
                title: record.title.clone(),
                subtitle: format!("{} | {} | 运行 {} 次", record.subtitle, time_str, record.execution_count),
                icon: record.icon.clone(),
                preview: None,
                score: 100 - results.len() as i32, // 按时间顺序排序
                context_data: serde_json::json!({
                    "original_id": record.id,
                    "action_id": record.action_id,
                    "plugin_id": record.plugin_id,
                }),
                group: Some("运行历史".to_string()),
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![
                    Action {
                        id: "execute".to_string(),
                        name: "再次运行".to_string(),
                        icon: Some(WoxImage::emoji("▶️")),
                        hotkey: None,
                        is_default: true,
                        prevent_hide: false,
                    },
                    Action {
                        id: "remove".to_string(),
                        name: "从历史中删除".to_string(),
                        icon: Some(WoxImage::emoji("🗑️")),
                        hotkey: None,
                        is_default: false,
                        prevent_hide: true,
                    },
                ],
            });
        }
        
        // 如果没有历史记录
        if results.is_empty() && search.is_empty() {
            results.push(QueryResult {
                id: "empty".to_string(),
                title: "暂无运行历史".to_string(),
                subtitle: "开始使用 iLauncher 后会自动记录运行历史".to_string(),
                icon: WoxImage::emoji("📭"),
                preview: None,
                score: 100,
                context_data: serde_json::Value::Null,
                group: None,
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![],
            });
        }
        
        Ok(results)
    }
    
    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        if action_id == "remove" {
            // 解析 result_id
            let parts: Vec<&str> = result_id.split(':').collect();
            if parts.len() == 2 {
                self.remove(parts[0], parts[1]).await?;
                tracing::info!("Removed from execution history: {}", result_id);
            }
            return Ok(());
        }
        
        // execute 操作需要转发到原插件
        // 这里返回错误，让调用者处理
        Err(anyhow::anyhow!("Execute action should be handled by caller"))
    }
}
