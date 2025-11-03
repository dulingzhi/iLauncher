// Tauri Commands - 前端调用的 Rust 函数

use crate::core::types::*;
use crate::plugin::PluginManager;
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
