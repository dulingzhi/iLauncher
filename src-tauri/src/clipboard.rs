// 剪贴板监听模块（增强版：支持文本、图片、富文本，持久化存储）

use anyhow::Result;
use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::ImageEncoder;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::Emitter;
use uuid::Uuid;

use crate::storage::clipboard_db::{ClipboardDatabase, ClipboardRecord};
use crate::utils::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String, // text, image, rich_text
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>, // 图片文件路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

impl From<ClipboardRecord> for ClipboardItem {
    fn from(record: ClipboardRecord) -> Self {
        ClipboardItem {
            id: record.id.to_string(),
            item_type: record.content_type,
            content: record.content,
            preview: record.preview,
            timestamp: record.timestamp.timestamp() as u64,
            favorite: Some(record.favorite),
            file_path: record.file_path,
            category: record.category,
            tags: if record.tags.is_empty() { None } else { Some(record.tags) },
        }
    }
}

pub struct ClipboardManager {
    db: Arc<ClipboardDatabase>,
    image_dir: PathBuf, // 图片存储目录
    monitoring: Arc<RwLock<bool>>, // 监控状态
}

impl ClipboardManager {
    pub fn new() -> Result<Self> {
        let app_data_dir = paths::get_app_data_dir()?;
        let db_path = app_data_dir.join("clipboard.db");
        let image_dir = app_data_dir.join("clipboard_images");
        
        // 创建图片存储目录
        std::fs::create_dir_all(&image_dir)?;
        
        let db = ClipboardDatabase::new(db_path)?;
        
        Ok(Self {
            db: Arc::new(db),
            image_dir,
            monitoring: Arc::new(RwLock::new(false)),
        })
    }

    /// 启动剪贴板监控
    pub fn start_monitoring(&self, app_handle: tauri::AppHandle) {
        let db = self.db.clone();
        let image_dir = self.image_dir.clone();
        let monitoring = self.monitoring.clone();
        
        // 设置监控状态
        *monitoring.write() = true;
        
        thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(cb) => cb,
                Err(e) => {
                    tracing::error!("Failed to create clipboard: {}", e);
                    return;
                }
            };
            
            let mut last_text = String::new();
            let mut last_image_hash: Option<u64> = None;
            
            tracing::info!("Clipboard monitoring started (enhanced mode)");
            
            loop {
                thread::sleep(Duration::from_millis(500));
                
                // 检查是否应该停止监控
                if !*monitoring.read() {
                    tracing::info!("Clipboard monitoring stopped");
                    break;
                }
                
                // 检查文本剪贴板
                if let Ok(text) = clipboard.get_text() {
                    if text != last_text && !text.is_empty() && text.len() < 100_000 {
                        // 限制文本长度 100KB
                        let preview = if text.len() > 200 {
                            Some(format!("{}...", &text[..200]))
                        } else {
                            None
                        };
                        
                        match db.add_record("text", &text, None, preview.as_deref(), None) {
                            Ok(id) => {
                                tracing::debug!("Added text clipboard: id={}", id);
                                last_text = text.clone();
                                
                                // 发送更新事件
                                let _ = app_handle.emit("clipboard:updated", ());
                            }
                            Err(e) => {
                                if !e.to_string().contains("Duplicate") {
                                    tracing::error!("Failed to add text clipboard: {}", e);
                                }
                            }
                        }
                    }
                }
                
                // 检查图片剪贴板
                if let Ok(image) = clipboard.get_image() {
                    let image_hash = Self::hash_image(&image);
                    
                    if Some(image_hash) != last_image_hash {
                        match Self::save_image(&image, &image_dir) {
                            Ok((base64_data, file_path)) => {
                                let preview = Some(format!(
                                    "Image {}x{}", 
                                    image.width, 
                                    image.height
                                ));
                                
                                match db.add_record(
                                    "image",
                                    &base64_data,
                                    None,
                                    preview.as_deref(),
                                    Some(&file_path.to_string_lossy()),
                                ) {
                                    Ok(id) => {
                                        tracing::info!("Added image clipboard: id={}, size={}x{}", 
                                            id, image.width, image.height);
                                        last_image_hash = Some(image_hash);
                                        
                                        // 发送更新事件
                                        let _ = app_handle.emit("clipboard:updated", ());
                                    }
                                    Err(e) => {
                                        if !e.to_string().contains("Duplicate") {
                                            tracing::error!("Failed to add image clipboard: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to save image: {}", e);
                            }
                        }
                    }
                }
            }
        });
    }

    /// 停止监控
    pub fn stop_monitoring(&self) {
        *self.monitoring.write() = false;
    }

    /// 保存图片到文件并返回 base64 数据
    fn save_image(image: &ImageData, image_dir: &PathBuf) -> Result<(String, PathBuf)> {
        use image::{ImageBuffer, RgbaImage};
        
        // 将 RGBA 数据转换为图片
        let img: RgbaImage = ImageBuffer::from_raw(
            image.width as u32,
            image.height as u32,
            image.bytes.to_vec(),
        )
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
        
        // 生成唯一文件名
        let filename = format!("{}.png", Uuid::new_v4());
        let file_path = image_dir.join(&filename);
        
        // 保存为 PNG 文件
        img.save(&file_path)?;
        
        // 同时生成 base64 数据（用于数据库存储和快速显示）
        let mut png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder.write_image(
            &img,
            image.width as u32,
            image.height as u32,
            image::ColorType::Rgba8.into(),
        )?;
        
        let base64_data = format!("data:image/png;base64,{}", BASE64.encode(&png_data));
        
        Ok((base64_data, file_path))
    }

    /// 计算图片哈希（用于去重）
    fn hash_image(image: &ImageData) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        image.width.hash(&mut hasher);
        image.height.hash(&mut hasher);
        // 只哈希前 1000 字节数据避免太慢
        let sample_size = image.bytes.len().min(1000);
        image.bytes[..sample_size].hash(&mut hasher);
        hasher.finish()
    }

    /// 获取历史记录
    pub fn get_history(&self, limit: usize, offset: usize) -> Result<Vec<ClipboardItem>> {
        let records = self.db.get_history(limit, offset, None, false)?;
        Ok(records.into_iter().map(ClipboardItem::from).collect())
    }

    /// 搜索剪贴板
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ClipboardItem>> {
        let records = self.db.search(query, limit)?;
        Ok(records.into_iter().map(ClipboardItem::from).collect())
    }

    /// 获取收藏列表
    pub fn get_favorites(&self) -> Result<Vec<ClipboardItem>> {
        let records = self.db.get_history(100, 0, None, true)?;
        Ok(records.into_iter().map(ClipboardItem::from).collect())
    }

    /// 复制到剪贴板
    pub fn copy_to_clipboard(&self, content: &str, content_type: &str) -> Result<()> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
        
        match content_type {
            "text" | "rich_text" => {
                clipboard.set_text(content)
                    .map_err(|e| anyhow::anyhow!("Failed to set text: {}", e))?;
            }
            "image" => {
                // 从 base64 解码图片
                if let Some(base64_str) = content.strip_prefix("data:image/png;base64,") {
                    let img_data = BASE64.decode(base64_str)?;
                    let img = image::load_from_memory(&img_data)?;
                    let rgba = img.to_rgba8();
                    
                    let image_data = ImageData {
                        width: rgba.width() as usize,
                        height: rgba.height() as usize,
                        bytes: rgba.into_raw().into(),
                    };
                    
                    clipboard.set_image(image_data)
                        .map_err(|e| anyhow::anyhow!("Failed to set image: {}", e))?;
                }
            }
            _ => {}
        }
        
        Ok(())
    }

    /// 切换收藏状态
    pub fn toggle_favorite(&self, id: &str) -> Result<bool> {
        let id_num: i64 = id.parse()?;
        self.db.toggle_favorite(id_num)
    }

    /// 设置分类
    pub fn set_category(&self, id: &str, category: Option<&str>) -> Result<()> {
        let id_num: i64 = id.parse()?;
        self.db.set_category(id_num, category)
    }

    /// 添加标签
    pub fn add_tag(&self, id: &str, tag: &str) -> Result<()> {
        let id_num: i64 = id.parse()?;
        self.db.add_tag(id_num, tag)
    }

    /// 删除记录
    pub fn delete_item(&self, id: &str) -> Result<()> {
        let id_num: i64 = id.parse()?;
        
        // 如果是图片，同时删除文件
        let records = self.db.get_history(1000, 0, None, false)?;
        if let Some(record) = records.iter().find(|r| r.id == id_num) {
            if record.content_type == "image" {
                if let Some(file_path) = &record.file_path {
                    let _ = std::fs::remove_file(file_path);
                }
            }
        }
        
        self.db.delete_record(id_num)
    }

    /// 清空历史记录
    pub fn clear(&self) -> Result<()> {
        // 删除所有图片文件
        if self.image_dir.exists() {
            std::fs::remove_dir_all(&self.image_dir)?;
            std::fs::create_dir_all(&self.image_dir)?;
        }
        
        // 清空数据库（保留收藏）
        self.db.cleanup_old_records(0)?;
        Ok(())
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> Result<(usize, usize, usize, usize)> {
        self.db.get_stats()
    }
}
