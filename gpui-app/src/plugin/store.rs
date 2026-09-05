//! 插件商店客户端（对齐 旧版对应实现）。
//! HTTP 走 gpui-kit ReqwestClient（与 updater.rs 同模式：调用方传 &dyn HttpClient），
//! URL 构建与响应解析为纯函数，可无网络单测。
//!
//! 相对 旧版的偏离：
//!   - 只迁移窗口 UI 有消费方的方法：search / popular / download（其余
//!     get_plugin_details / get_recent_plugins / get_plugins_by_category /
//!     check_updates / clear_cache 随更富的市场 UI 再补，不搬死代码）
//!   - URL 编码用 crate 内 encode_query（web_search 同款；不引入 urlencoding 依赖）

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use futures::AsyncReadExt;
use gpui_kit::http_client::{AsyncBody, HttpClient};
use serde::{Deserialize, Serialize};

use crate::plugin::web_search::encode_query;

/// 商店 API 基址（对齐 Tauri PluginStoreConfig 默认值）
pub const DEFAULT_BASE_URL: &str = "https://plugins.ilauncher.com/api";

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

/// 插件商店客户端（纯配置；HttpClient 按调用传入，便于测试替换）
#[derive(Debug, Clone)]
pub struct PluginStore {
    base_url: String,
    cache_dir: PathBuf,
}

impl PluginStore {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { base_url: DEFAULT_BASE_URL.to_string(), cache_dir }
    }

    /// 测试/自建源用（指向 mock 服务）；仅测试使用，生产固定 DEFAULT_BASE_URL
    #[cfg(test)]
    pub fn with_base_url(base_url: impl Into<String>, cache_dir: PathBuf) -> Self {
        Self { base_url: base_url.into(), cache_dir }
    }

    /// 搜索插件
    pub async fn search(
        &self,
        client: &dyn HttpClient,
        params: SearchParams,
    ) -> Result<SearchResult> {
        let url = build_search_url(&self.base_url, &params);
        let body = get_bytes(client, &url).await?;
        serde_json::from_slice(&body).map_err(|e| anyhow!("商店搜索响应解析失败: {e}"))
    }

    /// 热门插件（市场窗口初始列表）
    pub async fn popular(&self, client: &dyn HttpClient, limit: u32) -> Result<Vec<PluginListItem>> {
        let result = self
            .search(
                client,
                SearchParams { sort: Some("downloads".into()), page: 1, per_page: limit, ..Default::default() },
            )
            .await?;
        Ok(result.plugins)
    }

    /// 下载插件包到缓存目录，返回本地 .ilp 路径
    pub async fn download(
        &self,
        client: &dyn HttpClient,
        plugin_id: &str,
        version: Option<&str>,
    ) -> Result<PathBuf> {
        let mut url = format!("{}/plugins/{}/download", self.base_url, plugin_id);
        if let Some(v) = version {
            url.push_str(&format!("?version={}", encode_query(v)));
        }
        let mut resp = client.get(&url, AsyncBody::empty(), true).await?;
        if !resp.status().is_success() {
            return Err(anyhow!("插件下载返回 HTTP {}", resp.status().as_u16()));
        }
        let filename = extract_filename(resp.headers().get("content-disposition"), plugin_id);
        let mut bytes = Vec::new();
        resp.body_mut().read_to_end(&mut bytes).await?;
        std::fs::create_dir_all(&self.cache_dir)?;
        let path = self.cache_dir.join(filename);
        std::fs::write(&path, &bytes)?;
        Ok(path)
    }
}

/// GET 拉取响应体字节（非 2xx 报错；与 updater 同款语义）
async fn get_bytes(client: &dyn HttpClient, url: &str) -> Result<Vec<u8>> {
    let mut resp = client.get(url, AsyncBody::empty(), true).await?;
    if !resp.status().is_success() {
        return Err(anyhow!("商店返回 HTTP {}", resp.status().as_u16()));
    }
    let mut body = Vec::new();
    resp.body_mut().read_to_end(&mut body).await?;
    Ok(body)
}

/// 构建搜索 URL（纯函数：参数拼查询串，key 与 旧版一致）
fn build_search_url(base_url: &str, params: &SearchParams) -> String {
    let mut query_params = Vec::new();
    if let Some(q) = &params.query {
        query_params.push(format!("q={}", encode_query(q)));
    }
    if let Some(cat) = &params.category {
        query_params.push(format!("category={}", encode_query(cat)));
    }
    if let Some(sort) = &params.sort {
        query_params.push(format!("sort={sort}"));
    }
    query_params.push(format!("page={}", params.page));
    query_params.push(format!("per_page={}", params.per_page));

    format!("{base_url}/plugins?{}", query_params.join("&"))
}

/// 从 Content-Disposition 头提取文件名（Tauri 同款解析；缺头回退 "<id>.ilp"）
fn extract_filename(
    content_disposition: Option<&gpui_kit::http_client::http::HeaderValue>,
    plugin_id: &str,
) -> String {
    if let Some(cd) = content_disposition.and_then(|v| v.to_str().ok()) {
        for part in cd.split(';') {
            let trimmed = part.trim();
            if let Some(name) = trimmed.strip_prefix("filename=") {
                return name.trim_matches('"').to_string();
            }
        }
    }
    format!("{plugin_id}.ilp")
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::http_client::http::HeaderValue;

    #[test]
    fn build_search_url_all_params() {
        let params = SearchParams {
            query: Some("天气 插件".into()),
            category: Some("tools".into()),
            sort: Some("downloads".into()),
            page: 2,
            per_page: 20,
        };
        let url = build_search_url("https://plugins.ilauncher.com/api", &params);
        assert_eq!(
            url,
            "https://plugins.ilauncher.com/api/plugins?q=%E5%A4%A9%E6%B0%94%20%E6%8F%92%E4%BB%B6&category=tools&sort=downloads&page=2&per_page=20"
        );
    }

    #[test]
    fn build_search_url_minimal_params() {
        let url = build_search_url("https://x.test/api", &SearchParams::default());
        assert_eq!(url, "https://x.test/api/plugins?page=0&per_page=0");
    }

    #[test]
    fn filename_from_content_disposition() {
        let cd = HeaderValue::from_str(r#"attachment; filename="my-plugin.ilp""#).unwrap();
        assert_eq!(extract_filename(Some(&cd), "fallback"), "my-plugin.ilp");
        // 无头 → 回退 plugin_id.ilp
        assert_eq!(extract_filename(None, "com.x.y"), "com.x.y.ilp");
        // 有头但无 filename 段 → 回退
        let cd2 = HeaderValue::from_str("attachment").unwrap();
        assert_eq!(extract_filename(Some(&cd2), "com.x.y"), "com.x.y.ilp");
    }

    #[test]
    fn search_result_deserialization() {
        let json = serde_json::json!({
            "total": 1,
            "page": 1,
            "per_page": 20,
            "plugins": [{
                "id": "com.example.weather",
                "name": "Weather",
                "version": "1.0.0",
                "description": "查询天气预报",
                "author": "Example Corp",
                "downloads": 1000,
                "rating": 4.5,
                "icon_url": "https://example.com/icon.png",
                "download_url": "https://example.com/weather.ilp"
            }]
        });
        let result: SearchResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.plugins[0].id, "com.example.weather");
        assert_eq!(result.plugins[0].keywords, Vec::<String>::new());
    }

    /// Mock HTTP 客户端（共享实现在 test_util）
    fn mock(body: serde_json::Value, status: u16) -> crate::test_util::MockHttp {
        crate::test_util::MockHttp {
            body: serde_json::to_vec(&body).unwrap(),
            status,
        }
    }

    #[test]
    fn search_flow_with_mock_client() {
        let client = mock(serde_json::json!({
            "total": 0, "page": 1, "per_page": 20, "plugins": []
        }), 200);
        let store = PluginStore::with_base_url("https://x.test/api", PathBuf::from("."));
        let result = futures::executor::block_on(store.search(
            &client,
            SearchParams { query: Some("a".into()), page: 1, per_page: 20, ..Default::default() },
        ))
        .unwrap();
        assert_eq!(result.total, 0);
    }

    #[test]
    fn search_flow_http_error() {
        let client = mock(serde_json::json!(null), 500);
        let store = PluginStore::with_base_url("https://x.test/api", PathBuf::from("."));
        assert!(futures::executor::block_on(store.popular(&client, 10)).is_err());
    }
}
