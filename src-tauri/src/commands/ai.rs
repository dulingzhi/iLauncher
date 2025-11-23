// AI 助手相关命令

use crate::plugin::ai_assistant::{AIAssistantPlugin, AIConfig, Conversation};
use crate::plugin::PluginManager;
use tauri::State;

/// 加载 AI 配置
#[tauri::command]
pub async fn get_ai_config(
    manager: State<'_, PluginManager>,
) -> Result<AIConfig, String> {
    if let Some(ai_plugin) = manager.get_ai_plugin() {
        Ok(ai_plugin.get_config().await)
    } else {
        Err("AI plugin not found".to_string())
    }
}

/// 保存 AI 配置
#[tauri::command]
pub async fn save_ai_config(
    config: AIConfig,
    manager: State<'_, PluginManager>,
) -> Result<(), String> {
    if let Some(ai_plugin) = manager.get_ai_plugin() {
        ai_plugin.load_config(config).await;
        Ok(())
    } else {
        Err("AI plugin not found".to_string())
    }
}

/// 发送消息到 AI
#[tauri::command]
pub async fn send_ai_message(
    message: String,
    manager: State<'_, PluginManager>,
) -> Result<String, String> {
    if let Some(ai_plugin) = manager.get_ai_plugin() {
        ai_plugin.send_message(message).await
            .map_err(|e| e.to_string())
    } else {
        Err("AI plugin not found".to_string())
    }
}

/// 创建新对话
#[tauri::command]
pub async fn create_ai_conversation(
    title: String,
    manager: State<'_, PluginManager>,
) -> Result<String, String> {
    if let Some(ai_plugin) = manager.get_ai_plugin() {
        Ok(ai_plugin.create_conversation(title).await)
    } else {
        Err("AI plugin not found".to_string())
    }
}

/// 获取对话列表
#[tauri::command]
pub async fn get_ai_conversations(
    manager: State<'_, PluginManager>,
) -> Result<Vec<Conversation>, String> {
    if let Some(ai_plugin) = manager.get_ai_plugin() {
        Ok(ai_plugin.get_conversations().await)
    } else {
        Err("AI plugin not found".to_string())
    }
}

/// 切换对话
#[tauri::command]
pub async fn switch_ai_conversation(
    conv_id: String,
    manager: State<'_, PluginManager>,
) -> Result<(), String> {
    if let Some(ai_plugin) = manager.get_ai_plugin() {
        ai_plugin.switch_conversation(conv_id).await;
        Ok(())
    } else {
        Err("AI plugin not found".to_string())
    }
}

/// 删除对话
#[tauri::command]
pub async fn delete_ai_conversation(
    conv_id: String,
    manager: State<'_, PluginManager>,
) -> Result<(), String> {
    if let Some(ai_plugin) = manager.get_ai_plugin() {
        ai_plugin.delete_conversation(conv_id).await;
        Ok(())
    } else {
        Err("AI plugin not found".to_string())
    }
}
