// 存储相关命令

use crate::storage::{AppConfig, StorageManager};
use tauri::State;

#[tauri::command]
pub async fn load_config(storage: State<'_, StorageManager>) -> Result<AppConfig, String> {
    storage.load_config().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_config(
    config: AppConfig,
    storage: State<'_, StorageManager>,
) -> Result<(), String> {
    storage.save_config(&config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_cache(storage: State<'_, StorageManager>) -> Result<(), String> {
    storage.clear_cache().await.map_err(|e| e.to_string())
}

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
