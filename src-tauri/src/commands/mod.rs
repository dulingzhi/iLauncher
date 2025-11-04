// Tauri Commands - 前端调用的 Rust 函数

use crate::clipboard::ClipboardManager;
use crate::core::types::*;
use crate::plugin::PluginManager;
use crate::preview;
use crate::storage::{AppConfig, StorageManager};
use crate::statistics::StatisticsManager;
use tauri::State;

/// 查询命令
#[tauri::command]
pub async fn query(
    input: String,
    manager: State<'_, PluginManager>,
    stats: State<'_, StatisticsManager>,
) -> Result<Vec<QueryResult>, String> {
    // 记录查询
    if !input.is_empty() {
        let _ = stats.record_query(&input).await;
    }
    
    // 执行查询
    let mut results = manager.query(&input).await.map_err(|e| e.to_string())?;
    
    // 根据历史使用情况调整分数
    for result in &mut results {
        if let Ok(usage_count) = stats.get_result_score(&result.id, &result.plugin_id).await {
            // 给常用结果加分（每次使用加10分）
            result.score += usage_count * 10;
        }
    }
    
    // 重新排序
    results.sort_by(|a, b| b.score.cmp(&a.score));
    
    Ok(results)
}

/// 执行操作
#[tauri::command]
pub async fn execute_action(
    result_id: String,
    action_id: String,
    plugin_id: String,
    title: String,
    manager: State<'_, PluginManager>,
    stats: State<'_, StatisticsManager>,
) -> Result<(), String> {
    // 记录统计
    let _ = stats.record_result_click(&result_id, &plugin_id, &title).await;
    let _ = stats.record_plugin_usage(&plugin_id).await;
    
    // 执行操作
    manager.execute(&result_id, &action_id, &plugin_id).await.map_err(|e| e.to_string())
}

/// 获取插件列表
#[tauri::command]
pub async fn get_plugins(manager: State<'_, PluginManager>) -> Result<Vec<PluginMetadata>, String> {
    Ok(manager.get_plugins())
}

/// 显示应用
#[tauri::command]
pub async fn show_app(window: tauri::Window) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// 隐藏应用
#[tauri::command]
pub async fn hide_app(window: tauri::Window) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())?;
    Ok(())
}

/// 切换显示/隐藏
#[tauri::command]
pub async fn toggle_app(window: tauri::Window) -> Result<(), String> {
    if window.is_visible().map_err(|e| e.to_string())? {
        window.hide().map_err(|e| e.to_string())?;
    } else {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 加载配置
#[tauri::command]
pub async fn load_config(storage: State<'_, StorageManager>) -> Result<AppConfig, String> {
    storage.load_config().await.map_err(|e| e.to_string())
}

/// 保存配置
#[tauri::command]
pub async fn save_config(
    config: AppConfig,
    storage: State<'_, StorageManager>,
) -> Result<(), String> {
    storage.save_config(&config).await.map_err(|e| e.to_string())
}

/// 清除缓存
#[tauri::command]
pub async fn clear_cache(storage: State<'_, StorageManager>) -> Result<(), String> {
    storage.clear_cache().await.map_err(|e| e.to_string())
}

/// 获取存储路径
#[tauri::command]
pub async fn get_storage_paths(storage: State<'_, StorageManager>) -> Result<StoragePaths, String> {
    Ok(StoragePaths {
        data_dir: storage.get_data_dir().to_string_lossy().to_string(),
        cache_dir: storage.get_cache_dir().to_string_lossy().to_string(),
    })
}

/// 获取统计信息
#[tauri::command]
pub async fn get_statistics(stats: State<'_, StatisticsManager>) -> Result<Statistics, String> {
    let top_queries = stats.get_top_queries(10).await.map_err(|e| e.to_string())?;
    let top_results = stats.get_top_results(10).await.map_err(|e| e.to_string())?;
    
    Ok(Statistics {
        top_queries: top_queries.into_iter().map(|q| QueryStatInfo {
            query: q.query,
            count: q.count,
            last_used: q.last_used.to_rfc3339(),
        }).collect(),
        top_results: top_results.into_iter().map(|r| ResultStatInfo {
            title: r.title,
            count: r.count,
            plugin_id: r.plugin_id,
        }).collect(),
    })
}

/// 清除统计数据
#[tauri::command]
pub async fn clear_statistics(stats: State<'_, StatisticsManager>) -> Result<(), String> {
    stats.cleanup_old_data().await.map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct Statistics {
    pub top_queries: Vec<QueryStatInfo>,
    pub top_results: Vec<ResultStatInfo>,
}

#[derive(serde::Serialize)]
pub struct QueryStatInfo {
    pub query: String,
    pub count: i32,
    pub last_used: String,
}

#[derive(serde::Serialize)]
pub struct ResultStatInfo {
    pub title: String,
    pub count: i32,
    pub plugin_id: String,
}

#[derive(serde::Serialize)]
pub struct StoragePaths {
    pub data_dir: String,
    pub cache_dir: String,
}

/// 读取文件预览
#[tauri::command]
pub async fn read_file_preview(path: String) -> Result<preview::FilePreview, String> {
    preview::read_file_preview(&path).await.map_err(|e| e.to_string())
}

/// 获取剪贴板历史
#[tauri::command]
pub async fn get_clipboard_history(
    clipboard: State<'_, ClipboardManager>,
) -> Result<Vec<crate::clipboard::ClipboardItem>, String> {
    Ok(clipboard.get_history())
}

/// 复制到剪贴板
#[tauri::command]
pub async fn copy_to_clipboard(
    content: String,
    clipboard: State<'_, ClipboardManager>,
) -> Result<(), String> {
    clipboard.copy_to_clipboard(&content)
}

/// 更新剪贴板项时间戳
#[tauri::command]
pub async fn update_clipboard_timestamp(
    id: String,
    clipboard: State<'_, ClipboardManager>,
) -> Result<bool, String> {
    Ok(clipboard.update_timestamp(&id))
}

/// 删除剪贴板项
#[tauri::command]
pub async fn delete_clipboard_item(
    id: String,
    clipboard: State<'_, ClipboardManager>,
) -> Result<bool, String> {
    Ok(clipboard.delete_item(&id))
}

/// 切换收藏状态
#[tauri::command]
pub async fn toggle_clipboard_favorite(
    id: String,
    clipboard: State<'_, ClipboardManager>,
) -> Result<bool, String> {
    Ok(clipboard.toggle_favorite(&id))
}

/// 清空剪贴板历史
#[tauri::command]
pub async fn clear_clipboard_history(
    clipboard: State<'_, ClipboardManager>,
) -> Result<(), String> {
    clipboard.clear();
    Ok(())
}

