// 持久化存储模块

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub appearance: AppearanceConfig,
    pub plugins: PluginsConfig,
    pub advanced: AdvancedConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub hotkey: String,
    pub search_delay: u64,
    pub max_results: usize,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    pub theme: String,
    pub window_width: u32,
    pub window_height: u32,
    pub font_size: u32,
    pub transparency: u8,
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
            },
            appearance: AppearanceConfig {
                theme: "dark".to_string(),
                window_width: 800,
                window_height: 500,
                font_size: 14,
                transparency: 95,
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
        }
    }
}

/// 存储管理器
pub struct StorageManager {
    config_path: PathBuf,
    cache_dir: PathBuf,
    data_dir: PathBuf,
}

impl StorageManager {
    pub fn new() -> Result<Self> {
        // 获取应用数据目录
        let app_data_dir = if let Some(data_dir) = directories::ProjectDirs::from("com", "iLauncher", "iLauncher") {
            data_dir.data_dir().to_path_buf()
        } else {
            PathBuf::from(".")
        };

        // 创建必要的目录
        let config_dir = app_data_dir.join("config");
        let cache_dir = app_data_dir.join("cache");
        let data_dir = app_data_dir.join("data");

        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&cache_dir)?;
        std::fs::create_dir_all(&data_dir)?;

        Ok(Self {
            config_path: config_dir.join("config.json"),
            cache_dir,
            data_dir,
        })
    }

    /// 加载配置
    pub async fn load_config(&self) -> Result<AppConfig> {
        if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path).await?;
            let config: AppConfig = serde_json::from_str(&content)?;
            tracing::info!("Loaded config from {:?}", self.config_path);
            Ok(config)
        } else {
            tracing::info!("No config file found, using defaults");
            Ok(AppConfig::default())
        }
    }

    /// 保存配置
    pub async fn save_config(&self, config: &AppConfig) -> Result<()> {
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&self.config_path, content).await?;
        tracing::info!("Saved config to {:?}", self.config_path);
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
}
