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
    #[serde(default)]
    pub frequency: usize,  // 搜索频率（使用次数）
    #[serde(default)]
    pub last_executed: Option<DateTime<Utc>>,  // 最后一次执行时间
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
        
        // 查找是否已存在相同的查询
        if let Some(existing) = history.iter_mut().find(|item| item.query == query) {
            // 更新频率和时间戳
            existing.frequency += 1;
            existing.timestamp = Utc::now();
            existing.result_count = result_count;
        } else {
            // 添加新记录
            history.insert(0, SearchHistoryItem {
                query,
                timestamp: Utc::now(),
                result_count,
                frequency: 1,
                last_executed: None,
            });
        }
        
        // 限制历史记录数量
        if history.len() > MAX_HISTORY_SIZE {
            history.truncate(MAX_HISTORY_SIZE);
        }
        
        drop(history);
        
        // 异步保存
        self.save().await?;
        
        Ok(())
    }
    
    /// 记录执行（当用户选择并执行某个搜索结果时调用）
    pub async fn record_execution(&self, query: &str) -> Result<()> {
        let mut history = self.history.write().await;
        
        if let Some(item) = history.iter_mut().find(|item| item.query == query) {
            item.last_executed = Some(Utc::now());
            item.frequency += 1;
        }
        
        drop(history);
        self.save().await?;
        
        Ok(())
    }
    
    /// 获取历史记录（智能排序）
    pub async fn get_history(&self) -> Vec<SearchHistoryItem> {
        let mut history = self.history.read().await.clone();
        
        // 智能排序算法：综合考虑频率、时效性、执行情况
        let now = Utc::now();
        history.sort_by(|a, b| {
            let score_a = calculate_relevance_score(a, &now);
            let score_b = calculate_relevance_score(b, &now);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        history
    }
    
    /// 获取搜索建议（根据前缀匹配）
    pub async fn get_suggestions(&self, prefix: &str, limit: usize) -> Vec<SearchHistoryItem> {
        if prefix.trim().is_empty() {
            return Vec::new();
        }
        
        let history = self.get_history().await;
        let prefix_lower = prefix.to_lowercase();
        
        history
            .into_iter()
            .filter(|item| item.query.to_lowercase().starts_with(&prefix_lower))
            .take(limit)
            .collect()
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

/// 计算搜索项的相关性得分
/// 综合考虑：
/// 1. 频率（使用次数）
/// 2. 时效性（最近搜索时间）
/// 3. 执行情况（是否被执行过）
fn calculate_relevance_score(item: &SearchHistoryItem, now: &DateTime<Utc>) -> f64 {
    // 基础分数：频率权重
    let frequency_score = item.frequency as f64 * 10.0;
    
    // 时效性分数：最近搜索越近分数越高
    let hours_since_search = (now.timestamp() - item.timestamp.timestamp()) as f64 / 3600.0;
    let recency_score = if hours_since_search < 1.0 {
        100.0
    } else if hours_since_search < 24.0 {
        50.0 / hours_since_search
    } else if hours_since_search < 168.0 {  // 1周
        20.0 / (hours_since_search / 24.0)
    } else {
        5.0 / (hours_since_search / 168.0)
    };
    
    // 执行加成：如果被执行过，额外加分
    let execution_bonus = if let Some(last_exec) = item.last_executed {
        let hours_since_exec = (now.timestamp() - last_exec.timestamp()) as f64 / 3600.0;
        if hours_since_exec < 24.0 {
            50.0
        } else if hours_since_exec < 168.0 {
            20.0 / (hours_since_exec / 24.0)
        } else {
            5.0
        }
    } else {
        0.0
    };
    
    // 综合得分
    frequency_score + recency_score + execution_bonus
}
