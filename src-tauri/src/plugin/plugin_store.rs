// 插件商店 API 客户端
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// 插件商店配置
pub struct PluginStoreConfig {
    pub base_url: String,
}

impl Default for PluginStoreConfig {
    fn default() -> Self {
        Self {
            base_url: "https://plugins.ilauncher.com/api".to_string(),
        }
    }
}

/// 插件列表项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginListItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub rating: f32,
    pub icon_url: String,
    pub download_url: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
}

/// 插件详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDetails {
    pub id: String,
    pub manifest: serde_json::Value,
    pub readme: String,
    pub versions: Vec<String>,
    pub statistics: PluginStatistics,
    #[serde(default)]
    pub reviews: Vec<PluginReview>,
}

/// 插件统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStatistics {
    pub downloads: u64,
    pub rating: f32,
    pub reviews: u64,
    pub stars: u64,
}

/// 插件评论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginReview {
    pub user: String,
    pub rating: u8,
    pub comment: String,
    pub created_at: String,
}

/// 插件搜索参数
#[derive(Debug, Clone, Default)]
pub struct SearchParams {
    pub query: Option<String>,
    pub category: Option<String>,
    pub sort: Option<String>, // "downloads", "rating", "date"
    pub page: u32,
    pub per_page: u32,
}

/// 插件搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub plugins: Vec<PluginListItem>,
}

/// 插件商店客户端
pub struct PluginStore {
    config: PluginStoreConfig,
    client: Client,
    cache_dir: PathBuf,
}

impl PluginStore {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            config: PluginStoreConfig::default(),
            client: Client::new(),
            cache_dir,
        }
    }
    
    pub fn with_config(config: PluginStoreConfig, cache_dir: PathBuf) -> Self {
        Self {
            config,
            client: Client::new(),
            cache_dir,
        }
    }
    
    /// 搜索插件
    pub async fn search(&self, params: SearchParams) -> Result<SearchResult> {
        let mut url = format!("{}/plugins", self.config.base_url);
        let mut query_params = Vec::new();
        
        if let Some(q) = &params.query {
            query_params.push(format!("q={}", urlencoding::encode(q)));
        }
        if let Some(cat) = &params.category {
            query_params.push(format!("category={}", urlencoding::encode(cat)));
        }
        if let Some(sort) = &params.sort {
            query_params.push(format!("sort={}", sort));
        }
        query_params.push(format!("page={}", params.page));
        query_params.push(format!("per_page={}", params.per_page));
        
        if !query_params.is_empty() {
            url.push('?');
            url.push_str(&query_params.join("&"));
        }
        
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to search plugins: {}", response.status()));
        }
        
        let result: SearchResult = response.json().await?;
        Ok(result)
    }
    
    /// 获取插件详情
    pub async fn get_plugin_details(&self, plugin_id: &str) -> Result<PluginDetails> {
        let url = format!("{}/plugins/{}", self.config.base_url, plugin_id);
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get plugin details: {}", response.status()));
        }
        
        let details: PluginDetails = response.json().await?;
        Ok(details)
    }
    
    /// 下载插件
    pub async fn download_plugin(&self, plugin_id: &str, version: Option<&str>) -> Result<PathBuf> {
        // 1. 构建下载 URL
        let mut url = format!("{}/plugins/{}/download", self.config.base_url, plugin_id);
        if let Some(v) = version {
            url.push_str(&format!("?version={}", v));
        }
        
        // 2. 发起下载请求
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to download plugin: {}", response.status()));
        }
        
        // 3. 获取文件名
        let filename = Self::extract_filename_from_response(&response, plugin_id)?;
        
        // 4. 保存到缓存目录
        fs::create_dir_all(&self.cache_dir).await?;
        let file_path = self.cache_dir.join(&filename);
        
        // 5. 写入文件
        let bytes = response.bytes().await?;
        fs::write(&file_path, &bytes).await?;
        
        Ok(file_path)
    }
    
    /// 从响应头提取文件名
    fn extract_filename_from_response(response: &reqwest::Response, plugin_id: &str) -> Result<String> {
        if let Some(content_disposition) = response.headers().get("content-disposition") {
            let cd = content_disposition.to_str()?;
            if let Some(filename) = Self::parse_filename_from_content_disposition(cd) {
                return Ok(filename);
            }
        }
        
        // 降级方案: 使用 plugin_id + .ilp
        Ok(format!("{}.ilp", plugin_id))
    }
    
    /// 解析 Content-Disposition 头获取文件名
    fn parse_filename_from_content_disposition(cd: &str) -> Option<String> {
        // Content-Disposition: attachment; filename="plugin.ilp"
        for part in cd.split(';') {
            let trimmed = part.trim();
            if trimmed.starts_with("filename=") {
                let filename = trimmed[9..].trim_matches('"');
                return Some(filename.to_string());
            }
        }
        None
    }
    
    /// 获取热门插件
    pub async fn get_popular_plugins(&self, limit: u32) -> Result<Vec<PluginListItem>> {
        let params = SearchParams {
            sort: Some("downloads".to_string()),
            page: 1,
            per_page: limit,
            ..Default::default()
        };
        
        let result = self.search(params).await?;
        Ok(result.plugins)
    }
    
    /// 获取最新插件
    pub async fn get_recent_plugins(&self, limit: u32) -> Result<Vec<PluginListItem>> {
        let params = SearchParams {
            sort: Some("date".to_string()),
            page: 1,
            per_page: limit,
            ..Default::default()
        };
        
        let result = self.search(params).await?;
        Ok(result.plugins)
    }
    
    /// 按分类获取插件
    pub async fn get_plugins_by_category(&self, category: &str, page: u32) -> Result<SearchResult> {
        let params = SearchParams {
            category: Some(category.to_string()),
            page,
            per_page: 20,
            ..Default::default()
        };
        
        self.search(params).await
    }
    
    /// 检查插件更新
    pub async fn check_updates(&self, installed_plugins: Vec<(String, String)>) -> Result<Vec<(String, String)>> {
        let mut updates = Vec::new();
        
        for (plugin_id, current_version) in installed_plugins {
            if let Ok(details) = self.get_plugin_details(&plugin_id).await {
                if let Some(latest_version) = details.versions.first() {
                    if latest_version != &current_version {
                        updates.push((plugin_id, latest_version.clone()));
                    }
                }
            }
        }
        
        Ok(updates)
    }
    
    /// 清理缓存
    pub async fn clear_cache(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).await?;
            fs::create_dir_all(&self.cache_dir).await?;
        }
        Ok(())
    }
}

/// 模拟插件商店（用于开发测试）
#[cfg(debug_assertions)]
pub struct MockPluginStore;

#[cfg(debug_assertions)]
impl MockPluginStore {
    pub fn get_mock_plugins() -> Vec<PluginListItem> {
        vec![
            PluginListItem {
                id: "com.example.weather".to_string(),
                name: "Weather".to_string(),
                version: "1.0.0".to_string(),
                description: "查询天气预报".to_string(),
                author: "Example Corp".to_string(),
                downloads: 1000,
                rating: 4.5,
                icon_url: "https://example.com/icon.png".to_string(),
                download_url: "https://example.com/weather.ilp".to_string(),
                keywords: vec!["weather".to_string(), "forecast".to_string()],
                screenshots: vec![],
            },
            PluginListItem {
                id: "com.example.currency".to_string(),
                name: "Currency Converter".to_string(),
                version: "1.2.0".to_string(),
                description: "实时货币转换".to_string(),
                author: "Finance Team".to_string(),
                downloads: 500,
                rating: 4.2,
                icon_url: "https://example.com/currency-icon.png".to_string(),
                download_url: "https://example.com/currency.ilp".to_string(),
                keywords: vec!["currency".to_string(), "money".to_string()],
                screenshots: vec![],
            },
            PluginListItem {
                id: "com.example.screenshot".to_string(),
                name: "Screenshot OCR".to_string(),
                version: "2.0.0".to_string(),
                description: "截图并识别文字".to_string(),
                author: "OCR Labs".to_string(),
                downloads: 2000,
                rating: 4.8,
                icon_url: "https://example.com/ocr-icon.png".to_string(),
                download_url: "https://example.com/screenshot.ilp".to_string(),
                keywords: vec!["screenshot".to_string(), "ocr".to_string()],
                screenshots: vec![],
            },
        ]
    }
    
    pub fn get_mock_details(plugin_id: &str) -> Option<PluginDetails> {
        let plugins = Self::get_mock_plugins();
        let plugin = plugins.iter().find(|p| p.id == plugin_id)?;
        
        Some(PluginDetails {
            id: plugin.id.clone(),
            manifest: serde_json::json!({
                "id": plugin.id,
                "name": plugin.name,
                "version": plugin.version,
            }),
            readme: format!("# {}\n\n{}\n\n## Installation\n\nInstall from plugin marketplace.", plugin.name, plugin.description),
            versions: vec![plugin.version.clone(), "0.9.0".to_string()],
            statistics: PluginStatistics {
                downloads: plugin.downloads,
                rating: plugin.rating,
                reviews: 10,
                stars: (plugin.downloads / 10) as u64,
            },
            reviews: vec![
                PluginReview {
                    user: "User1".to_string(),
                    rating: 5,
                    comment: "Great plugin!".to_string(),
                    created_at: "2024-01-01".to_string(),
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_filename_from_content_disposition() {
        let cd = r#"attachment; filename="my-plugin.ilp""#;
        let filename = PluginStore::parse_filename_from_content_disposition(cd);
        assert_eq!(filename, Some("my-plugin.ilp".to_string()));
    }
    
    #[cfg(debug_assertions)]
    #[test]
    fn test_mock_plugins() {
        let plugins = MockPluginStore::get_mock_plugins();
        assert_eq!(plugins.len(), 3);
        assert_eq!(plugins[0].id, "com.example.weather");
    }
}
