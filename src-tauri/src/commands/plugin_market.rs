// 插件市场相关命令
use crate::plugin::plugin_installer::{InstalledPlugin, PluginInstaller, PluginRegistry};
use crate::plugin::plugin_store::{PluginDetails, PluginListItem, PluginStore, SearchParams};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// 插件市场状态
pub struct PluginMarketState {
    pub registry: Arc<PluginRegistry>,
    pub installer: Arc<PluginInstaller>,
    pub store: Arc<PluginStore>,
}

impl PluginMarketState {
    pub fn new(plugins_dir: PathBuf, cache_dir: PathBuf) -> Self {
        let registry = Arc::new(PluginRegistry::new(plugins_dir));
        let installer = Arc::new(PluginInstaller::new(registry.clone()));
        let store = Arc::new(PluginStore::new(cache_dir));

        Self {
            registry,
            installer,
            store,
        }
    }
}

/// 搜索插件
#[tauri::command]
pub async fn search_plugins(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    query: Option<String>,
    category: Option<String>,
    sort: Option<String>,
    page: u32,
) -> Result<serde_json::Value, String> {
    let state = state.read().await;

    let params = SearchParams {
        query,
        category,
        sort,
        page,
        per_page: 20,
    };

    state
        .store
        .search(params)
        .await
        .map(|result| serde_json::to_value(result).unwrap())
        .map_err(|e| e.to_string())
}

/// 获取插件详情
#[tauri::command]
pub async fn get_plugin_details(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    plugin_id: String,
) -> Result<PluginDetails, String> {
    let state = state.read().await;

    state
        .store
        .get_plugin_details(&plugin_id)
        .await
        .map_err(|e| e.to_string())
}

/// 下载并安装插件
#[tauri::command]
pub async fn install_plugin(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    plugin_id: String,
    version: Option<String>,
) -> Result<InstalledPlugin, String> {
    let state = state.read().await;

    // 1. 下载插件
    let ilp_path = state
        .store
        .download_plugin(&plugin_id, version.as_deref())
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    // 2. 安装插件
    let installed = state
        .installer
        .install(&ilp_path)
        .await
        .map_err(|e| format!("Installation failed: {}", e))?;

    // 3. 清理下载文件（可选）
    // tokio::fs::remove_file(&ilp_path).await.ok();

    Ok(installed)
}

/// 卸载插件
#[tauri::command]
pub async fn uninstall_plugin(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    plugin_id: String,
) -> Result<(), String> {
    let state = state.read().await;

    state
        .installer
        .uninstall(&plugin_id)
        .await
        .map_err(|e| e.to_string())
}

/// 更新插件
#[tauri::command]
pub async fn update_plugin(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    plugin_id: String,
) -> Result<InstalledPlugin, String> {
    let state = state.read().await;

    // 1. 下载最新版本
    let ilp_path = state
        .store
        .download_plugin(&plugin_id, None)
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    // 2. 更新插件
    let installed = state
        .installer
        .update(&plugin_id, &ilp_path)
        .await
        .map_err(|e| format!("Update failed: {}", e))?;

    Ok(installed)
}

/// 获取已安装插件列表
#[tauri::command]
pub async fn list_installed_plugins(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
) -> Result<Vec<InstalledPlugin>, String> {
    let state = state.read().await;

    Ok(state.registry.list_plugins().await)
}

/// 启用/禁用插件
#[tauri::command]
pub async fn toggle_plugin(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    plugin_id: String,
    enabled: bool,
) -> Result<(), String> {
    let state = state.read().await;

    state
        .registry
        .set_enabled(&plugin_id, enabled)
        .await
        .map_err(|e| e.to_string())
}

/// 更新插件设置
#[tauri::command]
pub async fn update_plugin_settings(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    plugin_id: String,
    settings: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    let state = state.read().await;

    state
        .registry
        .update_settings(&plugin_id, settings)
        .await
        .map_err(|e| e.to_string())
}

/// 检查插件更新
#[tauri::command]
pub async fn check_plugin_updates(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
) -> Result<Vec<(String, String)>, String> {
    let state = state.read().await;

    // 获取已安装插件
    let installed = state.registry.list_plugins().await;
    let plugin_versions: Vec<(String, String)> = installed
        .into_iter()
        .map(|p| (p.manifest.id, p.manifest.version))
        .collect();

    // 检查更新
    state
        .store
        .check_updates(plugin_versions)
        .await
        .map_err(|e| e.to_string())
}

/// 获取热门插件
#[tauri::command]
pub async fn get_popular_plugins(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    limit: u32,
) -> Result<Vec<PluginListItem>, String> {
    let state = state.read().await;

    state
        .store
        .get_popular_plugins(limit)
        .await
        .map_err(|e| e.to_string())
}

/// 获取最新插件
#[tauri::command]
pub async fn get_recent_plugins(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    limit: u32,
) -> Result<Vec<PluginListItem>, String> {
    let state = state.read().await;

    state
        .store
        .get_recent_plugins(limit)
        .await
        .map_err(|e| e.to_string())
}

/// 按分类获取插件
#[tauri::command]
pub async fn get_plugins_by_category(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    category: String,
    page: u32,
) -> Result<serde_json::Value, String> {
    let state = state.read().await;

    state
        .store
        .get_plugins_by_category(&category, page)
        .await
        .map(|result| serde_json::to_value(result).unwrap())
        .map_err(|e| e.to_string())
}

/// 清理下载缓存
#[tauri::command]
pub async fn clear_plugin_cache(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
) -> Result<(), String> {
    let state = state.read().await;

    state.store.clear_cache().await.map_err(|e| e.to_string())
}

/// 从本地 .ilp 文件安装插件
#[tauri::command]
pub async fn install_plugin_from_file(
    state: State<'_, Arc<RwLock<PluginMarketState>>>,
    file_path: String,
) -> Result<InstalledPlugin, String> {
    let state = state.read().await;

    let path = PathBuf::from(file_path);
    state
        .installer
        .install(&path)
        .await
        .map_err(|e| e.to_string())
}
