// 智能排序算法模块
// 基于多维度因素计算结果相关性分数

use crate::core::types::QueryResult;
use chrono::{DateTime, Utc, Duration};

/// 排序因素权重配置
#[derive(Debug, Clone)]
pub struct RankingWeights {
    /// 文本匹配分数权重 (默认: 1.0)
    pub text_match: f64,
    /// 使用频率权重 (默认: 2.0)
    pub usage_frequency: f64,
    /// 最近使用时间权重 (默认: 1.5)
    pub recency: f64,
    /// 结果类型权重 (默认: 0.5)
    pub result_type: f64,
    /// MRU 提升权重 (默认: 3.0)
    pub mru_boost: f64,
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self {
            text_match: 1.0,
            usage_frequency: 2.0,
            recency: 1.5,
            result_type: 0.5,
            mru_boost: 3.0,
        }
    }
}

/// 智能排序器
pub struct IntelligentRanker {
    weights: RankingWeights,
}

impl IntelligentRanker {
    pub fn new() -> Self {
        Self {
            weights: RankingWeights::default(),
        }
    }
    
    /// 计算综合排序分数
    pub fn calculate_score(
        &self,
        result: &QueryResult,
        query: &str,
        usage_count: u32,
        last_used: Option<DateTime<Utc>>,
        is_mru: bool,
    ) -> f64 {
        let mut total_score = 0.0;
        
        // 1. 文本匹配分数(基础分数)
        let text_score = self.calculate_text_match_score(result, query);
        total_score += text_score * self.weights.text_match;
        
        // 2. 使用频率分数
        let frequency_score = self.calculate_frequency_score(usage_count);
        total_score += frequency_score * self.weights.usage_frequency;
        
        // 3. 最近使用时间分数
        if let Some(last_used_time) = last_used {
            let recency_score = self.calculate_recency_score(last_used_time);
            total_score += recency_score * self.weights.recency;
        }
        
        // 4. 结果类型分数
        let type_score = self.calculate_type_score(result);
        total_score += type_score * self.weights.result_type;
        
        // 5. MRU 提升
        if is_mru {
            total_score += 100.0 * self.weights.mru_boost;
        }
        
        total_score
    }
    
    /// 计算文本匹配分数 (0-100)
    fn calculate_text_match_score(&self, result: &QueryResult, query: &str) -> f64 {
        if query.is_empty() {
            return 0.0;
        }
        
        let query_lower = query.to_lowercase();
        let title_lower = result.title.to_lowercase();
        let subtitle_lower = result.subtitle.to_lowercase();
        
        let mut score = 0.0;
        
        // 精确匹配最高分
        if title_lower == query_lower {
            score += 100.0;
        } else if title_lower.starts_with(&query_lower) {
            // 前缀匹配次高分
            score += 80.0;
        } else if title_lower.contains(&query_lower) {
            // 包含匹配
            score += 60.0;
        }
        
        // 副标题匹配
        if subtitle_lower.contains(&query_lower) {
            score += 20.0;
        }
        
        // 首字母缩写匹配 (例如: "gc" 匹配 "Git Client")
        if self.matches_initials(&title_lower, &query_lower) {
            score += 40.0;
        }
        
        // 连续字符匹配度
        let continuity_score = self.calculate_continuity_score(&title_lower, &query_lower);
        score += continuity_score * 30.0;
        
        // 使用原始 score 作为基础
        score += result.score as f64 * 0.1;
        
        score.min(100.0)
    }
    
    /// 计算使用频率分数 (0-100)
    fn calculate_frequency_score(&self, usage_count: u32) -> f64 {
        // 对数增长,避免频率过高主导排序
        if usage_count == 0 {
            return 0.0;
        }
        
        // log10(count + 1) * 20,最高100分
        ((usage_count as f64 + 1.0).log10() * 50.0).min(100.0)
    }
    
    /// 计算最近使用时间分数 (0-100)
    fn calculate_recency_score(&self, last_used: DateTime<Utc>) -> f64 {
        let now = Utc::now();
        let duration = now.signed_duration_since(last_used);
        
        // 时间衰减曲线
        if duration < Duration::minutes(5) {
            100.0  // 5分钟内: 满分
        } else if duration < Duration::hours(1) {
            80.0   // 1小时内: 80分
        } else if duration < Duration::hours(24) {
            60.0   // 1天内: 60分
        } else if duration < Duration::days(7) {
            40.0   // 1周内: 40分
        } else if duration < Duration::days(30) {
            20.0   // 1月内: 20分
        } else {
            10.0   // 更久: 10分
        }
    }
    
    /// 计算结果类型分数 (0-50)
    fn calculate_type_score(&self, result: &QueryResult) -> f64 {
        // 根据结果类型给予不同权重
        let base_score = match result.plugin_id.as_str() {
            "file-search" => {
                // 文件类型细分
                if result.subtitle.ends_with(".exe") {
                    30.0  // 可执行文件优先
                } else if result.subtitle.ends_with(".lnk") || result.subtitle.contains("快捷方式") {
                    25.0  // 快捷方式其次
                } else {
                    15.0  // 普通文件
                }
            },
            "app-search" => 40.0,        // 应用程序高优先级
            "git-projects" => 35.0,      // Git 项目高优先级
            "browser-bookmarks" => 20.0, // 书签中等
            "browser-history" => 10.0,   // 历史记录较低
            "process-manager" => 30.0,   // 进程管理中高
            _ => 15.0,                   // 其他默认
        };
        
        base_score
    }
    
    /// 检查是否匹配首字母缩写
    fn matches_initials(&self, text: &str, query: &str) -> bool {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() < query.len() {
            return false;
        }
        
        let initials: String = words.iter()
            .filter_map(|w| w.chars().next())
            .collect();
        
        initials.to_lowercase().starts_with(query)
    }
    
    /// 计算连续字符匹配度 (0.0-1.0)
    fn calculate_continuity_score(&self, text: &str, query: &str) -> f64 {
        if query.is_empty() {
            return 0.0;
        }
        
        let mut max_continuous = 0;
        let mut current_continuous = 0;
        let query_chars: Vec<char> = query.chars().collect();
        
        let mut query_idx = 0;
        for ch in text.chars() {
            if query_idx < query_chars.len() && ch == query_chars[query_idx] {
                current_continuous += 1;
                query_idx += 1;
            } else if current_continuous > 0 {
                max_continuous = max_continuous.max(current_continuous);
                current_continuous = 0;
            }
        }
        max_continuous = max_continuous.max(current_continuous);
        
        (max_continuous as f64) / (query.len() as f64)
    }
    
    /// 对结果列表进行智能排序
    pub fn rank_results(
        &self,
        results: &mut Vec<QueryResult>,
        query: &str,
        usage_stats: &[(String, u32, Option<DateTime<Utc>>)],  // (id, count, last_used)
        mru_ids: &[String],
    ) {
        // 构建统计数据映射
        let stats_map: std::collections::HashMap<&str, (u32, Option<DateTime<Utc>>)> = 
            usage_stats.iter()
                .map(|(id, count, last_used)| (id.as_str(), (*count, *last_used)))
                .collect();
        
        let mru_set: std::collections::HashSet<&str> = 
            mru_ids.iter().map(|s| s.as_str()).collect();
        
        // 计算每个结果的综合分数
        for result in results.iter_mut() {
            let (usage_count, last_used) = stats_map.get(result.id.as_str())
                .copied()
                .unwrap_or((0, None));
            
            let is_mru = mru_set.contains(result.id.as_str());
            
            let final_score = self.calculate_score(
                result,
                query,
                usage_count,
                last_used,
                is_mru,
            );
            
            // 更新结果分数
            result.score = final_score as i32;
        }
        
        // 按分数降序排序
        results.sort_by(|a, b| {
            b.score.cmp(&a.score)
                .then_with(|| a.title.cmp(&b.title))  // 分数相同时按标题排序
        });
    }
}

impl Default for IntelligentRanker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::WoxImage;
    
    #[test]
    fn test_text_match_score() {
        let ranker = IntelligentRanker::new();
        let result = QueryResult {
            id: "test".to_string(),
            title: "Visual Studio Code".to_string(),
            subtitle: "Code Editor".to_string(),
            icon: WoxImage::Emoji("🔍".to_string()),
            score: 50,
            actions: vec![],
            plugin_id: "app-search".to_string(),
            context_data: serde_json::Value::Null,
            preview: None,
            refreshable: false,
            group: None,
        };
        
        // 精确匹配
        let score = ranker.calculate_text_match_score(&result, "visual studio code");
        assert!(score > 90.0);
        
        // 前缀匹配
        let score = ranker.calculate_text_match_score(&result, "visual");
        assert!(score > 75.0 && score < 90.0);
        
        // 首字母缩写
        let score = ranker.calculate_text_match_score(&result, "vsc");
        assert!(score > 35.0);
    }
    
    #[test]
    fn test_frequency_score() {
        let ranker = IntelligentRanker::new();
        
        assert_eq!(ranker.calculate_frequency_score(0), 0.0);
        assert!(ranker.calculate_frequency_score(1) > 0.0);
        assert!(ranker.calculate_frequency_score(100) > ranker.calculate_frequency_score(10));
        assert!(ranker.calculate_frequency_score(1000) < 100.0);
    }
    
    #[test]
    fn test_recency_score() {
        let ranker = IntelligentRanker::new();
        
        // 刚刚使用
        let now = Utc::now();
        assert_eq!(ranker.calculate_recency_score(now), 100.0);
        
        // 1小时前
        let one_hour_ago = now - Duration::hours(1);
        let score = ranker.calculate_recency_score(one_hour_ago);
        assert!(score >= 60.0 && score <= 80.0);
        
        // 1周前
        let one_week_ago = now - Duration::days(7);
        let score = ranker.calculate_recency_score(one_week_ago);
        assert!(score >= 20.0 && score <= 40.0);
    }
}
