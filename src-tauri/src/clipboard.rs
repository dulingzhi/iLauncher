use arboard::Clipboard;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
}

pub struct ClipboardManager {
    items: Arc<Mutex<Vec<ClipboardItem>>>,
    max_items: usize,
}

impl ClipboardManager {
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
            max_items: 100,
        }
    }

    pub fn start_monitoring(app_handle: tauri::AppHandle) {
        let manager = Self::new();
        let items = manager.items.clone();
        
        std::thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(cb) => cb,
                Err(e) => {
                    tracing::error!("Failed to create clipboard: {}", e);
                    return;
                }
            };
            
            let mut last_content = String::new();
            
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                
                if let Ok(text) = clipboard.get_text() {
                    if text != last_content && !text.is_empty() {
                        let item = ClipboardItem {
                            id: Uuid::new_v4().to_string(),
                            item_type: "text".to_string(),
                            content: text.clone(),
                            preview: None,
                            timestamp: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                            favorite: None,
                        };
                        
                        if let Ok(mut items) = items.lock() {
                            items.insert(0, item);
                            if items.len() > manager.max_items {
                                items.truncate(manager.max_items);
                            }
                        }
                        
                        last_content = text;
                    }
                }
            }
        });
    }

    pub fn get_history(&self) -> Vec<ClipboardItem> {
        self.items.lock().unwrap().clone()
    }

    pub fn add_item(&self, item: ClipboardItem) {
        let mut items = self.items.lock().unwrap();
        items.insert(0, item);
        if items.len() > self.max_items {
            items.truncate(self.max_items);
        }
    }

    pub fn delete_item(&self, id: &str) -> bool {
        let mut items = self.items.lock().unwrap();
        if let Some(pos) = items.iter().position(|item| item.id == id) {
            items.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn toggle_favorite(&self, id: &str) -> bool {
        let mut items = self.items.lock().unwrap();
        if let Some(item) = items.iter_mut().find(|item| item.id == id) {
            item.favorite = Some(!item.favorite.unwrap_or(false));
            true
        } else {
            false
        }
    }

    pub fn update_timestamp(&self, id: &str) -> bool {
        let mut items = self.items.lock().unwrap();
        if let Some(pos) = items.iter().position(|item| item.id == id) {
            let mut item = items.remove(pos);
            item.timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            items.insert(0, item);
            true
        } else {
            false
        }
    }

    pub fn clear(&self) {
        self.items.lock().unwrap().clear();
    }

    pub fn copy_to_clipboard(&self, content: &str) -> Result<(), String> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| format!("Failed to access clipboard: {}", e))?;
        clipboard.set_text(content)
            .map_err(|e| format!("Failed to set clipboard text: {}", e))?;
        Ok(())
    }
}
