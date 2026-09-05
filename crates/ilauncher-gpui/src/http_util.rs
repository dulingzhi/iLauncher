//! 共享 HTTP 客户端构造（ReqwestClient 单点入口）。

use std::sync::Arc;

use gpui_kit::http_client::HttpClient;

/// 按 User-Agent 创建 HTTP 客户端；失败返回 None（调用处自行降级/报错）
pub fn client(user_agent: &str) -> Option<Arc<dyn HttpClient>> {
    reqwest_client::ReqwestClient::user_agent(user_agent)
        .ok()
        .map(|c| Arc::new(c) as Arc<dyn HttpClient>)
}
