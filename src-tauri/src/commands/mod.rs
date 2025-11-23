// Tauri Commands - 前端调用的 Rust 函数

pub mod audit;
pub mod ai;
pub mod plugin_market; // 插件市场
pub mod suggestion;    // 智能推荐
pub mod workflow;      // 工作流

use crate::clipboard::ClipboardManager;
use crate::core::types::*;
use crate::plugin::PluginManager;
use crate::preview;
use crate::ranking::IntelligentRanker;
use crate::storage::{AppConfig, StorageManager};
use crate::statistics::StatisticsManager;
use tauri::{State, Emitter};

/// 查询命令
#[tauri::command]
pub async fn query(
    input: String,
    manager: State<'_, PluginManager>,
    stats: State<'_, StatisticsManager>,
    history: State<'_, crate::search_history::SearchHistoryManager>,
) -> Result<Vec<QueryResult>, String> {
    let query_start = std::time::Instant::now();
    tracing::debug!("🔍 Query started: '{}'", input);
    
    // 记录查询
    if !input.is_empty() {
        let _ = stats.record_query(&input).await;
    }
    
    // 🔥 步骤 1: 执行插件查询
    let plugin_query_start = std::time::Instant::now();
    let mut plugin_results = manager.query(&input).await.map_err(|e| e.to_string())?;
    let plugin_elapsed = plugin_query_start.elapsed();
    
    // 🔥 步骤 2: 使用智能排序算法
    let ranking_start = std::time::Instant::now();
    
    // 创建排序器
    let ranker = IntelligentRanker::new();
    
    // 获取 MRU 结果列表
    let mru_results = stats.get_top_results(50).await.unwrap_or_default();
    let mru_ids: Vec<String> = mru_results.iter().map(|r| r.result_id.clone()).collect();
    
    // 构建使用统计数据 (id, count, last_used)
    let mut usage_stats = Vec::new();
    for result in &plugin_results {
        if let Ok(count) = stats.get_result_score(&result.id, &result.plugin_id).await {
            // 查找最后使用时间
            let last_used = mru_results.iter()
                .find(|mru| mru.result_id == result.id)
                .map(|mru| mru.last_used);
            
            usage_stats.push((result.id.clone(), count as u32, last_used));
        }
    }
    
    // 执行智能排序
    ranker.rank_results(
        &mut plugin_results,
        &input,
        &usage_stats,
        &mru_ids,
    );
    
    let ranking_elapsed = ranking_start.elapsed();
    
    let total_elapsed = query_start.elapsed();
    tracing::info!(
        "✅ Query completed: '{}' → {} results in {:.2}ms (plugin: {:.2}ms, ranking: {:.2}ms)",
        input,
        plugin_results.len(),
        total_elapsed.as_secs_f64() * 1000.0,
        plugin_elapsed.as_secs_f64() * 1000.0,
        ranking_elapsed.as_secs_f64() * 1000.0
    );
    
    // 记录搜索历史
    if !input.trim().is_empty() && plugin_results.len() > 0 {
        let _ = history.add(input.clone(), plugin_results.len()).await;
    }
    
    Ok(plugin_results)
}

/// 执行操作
#[tauri::command]
pub async fn execute_action(
    result_id: String,
    action_id: String,
    plugin_id: String,
    title: String,
    subtitle: String,
    icon: WoxImage,
    manager: State<'_, PluginManager>,
    stats: State<'_, StatisticsManager>,
) -> Result<(), String> {
    // 记录统计
    let _ = stats.record_result_click(&result_id, &plugin_id, &title).await;
    let _ = stats.record_plugin_usage(&plugin_id).await;
    
    // 执行操作
    let result = manager.execute(&result_id, &action_id, &plugin_id).await.map_err(|e| e.to_string());
    
    // 如果执行成功，记录到运行历史（排除一些特殊插件）
    if result.is_ok() && !matches!(plugin_id.as_str(), 
        "execution-history" | "settings" | "clipboard" | "plugin-manager"
    ) {
        if let Some(exec_history) = manager.get_execution_history_plugin() {
            let _ = exec_history.record_execution(
                result_id.clone(),
                title.clone(),
                subtitle.clone(),
                icon.clone(),
                plugin_id.clone(),
                action_id.clone(),
            ).await;
            tracing::info!("Recorded to execution history: {}", title);
        }
    }
    
    result
}

/// 获取插件列表
#[tauri::command]
pub async fn get_plugins(manager: State<'_, PluginManager>) -> Result<Vec<PluginMetadata>, String> {
    Ok(manager.get_plugins())
}

/// 获取插件配置
#[tauri::command]
pub async fn get_plugin_config(
    plugin_id: String,
    storage: State<'_, StorageManager>,
) -> Result<serde_json::Value, String> {
    storage.get_plugin_config(&plugin_id).await.map_err(|e| e.to_string())
}

/// 保存插件配置
#[tauri::command]
pub async fn save_plugin_config(
    plugin_id: String,
    config: serde_json::Value,
    storage: State<'_, StorageManager>,
) -> Result<(), String> {
    storage.save_plugin_config(&plugin_id, config).await.map_err(|e| e.to_string())
}

/// 显示应用
#[tauri::command]
pub async fn show_app(window: tauri::Window) -> Result<(), String> {
    // 🔥 显示前先居中窗口
    window.center().map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// 隐藏应用
#[tauri::command]
pub async fn hide_app(window: tauri::Window) -> Result<(), String> {
    // 发送隐藏事件到前端，让前端根据配置清空搜索结果
    let _ = window.emit("app-hiding", ());
    window.hide().map_err(|e| e.to_string())?;
    Ok(())
}

/// 切换显示/隐藏
#[tauri::command]
pub async fn toggle_app(window: tauri::Window) -> Result<(), String> {
    if window.is_visible().map_err(|e| e.to_string())? {
        window.hide().map_err(|e| e.to_string())?;
    } else {
        // 🔥 显示前先居中窗口
        window.center().map_err(|e| e.to_string())?;
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

/// 获取配置（load_config 的别名）
#[tauri::command]
pub async fn get_config(storage: State<'_, StorageManager>) -> Result<AppConfig, String> {
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

/// 切换 MFT 开关（Windows only）
#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn toggle_mft(
    enabled: bool,
    storage: State<'_, StorageManager>,
) -> Result<(), String> {
    use std::process::Command;
    
    // 读取当前 file_search 插件配置
    let mut plugin_config = storage
        .get_plugin_config("file_search")
        .await
        .unwrap_or_else(|_| serde_json::json!({}));
    
    // 更新 use_mft 字段
    if let Some(obj) = plugin_config.as_object_mut() {
        obj.insert("use_mft".to_string(), serde_json::json!(enabled));
    }
    
    // 保存插件配置
    storage
        .save_plugin_config("file_search", plugin_config)
        .await
        .map_err(|e| e.to_string())?;
    
    tracing::info!("✓ File Search plugin config updated: use_mft = {}", enabled);
    
    if enabled {
        // 启动 MFT service 子进程（使用管理员权限）
        tracing::info!("MFT enabled, starting MFT service subprocess with admin rights...");
        
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Failed to get exe path: {}", e))?;
        
        // 使用 PowerShell Start-Process -Verb RunAs 请求管理员权限
        let ps_command = format!(
            "Start-Process -FilePath '{}' -ArgumentList '--mft-service' -Verb RunAs -WindowStyle Hidden",
            exe_path.display()
        );
        
        // 🔥 使用 CREATE_NO_WINDOW 标志隐藏控制台窗口
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        Command::new("powershell.exe")
            .args(["-WindowStyle", "Hidden", "-Command", &ps_command])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to start MFT service: {}", e))?;
        
        tracing::info!("✓ MFT service launch requested (UAC prompt will appear)");
    } else {
        // 停止 MFT service（发送信号或杀掉进程）
        tracing::info!("MFT disabled, stopping MFT service...");
        
        // 强制终止所有 MFT Service 进程
        #[cfg(target_os = "windows")]
        {
            // 🔥 使用 CREATE_NO_WINDOW 标志隐藏控制台窗口
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            
            // 查找并终止带有 --mft-service 参数的进程
            let _ = Command::new("powershell.exe")
                .args([
                    "-WindowStyle", "Hidden",
                    "-Command",
                    "Get-Process ilauncher | Where-Object { $_.CommandLine -like '*--mft-service*' } | Stop-Process -Force"
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
        
        tracing::info!("✓ MFT service stop requested");
    }
    
    Ok(())
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

/// 获取 MFT 扫描状态（Windows only）
#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn get_mft_status() -> Result<MftStatus, String> {
    use crate::utils::paths;
    
    let output_dir = paths::get_mft_database_dir()
        .map_err(|e| format!("Failed to get database directory: {}", e))?;
    
    // 检查数据库目录
    if !output_dir.exists() {
        return Ok(MftStatus {
            is_scanning: true,
            is_ready: false,
            database_exists: false,
            drives: vec![],
            total_files: 0,
            message: "MFT database not found. Scanner may not be running.".to_string(),
        });
    }
    
    // 检查各个盘符的数据库
    let mut drives = Vec::new();
    let mut total_files = 0u64;
    
    for drive in b'A'..=b'Z' {
        let drive_letter = drive as char;
        let db_path = output_dir.join(format!("{}.db", drive_letter));
        
        if db_path.exists() {
            // 检查数据库大小
            if let Ok(metadata) = std::fs::metadata(&db_path) {
                let size_mb = metadata.len() / 1024 / 1024;
                
                // 估算文件数（粗略：1MB ≈ 10000 文件）
                let estimated_files = (metadata.len() / 100) as u64;
                total_files += estimated_files;
                
                drives.push(MftDriveInfo {
                    letter: drive_letter,
                    database_size_mb: size_mb,
                    estimated_files,
                });
            }
        }
    }
    
    let is_ready = !drives.is_empty();
    let message = if is_ready {
        format!("MFT ready: {} drives, ~{} files indexed", drives.len(), total_files)
    } else {
        "MFT scanner is running initial scan...".to_string()
    };
    
    Ok(MftStatus {
        is_scanning: !is_ready,
        is_ready,
        database_exists: true,
        drives,
        total_files,
        message,
    })
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn get_mft_status() -> Result<MftStatus, String> {
    Ok(MftStatus {
        is_scanning: false,
        is_ready: false,
        database_exists: false,
        drives: vec![],
        total_files: 0,
        message: "MFT is only available on Windows".to_string(),
    })
}

#[derive(serde::Serialize)]
pub struct MftStatus {
    pub is_scanning: bool,
    pub is_ready: bool,
    pub database_exists: bool,
    pub drives: Vec<MftDriveInfo>,
    pub total_files: u64,
    pub message: String,
}

#[derive(serde::Serialize)]
pub struct MftDriveInfo {
    pub letter: char,
    pub database_size_mb: u64,
    pub estimated_files: u64,
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
    limit: Option<usize>,
    offset: Option<usize>,
    clipboard: State<'_, ClipboardManager>,
) -> Result<Vec<crate::clipboard::ClipboardItem>, String> {
    clipboard.get_history(limit.unwrap_or(100), offset.unwrap_or(0))
        .map_err(|e| e.to_string())
}

/// 搜索剪贴板
#[tauri::command]
pub async fn search_clipboard(
    query: String,
    limit: Option<usize>,
    clipboard: State<'_, ClipboardManager>,
) -> Result<Vec<crate::clipboard::ClipboardItem>, String> {
    clipboard.search(&query, limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

/// 获取收藏的剪贴板项
#[tauri::command]
pub async fn get_clipboard_favorites(
    clipboard: State<'_, ClipboardManager>,
) -> Result<Vec<crate::clipboard::ClipboardItem>, String> {
    clipboard.get_favorites()
        .map_err(|e| e.to_string())
}

/// 复制到剪贴板
#[tauri::command]
pub async fn copy_to_clipboard(
    content: String,
    content_type: Option<String>,
    clipboard: State<'_, ClipboardManager>,
) -> Result<(), String> {
    clipboard.copy_to_clipboard(&content, &content_type.unwrap_or("text".to_string()))
        .map_err(|e| e.to_string())
}

/// 删除剪贴板项
#[tauri::command]
pub async fn delete_clipboard_item(
    id: String,
    clipboard: State<'_, ClipboardManager>,
) -> Result<(), String> {
    clipboard.delete_item(&id)
        .map_err(|e| e.to_string())
}

/// 切换收藏状态
#[tauri::command]
pub async fn toggle_clipboard_favorite(
    id: String,
    clipboard: State<'_, ClipboardManager>,
) -> Result<bool, String> {
    clipboard.toggle_favorite(&id)
        .map_err(|e| e.to_string())
}

/// 设置剪贴板项分类
#[tauri::command]
pub async fn set_clipboard_category(
    id: String,
    category: Option<String>,
    clipboard: State<'_, ClipboardManager>,
) -> Result<(), String> {
    clipboard.set_category(&id, category.as_deref())
        .map_err(|e| e.to_string())
}

/// 添加剪贴板项标签
#[tauri::command]
pub async fn add_clipboard_tag(
    id: String,
    tag: String,
    clipboard: State<'_, ClipboardManager>,
) -> Result<(), String> {
    clipboard.add_tag(&id, &tag)
        .map_err(|e| e.to_string())
}

/// 清空剪贴板历史
#[tauri::command]
pub async fn clear_clipboard_history(
    clipboard: State<'_, ClipboardManager>,
) -> Result<(), String> {
    clipboard.clear()
        .map_err(|e| e.to_string())
}

/// 获取剪贴板统计
#[tauri::command]
pub async fn get_clipboard_stats(
    clipboard: State<'_, ClipboardManager>,
) -> Result<(usize, usize, usize, usize), String> {
    clipboard.get_stats()
        .map_err(|e| e.to_string())
}

/// 启用开机自启
#[tauri::command]
pub async fn enable_autostart() -> Result<(), String> {
    crate::utils::autostart::enable()
        .map_err(|e| format!("Failed to enable autostart: {}", e))
}

/// 禁用开机自启
#[tauri::command]
pub async fn disable_autostart() -> Result<(), String> {
    crate::utils::autostart::disable()
        .map_err(|e| format!("Failed to disable autostart: {}", e))
}

/// 检查开机自启状态
#[tauri::command]
pub async fn is_autostart_enabled() -> Result<bool, String> {
    crate::utils::autostart::is_enabled()
        .map_err(|e| format!("Failed to check autostart status: {}", e))
}

/// 设置开机自启（根据布尔值启用或禁用）
#[tauri::command]
pub async fn set_autostart(enabled: bool) -> Result<(), String> {
    if enabled {
        enable_autostart().await
    } else {
        disable_autostart().await
    }
}

// ==================== 搜索历史管理 ====================

/// 获取搜索历史
#[tauri::command]
pub async fn get_search_history(
    history: State<'_, crate::search_history::SearchHistoryManager>,
) -> Result<Vec<crate::search_history::SearchHistoryItem>, String> {
    Ok(history.get_history().await)
}

/// 清空搜索历史
#[tauri::command]
pub async fn clear_search_history(
    history: State<'_, crate::search_history::SearchHistoryManager>,
) -> Result<(), String> {
    history.clear().await.map_err(|e| e.to_string())
}

/// 删除指定的搜索历史
#[tauri::command]
pub async fn remove_search_history(
    query: String,
    history: State<'_, crate::search_history::SearchHistoryManager>,
) -> Result<(), String> {
    history.remove(&query).await.map_err(|e| e.to_string())
}

/// 获取搜索建议（根据前缀匹配）
#[tauri::command]
pub async fn get_search_suggestions(
    prefix: String,
    limit: Option<usize>,
    history: State<'_, crate::search_history::SearchHistoryManager>,
) -> Result<Vec<crate::search_history::SearchHistoryItem>, String> {
    Ok(history.get_suggestions(&prefix, limit.unwrap_or(5)).await)
}

/// 记录搜索执行（当用户选择并执行某个结果时）
#[tauri::command]
pub async fn record_search_execution(
    query: String,
    history: State<'_, crate::search_history::SearchHistoryManager>,
) -> Result<(), String> {
    history.record_execution(&query).await.map_err(|e| e.to_string())
}

// ==================== 插件沙盒管理 ====================

/// 获取插件沙盒配置
#[tauri::command]
pub async fn get_sandbox_config(
    plugin_id: String,
    manager: State<'_, PluginManager>,
) -> Result<Option<crate::plugin::sandbox::SandboxConfig>, String> {
    Ok(manager.sandbox_manager().get_config(&plugin_id))
}

/// 更新插件沙盒配置
#[tauri::command]
pub async fn update_sandbox_config(
    config: crate::plugin::sandbox::SandboxConfig,
    manager: State<'_, PluginManager>,
) -> Result<(), String> {
    let plugin_id = config.plugin_id.clone();
    manager.sandbox_manager().update_config(config);
    tracing::info!("🔒 Updated sandbox config for plugin: {}", plugin_id);
    Ok(())
}

/// 获取插件权限列表
#[tauri::command]
pub async fn get_plugin_permissions(
    plugin_id: String,
    manager: State<'_, PluginManager>,
) -> Result<Vec<String>, String> {
    if let Some(config) = manager.sandbox_manager().get_config(&plugin_id) {
        let perms = config.effective_permissions();
        Ok(perms.iter().map(|p| format!("{:?}", p)).collect())
    } else {
        Ok(vec![])
    }
}

/// 检查插件权限
#[tauri::command]
pub async fn check_plugin_permission(
    _plugin_id: String,
    _permission: String,
    _manager: State<'_, PluginManager>,
) -> Result<bool, String> {
    // 这里需要解析 permission 字符串，简化处理
    // 实际应该实现完整的权限解析逻辑
    Ok(true) // 暂时返回 true，实际需要实现权限检查
}
