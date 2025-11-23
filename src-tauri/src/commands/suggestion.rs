use crate::plugin::smart_suggestion::{SmartSuggestionEngine, Suggestion, SuggestionContext};
use crate::statistics::StatisticsManager;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// 获取智能推荐
#[tauri::command]
pub async fn get_smart_suggestions(
    query: String,
    limit: Option<usize>,
    stats_manager: State<'_, Arc<RwLock<StatisticsManager>>>,
) -> Result<Vec<Suggestion>, String> {
    let stats_arc = stats_manager.inner().clone();
    let engine = SmartSuggestionEngine::new(stats_arc);
    
    let context = SuggestionContext {
        query: if query.is_empty() { None } else { Some(query) },
        current_time: chrono::Utc::now(),
        recent_queries: vec![], // 可以从历史记录获取
    };
    
    let suggestions = engine.get_suggestions(&context, limit.unwrap_or(10)).await
        .map_err(|e| e.to_string())?;
    
    Ok(suggestions)
}

/// 获取频率推荐
#[tauri::command]
pub async fn get_frequent_suggestions(
    limit: usize,
    stats_manager: State<'_, Arc<RwLock<StatisticsManager>>>,
) -> Result<Vec<Suggestion>, String> {
    let stats_arc = stats_manager.inner().clone();
    let engine = SmartSuggestionEngine::new(stats_arc);
    
    let suggestions = engine.get_frequent_suggestions(limit).await
        .map_err(|e| e.to_string())?;
    
    Ok(suggestions)
}

/// 获取时间推荐
#[tauri::command]
pub async fn get_time_based_suggestions(
    limit: usize,
    stats_manager: State<'_, Arc<RwLock<StatisticsManager>>>,
) -> Result<Vec<Suggestion>, String> {
    let stats_arc = stats_manager.inner().clone();
    let engine = SmartSuggestionEngine::new(stats_arc);
    
    let now = chrono::Utc::now();
    let suggestions = engine.get_time_based_suggestions(&now, limit).await
        .map_err(|e| e.to_string())?;
    
    Ok(suggestions)
}

/// 获取最近推荐
#[tauri::command]
pub async fn get_recent_suggestions(
    limit: usize,
    stats_manager: State<'_, Arc<RwLock<StatisticsManager>>>,
) -> Result<Vec<Suggestion>, String> {
    let stats_arc = stats_manager.inner().clone();
    let engine = SmartSuggestionEngine::new(stats_arc);
    
    let suggestions = engine.get_recent_suggestions(limit).await
        .map_err(|e| e.to_string())?;
    
    Ok(suggestions)
}
