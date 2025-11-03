// Settings 插件 - 快速打开设置界面

use super::Plugin;
use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;

pub struct SettingsPlugin {
    metadata: PluginMetadata,
}

impl SettingsPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "ilauncher.plugin.settings".to_string(),
                name: "Settings".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                description: "Open settings interface".to_string(),
                icon: WoxImage::Emoji("⚙️".to_string()),
                trigger_keywords: vec![
                    "settings".to_string(),
                    "config".to_string(),
                    "preferences".to_string(),
                    "设置".to_string(),
                    "配置".to_string(),
                ],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["Windows".to_string(), "macOS".to_string(), "Linux".to_string()],
                plugin_type: PluginType::Native,
            },
        }
    }
}

#[async_trait]
impl Plugin for SettingsPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.to_lowercase();
        
        // 匹配关键词
        let keywords = ["settings", "setting", "config", "preferences", "设置", "配置"];
        let matched = keywords.iter().any(|kw| kw.contains(&query) || query.contains(kw));
        
        if !matched && query.len() < 2 {
            return Ok(vec![]);
        }
        
        let score = if query.is_empty() {
            0
        } else if keywords.iter().any(|kw| kw.starts_with(&query)) {
            100
        } else if matched {
            80
        } else {
            0
        };
        
        if score == 0 {
            return Ok(vec![]);
        }
        
        Ok(vec![QueryResult {
            id: "settings".to_string(),
            title: "Settings".to_string(),
            subtitle: "Open iLauncher settings".to_string(),
            icon: WoxImage::Emoji("⚙️".to_string()),
            score,
            plugin_id: self.metadata.id.clone(),
            context_data: serde_json::Value::String("settings".to_string()),
            actions: vec![Action {
                id: "open".to_string(),
                name: "Open Settings".to_string(),
                icon: None,
                is_default: true,
                hotkey: None,
                prevent_hide: true,
            }],
            preview: None,
            refreshable: false,
            group: None,
        }])
    }

    async fn execute(&self, result_id: &str, _action_id: &str) -> Result<()> {
        if result_id != "settings" {
            return Err(anyhow::anyhow!("Unknown result_id"));
        }
        
        // 通过 emit 事件通知前端打开设置界面
        // 前端需要监听 'open-settings' 事件
        Ok(())
    }
}

/// Plugin Manager 插件 - 快速打开插件管理器
pub struct PluginManagerPlugin {
    metadata: PluginMetadata,
}

impl PluginManagerPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "ilauncher.plugin.plugin_manager".to_string(),
                name: "Plugin Manager".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                description: "Open plugin manager interface".to_string(),
                icon: WoxImage::Emoji("🧩".to_string()),
                trigger_keywords: vec![
                    "plugins".to_string(),
                    "plugin".to_string(),
                    "extensions".to_string(),
                    "插件".to_string(),
                    "扩展".to_string(),
                ],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["Windows".to_string(), "macOS".to_string(), "Linux".to_string()],
                plugin_type: PluginType::Native,
            },
        }
    }
}

#[async_trait]
impl Plugin for PluginManagerPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.to_lowercase();
        
        // 匹配关键词
        let keywords = ["plugins", "plugin", "extensions", "插件", "扩展"];
        let matched = keywords.iter().any(|kw| kw.contains(&query) || query.contains(kw));
        
        if !matched && query.len() < 2 {
            return Ok(vec![]);
        }
        
        let score = if query.is_empty() {
            0
        } else if keywords.iter().any(|kw| kw.starts_with(&query)) {
            100
        } else if matched {
            80
        } else {
            0
        };
        
        if score == 0 {
            return Ok(vec![]);
        }
        
        Ok(vec![QueryResult {
            id: "plugin_manager".to_string(),
            title: "Plugin Manager".to_string(),
            subtitle: "Manage iLauncher plugins".to_string(),
            icon: WoxImage::Emoji("🧩".to_string()),
            score,
            plugin_id: self.metadata.id.clone(),
            context_data: serde_json::Value::String("plugin_manager".to_string()),
            actions: vec![Action {
                id: "open".to_string(),
                name: "Open Plugin Manager".to_string(),
                icon: None,
                is_default: true,
                hotkey: None,
                prevent_hide: true,
            }],
            preview: None,
            refreshable: false,
            group: None,
        }])
    }

    async fn execute(&self, result_id: &str, _action_id: &str) -> Result<()> {
        if result_id != "plugin_manager" {
            return Err(anyhow::anyhow!("Unknown result_id"));
        }
        
        // 通过 emit 事件通知前端打开插件管理器
        // 前端需要监听 'open-plugin-manager' 事件
        Ok(())
    }
}
