// 智能建议系统 - 基于使用频率和上下文的推荐引擎
use crate::statistics::StatisticsManager;
use anyhow::Result;
use chrono::{DateTime, Utc, Timelike};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 智能建议引擎
pub struct SmartSuggestionEngine {
    stats_manager: Arc<RwLock<StatisticsManager>>,
}

/// 建议项
#[derive(Debug, Clone, serde::Serialize)]
pub struct Suggestion {
    pub result_id: String,
    pub title: String,
    pub subtitle: String,
    pub score: f64,
    pub reason: String, // 推荐原因
}

impl SmartSuggestionEngine {
    pub fn new(stats_manager: Arc<RwLock<StatisticsManager>>) -> Self {
        Self { stats_manager }
    }

    /// 获取智能建议
    pub async fn get_suggestions(&self, context: &SuggestionContext, limit: usize) -> Result<Vec<Suggestion>> {
        let mut suggestions = Vec::new();

        // 1. 基于使用频率的建议
        let frequent = self.get_frequent_suggestions(10).await?;
        suggestions.extend(frequent);

        // 2. 基于时间段的建议（例如：早上推荐打开IDE，晚上推荐关闭应用）
        let time_based = self.get_time_based_suggestions(&context.current_time, 5).await?;
        suggestions.extend(time_based);

        // 3. 基于最近使用的建议
        let recent = self.get_recent_suggestions(5).await?;
        suggestions.extend(recent);

        // 去重并排序
        let mut suggestion_map: HashMap<String, Suggestion> = HashMap::new();
        for suggestion in suggestions {
            let entry = suggestion_map.entry(suggestion.result_id.clone())
                .or_insert(suggestion.clone());
            // 合并分数（取最高分）
            if suggestion.score > entry.score {
                *entry = suggestion;
            }
        }

        let mut result: Vec<Suggestion> = suggestion_map.into_values().collect();
        result.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        result.truncate(limit);

        Ok(result)
    }

    /// 基于使用频率的建议
    pub async fn get_frequent_suggestions(&self, limit: usize) -> Result<Vec<Suggestion>> {
        let top_results = self.stats_manager.read().await.get_top_results(limit).await?;
        
        Ok(top_results
            .into_iter()
            .enumerate()
            .map(|(i, r)| Suggestion {
                result_id: r.result_id,
                title: r.title,
                subtitle: format!("使用 {} 次", r.count),
                score: 100.0 - (i as f64 * 5.0),
                reason: "常用项目".to_string(),
            })
            .collect())
    }

    /// 基于时间段的建议
    pub async fn get_time_based_suggestions(&self, current_time: &DateTime<Utc>, limit: usize) -> Result<Vec<Suggestion>> {
        let hour = current_time.hour();
        
        let suggestions = match hour {
            6..=9 => vec![
                Suggestion {
                    result_id: "morning_routine".to_string(),
                    title: "早间例行".to_string(),
                    subtitle: "打开常用工作应用".to_string(),
                    score: 80.0,
                    reason: "早上时段推荐".to_string(),
                },
            ],
            12..=14 => vec![
                Suggestion {
                    result_id: "lunch_break".to_string(),
                    title: "午休".to_string(),
                    subtitle: "锁定屏幕或打开娱乐应用".to_string(),
                    score: 70.0,
                    reason: "午间时段推荐".to_string(),
                },
            ],
            18..=23 => vec![
                Suggestion {
                    result_id: "evening_routine".to_string(),
                    title: "晚间例行".to_string(),
                    subtitle: "关闭工作应用或同步数据".to_string(),
                    score: 75.0,
                    reason: "晚间时段推荐".to_string(),
                },
            ],
            _ => vec![],
        };

        Ok(suggestions)
    }

    /// 基于最近使用的建议
    pub async fn get_recent_suggestions(&self, limit: usize) -> Result<Vec<Suggestion>> {
        let recent = self.stats_manager.read().await.get_top_results(limit * 2).await?;
        
        Ok(recent
            .into_iter()
            .take(limit)
            .map(|r| Suggestion {
                result_id: r.result_id,
                title: r.title,
                subtitle: "最近使用".to_string(),
                score: 60.0,
                reason: "最近访问".to_string(),
            })
            .collect())
    }
}

/// 建议上下文
#[derive(Debug, Clone)]
pub struct SuggestionContext {
    pub current_time: DateTime<Utc>,
    pub query: Option<String>,
    pub recent_queries: Vec<String>,
}

impl Default for SuggestionContext {
    fn default() -> Self {
        Self {
            current_time: Utc::now(),
            query: None,
            recent_queries: Vec::new(),
        }
    }
}
