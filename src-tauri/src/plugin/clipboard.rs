// 剪贴板历史插件

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Local};

#[derive(Debug, Clone)]
struct ClipboardItem {
    id: String,
    content: String,
    timestamp: DateTime<Local>,
    item_type: ClipboardType,
}

#[derive(Debug, Clone, PartialEq)]
enum ClipboardType {
    Text,
    Image,
    File,
}

pub struct ClipboardPlugin {
    metadata: PluginMetadata,
    history: Arc<RwLock<Vec<ClipboardItem>>>,
    matcher: SkimMatcherV2,
    max_history: usize,
}

impl ClipboardPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "clipboard".to_string(),
                name: "Clipboard History".to_string(),
                description: "Search and paste clipboard history".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("📋"),
                trigger_keywords: vec!["cb".to_string(), "clip".to_string()],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: PluginType::Native,
            },
            history: Arc::new(RwLock::new(Vec::new())),
            matcher: SkimMatcherV2::default(),
            max_history: 100,
        }
    }

    pub async fn init(&self) {
        tracing::info!("Clipboard plugin initialized (history tracking disabled for now)");
        // TODO: 实现剪贴板监听
        // 由于跨平台剪贴板监听比较复杂，这里先提供查询接口
    }

    /// 添加剪贴板项
    pub async fn add_item(&self, content: String, item_type: ClipboardType) {
        let mut history = self.history.write().await;
        
        // 检查是否重复
        if let Some(last) = history.first() {
            if last.content == content {
                return;
            }
        }
        
        let item = ClipboardItem {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            timestamp: Local::now(),
            item_type,
        };
        
        history.insert(0, item);
        
        // 限制历史记录数量
        if history.len() > self.max_history {
            history.truncate(self.max_history);
        }
    }

    /// 复制到剪贴板
    async fn copy_to_clipboard(content: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::DataExchange::{SetClipboardData, OpenClipboard, CloseClipboard, EmptyClipboard};
            use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
            use windows::Win32::Foundation::HWND;
            use windows::core::PCSTR;
            
            unsafe {
                OpenClipboard(HWND::default())?;
                EmptyClipboard()?;
                
                let text_bytes = content.as_bytes();
                let h_mem = GlobalAlloc(GMEM_MOVEABLE, text_bytes.len() + 1)?;
                let locked = GlobalLock(h_mem);
                
                if !locked.is_null() {
                    std::ptr::copy_nonoverlapping(
                        text_bytes.as_ptr(),
                        locked as *mut u8,
                        text_bytes.len(),
                    );
                    GlobalUnlock(h_mem)?;
                }
                
                CloseClipboard()?;
            }
        }
        
        tracing::info!("Copied to clipboard: {}", &content[..content.len().min(50)]);
        Ok(())
    }
    
    fn format_timestamp(dt: &DateTime<Local>) -> String {
        let now = Local::now();
        let diff = now.signed_duration_since(*dt);
        
        if diff.num_seconds() < 60 {
            "just now".to_string()
        } else if diff.num_minutes() < 60 {
            format!("{} min ago", diff.num_minutes())
        } else if diff.num_hours() < 24 {
            format!("{} hours ago", diff.num_hours())
        } else if diff.num_days() < 7 {
            format!("{} days ago", diff.num_days())
        } else {
            dt.format("%Y-%m-%d %H:%M").to_string()
        }
    }
    
    fn truncate_text(text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            text.to_string()
        } else {
            format!("{}...", &text[..max_len])
        }
    }
}

#[async_trait]
impl Plugin for ClipboardPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let search = ctx.search.trim();
        let history = self.history.read().await;
        
        // 如果没有历史记录
        if history.is_empty() {
            return Ok(vec![QueryResult {
                id: "empty".to_string(),
                title: "No clipboard history".to_string(),
                subtitle: "Start copying text to build your history".to_string(),
                icon: WoxImage::emoji("📋"),
                preview: None,
                score: 100,
                context_data: serde_json::Value::Null,
                group: None,
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![],
            }]);
        }
        
        let mut results = Vec::new();
        
        for item in history.iter() {
            // 如果有搜索词，使用模糊匹配
            let score = if search.is_empty() {
                100
            } else {
                match self.matcher.fuzzy_match(&item.content, search) {
                    Some(s) => s,
                    None => continue,
                }
            };
            
            let icon_emoji = match item.item_type {
                ClipboardType::Text => "📝",
                ClipboardType::Image => "🖼️",
                ClipboardType::File => "📎",
            };
            
            let preview = if item.content.len() > 100 {
                Some(Preview::Text(item.content.clone()))
            } else {
                None
            };
            
            results.push(QueryResult {
                id: item.id.clone(),
                title: Self::truncate_text(&item.content, 80),
                subtitle: Self::format_timestamp(&item.timestamp),
                icon: WoxImage::emoji(icon_emoji),
                preview,
                score: score as i32,
                context_data: serde_json::to_value(&item.content)?,
                group: Some("Clipboard".to_string()),
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![
                    Action {
                        id: "paste".to_string(),
                        name: "Copy to Clipboard".to_string(),
                        icon: None,
                        is_default: true,
                        prevent_hide: false,
                        hotkey: None,
                    },
                ],
            });
            
            // 限制结果数量
            if results.len() >= 20 {
                break;
            }
        }
        
        Ok(results)
    }
    
    async fn execute(&self, result_id: &str, _action_id: &str) -> Result<()> {
        let history = self.history.read().await;
        
        if let Some(item) = history.iter().find(|i| i.id == result_id) {
            Self::copy_to_clipboard(&item.content).await?;
        }
        
        Ok(())
    }
}
