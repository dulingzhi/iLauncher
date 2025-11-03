// Tauri Commands - 前端调用的 Rust 函数

use crate::core::types::*;
use tauri::State;

/// 查询命令
#[tauri::command]
pub async fn query(input: String) -> Result<Vec<QueryResult>, String> {
    // 暂时返回模拟数据
    Ok(vec![
        QueryResult::new(format!("Search for: {}", input))
            .with_subtitle("Press Enter to search")
            .with_icon(WoxImage::emoji("🔍"))
            .with_score(100)
            .with_action(Action::new("Search").default()),
        QueryResult::new("Calculator")
            .with_subtitle("Basic calculator")
            .with_icon(WoxImage::emoji("🔢"))
            .with_score(90)
            .with_action(Action::new("Calculate").default()),
    ])
}

/// 执行操作
#[tauri::command]
pub async fn execute_action(
    result_id: String,
    action_id: String,
) -> Result<(), String> {
    tracing::info!("Executing action {} for result {}", action_id, result_id);
    Ok(())
}

/// 获取插件列表
#[tauri::command]
pub async fn get_plugins() -> Result<Vec<PluginMetadata>, String> {
    Ok(vec![])
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
