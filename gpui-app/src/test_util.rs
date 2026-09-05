//! 测试共享辅助：临时目录 + Mock HTTP 客户端（各模块测试统一入口）。

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use gpui_kit::http_client::http::{Request, Response, StatusCode};
use gpui_kit::http_client::{AsyncBody, HttpClient};

/// 进程内唯一测试临时目录（tag 区分用例，pid 防跨进程碰撞；先清空再创建返回）
pub(crate) fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ilauncher_{tag}_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("创建测试临时目录失败");
    dir
}

/// 返回预设状态码 + 响应体的 HTTP Mock（请求形状由纯函数测试覆盖，不经网络）
pub(crate) struct MockHttp {
    pub body: Vec<u8>,
    pub status: u16,
}

impl MockHttp {
    pub(crate) fn arc(body: &[u8], status: u16) -> Arc<Self> {
        Arc::new(Self { body: body.to_vec(), status })
    }
}

impl HttpClient for MockHttp {
    fn user_agent(&self) -> Option<&gpui_kit::http_client::http::HeaderValue> {
        None
    }
    fn proxy(&self) -> Option<&gpui_kit::http_client::Url> {
        None
    }
    fn send(&self, _req: Request<AsyncBody>) -> BoxFuture<'static, Result<Response<AsyncBody>>> {
        let body = self.body.clone();
        let status = self.status;
        Box::pin(async move {
            let mut resp = Response::new(AsyncBody::from(body));
            *resp.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
            Ok(resp)
        })
    }
}
