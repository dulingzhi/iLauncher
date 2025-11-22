// 搜索历史管理器

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

const MAX_HISTORY_SIZE: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHistoryItem {
    pub query: String,
    pub timestamp: DateTime<Utc>,
    pub result_count: usize,
}

pub struct SearchHistoryManager {
    history: Arc<RwLock<Vec<SearchHistoryItem>>>,
    storage_path: String,
}

impl SearchHistoryManager {
    pub fn new(storage_path: String) -> Self {
        let manager = Self {
            history: Arc::new(RwLock::new(Vec::new())),
            storage_path,
        };
        
        // 尝试加载历史记录
        if let Err(e) = manager.load_blocking() {
            tracing::warn!("Failed to load search history: {}", e);
        }
        
        manager
    }
    
    /// 添加搜索记录
    pub async fn add(&self, query: String, result_count: usize) -> Result<()> {
        if query.trim().is_empty() {
            return Ok(());
        }
        
        let mut history = self.history.write().await;
        
        // 移除重复项（保留最新的）
        history.retain(|item| item.query != query);
        
        // 添加新记录
        history.insert(0, SearchHistoryItem {
            query,
            timestamp: Utc::now(),
            result_count,
        });
        
        // 限制历史记录数量
        if history.len() > MAX_HISTORY_SIZE {
            history.truncate(MAX_HISTORY_SIZE);
        }
        
        drop(history);
        
        // 异步保存
        self.save().await?;
        
        Ok(())
    }
    
    /// 获取历史记录
    pub async fn get_history(&self) -> Vec<SearchHistoryItem> {
        self.history.read().await.clone()
    }
    
    /// 清空历史记录
    pub async fn clear(&self) -> Result<()> {
        self.history.write().await.clear();
        self.save().await?;
        Ok(())
    }
    
    /// 删除指定记录
    pub async fn remove(&self, query: &str) -> Result<()> {
        let mut history = self.history.write().await;
        history.retain(|item| item.query != query);
        drop(history);
        self.save().await?;
        Ok(())
    }
    
    /// 保存历史记录
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
    
    /// 加载历史记录（同步版本，用于初始化）
    fn load_blocking(&self) -> Result<()> {
        if !std::path::Path::new(&self.storage_path).exists() {
            return Ok(());
        }
        
        let content = std::fs::read_to_string(&self.storage_path)?;
        let history: Vec<SearchHistoryItem> = serde_json::from_str(&content)?;
        
        // 使用 blocking API 写入
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            handle.block_on(async {
                *self.history.write().await = history;
            });
        }
        
        Ok(())
    }
}
