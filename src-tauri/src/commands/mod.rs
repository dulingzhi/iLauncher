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
    let query_start = std::time::Instant::now();
    tracing::debug!("🔍 Query started: '{}'", input);
    
    // 记录查询
    if !input.is_empty() {
        let _ = stats.record_query(&input).await;
    }
    
    // 🔥 步骤 1: 获取 MRU 热门结果
    let mru_start = std::time::Instant::now();
    let mru_results = stats.get_top_results(20).await.unwrap_or_default();
    let mru_elapsed = mru_start.elapsed();
    
    // 🔥 步骤 2: 执行插件查询
    let plugin_query_start = std::time::Instant::now();
    let mut plugin_results = manager.query(&input).await.map_err(|e| e.to_string())?;
    let plugin_elapsed = plugin_query_start.elapsed();
    
    // 🔥 步骤 3: 注入 MRU 匹配项（直接创建结果，不依赖插件）
    let inject_start = std::time::Instant::now();
    let mut matched_mru = Vec::new();
    let input_lower = input.to_lowercase();
    
    tracing::debug!("📋 Checking {} MRU items against input: '{}'", mru_results.len(), input);
    
    for mru_item in mru_results {
        // 检查 MRU 项是否匹配当前搜索
        let title_lower = mru_item.title.to_lowercase();
        let id_lower = mru_item.result_id.to_lowercase();
        
        // 判断 MRU 项是否与当前搜索相关
        let is_match = title_lower.contains(&input_lower) || id_lower.contains(&input_lower);
        
        if !is_match {
            continue;
        }
        
        tracing::debug!("✅ MRU item matches search: '{}' (id: {}, plugin: {}, count: {})", 
            mru_item.title, mru_item.result_id, mru_item.plugin_id, mru_item.count);
        
        // 🔥 方案 A: 先尝试从插件结果中找到并提升
        let found_pos = plugin_results.iter().position(|r| {
            if r.plugin_id != mru_item.plugin_id {
                return false;
            }
            
            let r_id_normalized = r.id.to_lowercase().replace("/", "\\");
            let mru_id_normalized = mru_item.result_id.to_lowercase().replace("/", "\\");
            
            r_id_normalized == mru_id_normalized || 
            r.title.to_lowercase() == title_lower ||
            r_id_normalized.contains(&mru_id_normalized) ||
            mru_id_normalized.contains(&r_id_normalized)
        });
        
        if let Some(pos) = found_pos {
            // 插件返回了这个结果，提升分数
            let mut result = plugin_results.remove(pos);
            result.score = 1000 + mru_item.count * 10;
            tracing::info!("🎯 MRU boosted (from plugin): '{}' → score {}", result.title, result.score);
            matched_mru.push(result);
        } else {
            // 🔥 方案 B: 插件没有返回，直接注入 MRU 结果
            tracing::info!("💉 MRU injected (not in plugin results): '{}' (id: {})", 
                mru_item.title, mru_item.result_id);
            
            // 🔥 直接创建 QueryResult（复用 MRU 的元数据）
            let injected_result = stats.create_result_from_mru(&mru_item).await
                .map_err(|e| format!("Failed to create MRU result: {}", e))?;
            
            matched_mru.push(injected_result);
        }
    }
    let inject_elapsed = inject_start.elapsed();
    
    // 🔥 步骤 4: 为剩余插件结果调整分数
    let score_adjust_start = std::time::Instant::now();
    for result in &mut plugin_results {
        if let Ok(usage_count) = stats.get_result_score(&result.id, &result.plugin_id).await {
            // 给常用结果加分（每次使用加10分）
            result.score += usage_count * 10;
        }
    }
    let score_elapsed = score_adjust_start.elapsed();
    
    // 🔥 步骤 5: 合并结果（MRU 在前，其他在后）
    let sort_start = std::time::Instant::now();
    matched_mru.sort_by(|a, b| b.score.cmp(&a.score));
    plugin_results.sort_by(|a, b| b.score.cmp(&a.score));
    
    let mru_count = matched_mru.len();  // 先记录长度
    let mut final_results = matched_mru;
    final_results.extend(plugin_results);
    let sort_elapsed = sort_start.elapsed();
    
    let total_elapsed = query_start.elapsed();
    tracing::info!(
        "✅ Query completed: '{}' → {} results ({} MRU) in {:.2}ms (mru: {:.2}ms, plugin: {:.2}ms, inject: {:.2}ms, score: {:.2}ms, sort: {:.2}ms)",
        input,
        final_results.len(),
        mru_count,
        total_elapsed.as_secs_f64() * 1000.0,
        mru_elapsed.as_secs_f64() * 1000.0,
        plugin_elapsed.as_secs_f64() * 1000.0,
        inject_elapsed.as_secs_f64() * 1000.0,
        score_elapsed.as_secs_f64() * 1000.0,
        sort_elapsed.as_secs_f64() * 1000.0
    );
    
    Ok(final_results)
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
