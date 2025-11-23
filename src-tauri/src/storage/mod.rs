// 持久化存储模块

pub mod clipboard_db;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub appearance: AppearanceConfig,
    pub plugins: PluginsConfig,
    pub advanced: AdvancedConfig,
    #[serde(default)]
    pub ui: UIConfig,
    #[serde(default)]
    pub font: FontConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub hotkey: String,
    pub search_delay: u64,
    pub max_results: usize,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_true")]
    pub clear_on_hide: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    pub theme: String,
    #[serde(default = "default_zh_language")]
    pub language: String,
    pub window_width: u32,
    pub window_height: u32,
    pub font_size: u32,
    pub transparency: u8,
    #[serde(default = "default_true")]
    pub show_preview: bool,
}

// UI外观微调配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    #[serde(default = "default_opacity")]
    pub opacity: u8,              // 窗口透明度 60-100
    #[serde(default = "default_blur")]
    pub blur: u8,                 // 背景模糊 0-50
    #[serde(default = "default_border_radius")]
    pub border_radius: u8,        // 圆角大小 0-30
    #[serde(default = "default_shadow_size")]
    pub shadow_size: u8,          // 阴影大小 0-50
    #[serde(default = "default_result_height")]
    pub result_height: u8,        // 结果项高度 40-80
    #[serde(default = "default_max_results")]
    pub max_results: u8,          // 最大结果数 5-20
    #[serde(default = "default_animation_speed")]
    pub animation_speed: u16,     // 动画速度 0-500ms
    #[serde(default = "default_icon_size")]
    pub icon_size: u8,            // 图标大小 16-48
}

// 字体配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: u8,            // 12-24
    #[serde(default = "default_line_height")]
    pub line_height: f32,         // 1.0-2.0
    #[serde(default)]
    pub letter_spacing: f32,      // -0.05 - 0.2
    #[serde(default = "default_font_weight")]
    pub font_weight: u16,         // 300-700
    #[serde(default = "default_title_size")]
    pub title_size: u8,           // 12-20
    #[serde(default = "default_subtitle_size")]
    pub subtitle_size: u8,        // 10-16
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            opacity: 95,
            blur: 10,
            border_radius: 12,
            shadow_size: 20,
            result_height: 60,
            max_results: 8,
            animation_speed: 200,
            icon_size: 32,
        }
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            font_family: "system-ui, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif".to_string(),
            font_size: 14,
            line_height: 1.5,
            letter_spacing: 0.0,
            font_weight: 400,
            title_size: 14,
            subtitle_size: 12,
        }
    }
}

fn default_opacity() -> u8 { 95 }
fn default_blur() -> u8 { 10 }
fn default_border_radius() -> u8 { 12 }
fn default_shadow_size() -> u8 { 20 }
fn default_result_height() -> u8 { 60 }
fn default_max_results() -> u8 { 8 }
fn default_animation_speed() -> u16 { 200 }
fn default_icon_size() -> u8 { 32 }
fn default_font_family() -> String {
    "system-ui, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif".to_string()
}
fn default_font_size() -> u8 { 14 }
fn default_line_height() -> f32 { 1.5 }
fn default_font_weight() -> u16 { 400 }
fn default_title_size() -> u8 { 14 }
fn default_subtitle_size() -> u8 { 12 }

fn default_language() -> String {
    "en".to_string()
}

fn default_zh_language() -> String {
    "zh-CN".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    pub enabled_plugins: Vec<String>,
    pub disabled_plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    pub start_on_boot: bool,
    pub show_tray_icon: bool,
    pub enable_analytics: bool,
    pub cache_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                hotkey: "Alt+Space".to_string(),
                search_delay: 100,
                max_results: 10,
                language: "en".to_string(),
                clear_on_hide: true,
            },
            appearance: AppearanceConfig {
                theme: "dark".to_string(),
                language: "zh".to_string(),
                window_width: 800,
                window_height: 600,
                font_size: 14,
                transparency: 95,
                show_preview: true,
            },
            plugins: PluginsConfig {
                enabled_plugins: vec![
                    "calculator".to_string(),
                    "app_search".to_string(),
                    "file_search".to_string(),
                    "web_search".to_string(),
                    "clipboard".to_string(),
                    "unit_converter".to_string(),
                ],
                disabled_plugins: vec![],
            },
            advanced: AdvancedConfig {
                start_on_boot: false,
                show_tray_icon: true,
                enable_analytics: false,
                cache_enabled: true,
            },
            ui: UIConfig::default(),
            font: FontConfig::default(),
        }
    }
}

/// 存储管理器
pub struct StorageManager {
    config_path: PathBuf,
    cache_dir: PathBuf,
    data_dir: PathBuf,
    // 配置缓存，避免重复读取文件
    config_cache: Arc<RwLock<Option<AppConfig>>>,
}

impl StorageManager {
    pub fn new() -> Result<Self> {
        // 使用统一的 AppData\Local\iLauncher 目录
        use crate::utils::paths;
        
        let app_data_dir = paths::get_app_data_dir()?;

        // 创建必要的目录
        let config_dir = app_data_dir.join("config");
        let cache_dir = app_data_dir.join("cache");
        let data_dir = app_data_dir.join("data");

        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&cache_dir)?;
        std::fs::create_dir_all(&data_dir)?;

        let config_path = config_dir.join("config.json");

        Ok(Self {
            config_path,
            cache_dir,
            data_dir,
            config_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// 加载配置
    pub async fn load_config(&self) -> Result<AppConfig> {
        // 先检查缓存
        {
            let cache = self.config_cache.read().await;
            if let Some(config) = cache.as_ref() {
                return Ok(config.clone());
            }
        }
        
        // 从文件加载
        let config = if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path).await?;
            let config: AppConfig = serde_json::from_str(&content)?;
            tracing::info!("Loaded config from {:?}", self.config_path);
            config
        } else {
            tracing::info!("No config file found, using defaults");
            AppConfig::default()
        };
        
        // 更新缓存
        {
            let mut cache = self.config_cache.write().await;
            *cache = Some(config.clone());
        }
        
        Ok(config)
    }

    /// 保存配置
    pub async fn save_config(&self, config: &AppConfig) -> Result<()> {
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&self.config_path, content).await?;
        tracing::info!("Saved config to {:?}", self.config_path);
        
        // 更新缓存
        {
            let mut cache = self.config_cache.write().await;
            *cache = Some(config.clone());
        }
        
        Ok(())
    }

    /// 保存缓存数据
    pub async fn save_cache(&self, key: &str, data: &[u8]) -> Result<()> {
        let cache_file = self.cache_dir.join(format!("{}.cache", key));
        fs::write(&cache_file, data).await?;
        Ok(())
    }

    /// 加载缓存数据
    pub async fn load_cache(&self, key: &str) -> Result<Vec<u8>> {
        let cache_file = self.cache_dir.join(format!("{}.cache", key));
        if cache_file.exists() {
            Ok(fs::read(&cache_file).await?)
        } else {
            Ok(Vec::new())
        }
    }

    /// 保存数据文件
    pub async fn save_data(&self, filename: &str, data: &str) -> Result<()> {
        let data_file = self.data_dir.join(filename);
        fs::write(&data_file, data).await?;
        Ok(())
    }

    /// 加载数据文件
    pub async fn load_data(&self, filename: &str) -> Result<String> {
        let data_file = self.data_dir.join(filename);
        if data_file.exists() {
            Ok(fs::read_to_string(&data_file).await?)
        } else {
            Ok(String::new())
        }
    }

    /// 清除所有缓存
    pub async fn clear_cache(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).await?;
            fs::create_dir_all(&self.cache_dir).await?;
            tracing::info!("Cleared all cache");
        }
        Ok(())
    }

    /// 获取数据目录路径
    pub fn get_data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// 获取缓存目录路径
    pub fn get_cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// 获取插件配置目录
    fn get_plugin_config_dir(&self) -> PathBuf {
        self.data_dir.join("plugins_config")
    }

    /// 获取插件配置
    pub async fn get_plugin_config(&self, plugin_id: &str) -> Result<serde_json::Value> {
        let config_dir = self.get_plugin_config_dir();
        let config_file = config_dir.join(format!("{}.json", plugin_id));
        
        if config_file.exists() {
            let content = fs::read_to_string(&config_file).await?;
            let config: serde_json::Value = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            // 返回空对象
            Ok(serde_json::json!({}))
        }
    }

    /// 保存插件配置
    pub async fn save_plugin_config(&self, plugin_id: &str, config: serde_json::Value) -> Result<()> {
        let config_dir = self.get_plugin_config_dir();
        std::fs::create_dir_all(&config_dir)?;
        
        let config_file = config_dir.join(format!("{}.json", plugin_id));
        let content = serde_json::to_string_pretty(&config)?;
        fs::write(&config_file, content).await?;
        
        tracing::info!("Saved config for plugin: {}", plugin_id);
        Ok(())
    }
}

// ==================== 公共函数 ====================

/// 获取插件安装目录
pub fn get_plugins_dir() -> Result<PathBuf> {
    use crate::utils::paths;
    let app_data_dir = paths::get_app_data_dir()?;
    let plugins_dir = app_data_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir)?;
    Ok(plugins_dir)
}

/// 获取缓存目录
pub fn get_cache_dir() -> Result<PathBuf> {
    use crate::utils::paths;
    let app_data_dir = paths::get_app_data_dir()?;
    let cache_dir = app_data_dir.join("cache");
    std::fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir)
}
