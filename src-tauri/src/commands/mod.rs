// Tauri Commands - 前端调用的 Rust 函数

use crate::core::types::*;
use crate::plugin::PluginManager;
use crate::storage::{AppConfig, StorageManager};
use tauri::State;

/// 查询命令
#[tauri::command]
pub async fn query(input: String, manager: State<'_, PluginManager>) -> Result<Vec<QueryResult>, String> {
    manager.query(&input).await.map_err(|e| e.to_string())
}

/// 执行操作
#[tauri::command]
pub async fn execute_action(
    result_id: String,
    action_id: String,
    manager: State<'_, PluginManager>,
) -> Result<(), String> {
    manager.execute(&result_id, &action_id).await.map_err(|e| e.to_string())
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

#[derive(serde::Serialize)]
pub struct StoragePaths {
    pub data_dir: String,
    pub cache_dir: String,
}
