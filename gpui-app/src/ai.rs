//! AI 助手引擎（跨平台，无 gpui 依赖，可单测）。
//!
//! 对齐 Tauri `plugin/ai_assistant.rs` 的对外语义：配置模型、对话模型、
//! send_message 流程（空 key 报错 → 追加用户消息 → 调 API → 追加回复 →
//! 首条消息重命名标题）。与 Tauri 版的差异：
//! - 时间戳 Unix 秒 u64（Tauri 为 chrono i64，同 workflow 处理）
//! - 会话/配置持久化到 JSON（Tauri 仅内存，重启即丢）
//! - Tauri 六个 provider 调用函数大量雷同（openai/custom/deepseek/github
//!   四份几乎相同），这里收敛为纯函数 build_request/parse_response 两张表
//! - 标题截取用 chars()（Tauri 按字节切 30，中文多字节下有 panic 风险）

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use futures::AsyncReadExt;
use gpui_kit::http_client::http::Method;
use gpui_kit::http_client::{AsyncBody, HttpClient};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

pub const DEFAULT_TITLE: &str = "新对话";

// ── 配置与对话模型（serde 字段与 Tauri 版一致） ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub provider: String, // openai / anthropic / github / deepseek / gemini / ollama / custom
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
    pub temperature: f32,
    pub max_tokens: usize,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            api_key: String::new(),
            model: "gpt-3.5-turbo".into(),
            base_url: None,
            temperature: 0.7,
            max_tokens: 2000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String, // system / user / assistant
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub messages: Vec<ChatMessage>,
    /// Unix 秒（Tauri 为 chrono::Local i64）
    pub timestamp: u64,
}

// ── provider 请求构造/响应解析（纯函数，核心去重） ──────────────────────────
//
// Tauri 版六个 async 调用函数 → 两类纯函数：
// - OpenAI 兼容族（openai/custom/deepseek/github copilot）：同 endpoint
//   同请求体同响应 shape，仅默认 base_url 与额外头不同 → 查表
// - anthropic / gemini / ollama：各家一份构造 + 一份解析

/// OpenAI 兼容族的差异表
struct CompatVariant {
    default_base: &'static str,
    extra_headers: &'static [(&'static str, &'static str)],
}

fn compat_variant(provider: &str) -> Option<CompatVariant> {
    match provider {
        // custom 与 openai 完全同构（Tauri 版即复用 openai 函数）
        "openai" | "custom" => {
            Some(CompatVariant { default_base: "https://api.openai.com/v1", extra_headers: &[] })
        }
        "deepseek" => Some(CompatVariant { default_base: "https://api.deepseek.com", extra_headers: &[] }),
        "github" => Some(CompatVariant {
            default_base: "https://api.githubcopilot.com",
            extra_headers: &[
                ("Editor-Version", "vscode/1.85.0"),
                ("Editor-Plugin-Version", "copilot/1.145.0"),
            ],
        }),
        _ => None,
    }
}

fn base_or(config: &AIConfig, default: &str) -> String {
    config.base_url.clone().unwrap_or_else(|| default.to_string())
}

/// 一次 API 调用的完整描述（url/头/JSON 体），由调用层翻译成 HTTP 请求
pub struct ApiRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: serde_json::Value,
}

/// 按 provider 构造请求（纯函数，便于单测）
pub fn build_request(config: &AIConfig, messages: &[ChatMessage]) -> Result<ApiRequest> {
    let provider = config.provider.as_str();
    if let Some(variant) = compat_variant(provider) {
        let mut headers = vec![
            ("Authorization".into(), format!("Bearer {}", config.api_key)),
            ("Content-Type".into(), "application/json".into()),
        ];
        for (k, v) in variant.extra_headers {
            headers.push(((*k).into(), (*v).into()));
        }
        return Ok(ApiRequest {
            url: format!("{}/chat/completions", base_or(config, variant.default_base)),
            headers,
            body: serde_json::json!({
                "model": config.model,
                "messages": messages,
                "temperature": config.temperature,
                "max_tokens": config.max_tokens,
            }),
        });
    }
    match provider {
        "anthropic" => Ok(ApiRequest {
            url: format!("{}/messages", base_or(config, "https://api.anthropic.com/v1")),
            headers: vec![
                ("x-api-key".into(), config.api_key.clone()),
                ("anthropic-version".into(), "2023-06-01".into()),
                ("Content-Type".into(), "application/json".into()),
            ],
            body: serde_json::json!({
                "model": config.model,
                "messages": messages,
                "max_tokens": config.max_tokens,
            }),
        }),
        "gemini" => Ok(ApiRequest {
            url: format!(
                "{}/models/{}:generateContent?key={}",
                base_or(config, "https://generativelanguage.googleapis.com/v1beta"),
                config.model,
                config.api_key
            ),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: serde_json::json!({
                "contents": messages.iter().map(|m| serde_json::json!({
                    "parts": [{ "text": m.content }]
                })).collect::<Vec<_>>(),
            }),
        }),
        "ollama" => Ok(ApiRequest {
            url: format!("{}/api/chat", base_or(config, "http://localhost:11434")),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: serde_json::json!({
                "model": config.model,
                "messages": messages,
                "stream": false,
            }),
        }),
        other => Err(anyhow!("未知 AI provider: {other}")),
    }
}

fn json_path<'v>(v: &'v serde_json::Value, path: &str) -> Result<&'v serde_json::Value> {
    v.pointer(path).with_context(|| format!("AI 响应缺少字段 {path}"))
}

fn first_content(v: &serde_json::Value, path: &str) -> Result<String> {
    Ok(json_path(v, path)?.as_str().unwrap_or_default().to_string())
}

/// 按 provider 解析响应体（纯函数；HTTP 状态码错误由调用层先拦截）
pub fn parse_response(config: &AIConfig, body: &[u8]) -> Result<String> {
    let v: serde_json::Value = serde_json::from_slice(body).context("AI 响应不是合法 JSON")?;
    let provider = config.provider.as_str();
    if compat_variant(provider).is_some() {
        first_content(&v, "/choices/0/message/content")
    } else {
        match provider {
            "anthropic" => first_content(&v, "/content/0/text"),
            "gemini" => first_content(&v, "/candidates/0/content/parts/0/text"),
            "ollama" => first_content(&v, "/message/content"),
            other => Err(anyhow!("未知 AI provider: {other}")),
        }
    }
}

// ── 会话引擎 ────────────────────────────────────────────────────────────────

fn new_id() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("conv-{nanos}-{}", SEQ.fetch_add(1, Ordering::Relaxed))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub struct AiChat {
    config: RwLock<AIConfig>,
    conversations: RwLock<Vec<Conversation>>,
    current: RwLock<Option<String>>,
    http: Arc<dyn HttpClient>,
    /// 持久化目录（ai-config.json + ai-conversations.json）
    dir: PathBuf,
}

impl AiChat {
    pub fn new(http: Arc<dyn HttpClient>, dir: PathBuf) -> Self {
        Self {
            config: RwLock::new(AIConfig::default()),
            conversations: RwLock::new(Vec::new()),
            current: RwLock::new(None),
            http,
            dir,
        }
    }

    /// 加载配置与会话（文件缺失/损坏时静默用默认值，不影响启动）
    pub fn load(&self) {
        if let Ok(text) = std::fs::read_to_string(self.dir.join("ai-config.json"))
            && let Ok(cfg) = serde_json::from_str(&text) {
                *self.config.write() = cfg;
            }
        if let Ok(text) = std::fs::read_to_string(self.dir.join("ai-conversations.json"))
            && let Ok(convs) = serde_json::from_str(&text) {
                *self.conversations.write() = convs;
            }
    }

    pub fn config(&self) -> AIConfig {
        self.config.read().clone()
    }

    /// 保存配置并落盘（API key 本地明文存储——与 Tauri 实际行为一致，
    /// 其设置页标注 encrypted storage 但仅经前端内存传递）
    pub fn save_config(&self, cfg: AIConfig) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(
            self.dir.join("ai-config.json"),
            serde_json::to_string_pretty(&cfg)?,
        )?;
        *self.config.write() = cfg;
        Ok(())
    }

    pub fn conversations(&self) -> Vec<Conversation> {
        self.conversations.read().clone()
    }

    pub fn current_conversation(&self) -> Option<Conversation> {
        let current = self.current.read();
        self.conversations.read().iter().find(|c| Some(&c.id) == current.as_ref()).cloned()
    }

    /// 新建对话并置为当前（新对话插到列表头，对齐 Tauri）
    pub fn create_conversation(&self, title: String) -> String {
        let conv = Conversation {
            id: new_id(),
            title,
            messages: Vec::new(),
            timestamp: now_secs(),
        };
        let id = conv.id.clone();
        let mut convs = self.conversations.write();
        convs.insert(0, conv);
        drop(convs);
        *self.current.write() = Some(id.clone());
        self.persist_conversations();
        id
    }

    pub fn switch_conversation(&self, id: String) {
        *self.current.write() = Some(id);
    }

    pub fn delete_conversation(&self, id: &str) {
        self.conversations.write().retain(|c| c.id != id);
        let mut current = self.current.write();
        if current.as_deref() == Some(id) {
            *current = None;
        }
        drop(current);
        self.persist_conversations();
    }

    /// 发送消息：返回 AI 回复文本（同步追加双方消息，对齐 Tauri 流程）
    pub async fn send_message(&self, message: &str) -> Result<String> {
        let config = self.config.read().clone();
        if config.api_key.is_empty() {
            return Err(anyhow!("API key 未配置"));
        }

        // 无当前对话则新建
        if self.current.read().is_none() {
            self.create_conversation(DEFAULT_TITLE.into());
        }
        let conv_id = self.current.read().clone().expect("刚创建");

        {
            let mut convs = self.conversations.write();
            if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
                conv.messages.push(ChatMessage { role: "user".into(), content: message.into() });
            }
        }

        let messages = self
            .conversations
            .read()
            .iter()
            .find(|c| c.id == conv_id)
            .map(|c| c.messages.clone())
            .unwrap_or_default();

        let response = self.call_api(&config, &messages).await?;

        {
            let mut convs = self.conversations.write();
            if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
                conv.messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: response.clone(),
                });
                // 首条用户消息重命名默认标题（chars 截断，避免 Tauri 字节切边界 panic）
                if conv.title == DEFAULT_TITLE {
                    conv.title = message.chars().take(30).collect();
                }
            }
        }
        self.persist_conversations();
        Ok(response)
    }

    async fn call_api(&self, config: &AIConfig, messages: &[ChatMessage]) -> Result<String> {
        let api = build_request(config, messages)?;
        let mut request = gpui_kit::http_client::http::Request::builder()
            .method(Method::POST)
            .uri(&api.url)
            .body(AsyncBody::from(api.body.to_string()))?;
        for (key, value) in api.headers {
            request.headers_mut().insert(
                gpui_kit::http_client::http::HeaderName::from_bytes(key.as_bytes())?,
                gpui_kit::http_client::http::HeaderValue::from_str(&value)?,
            );
        }
        let mut resp = self.http.send(request).await?;
        let status = resp.status().as_u16();
        let mut bytes = Vec::new();
        resp.body_mut().read_to_end(&mut bytes).await?;
        if !(200..300).contains(&status) {
            // 对齐 Tauri：非 2xx 时报错文本随错误返回
            return Err(anyhow!(
                "{} API 错误 (HTTP {}): {}",
                config.provider,
                status,
                String::from_utf8_lossy(&bytes)
            ));
        }
        parse_response(config, &bytes)
    }

    fn persist_conversations(&self) {
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let _ = std::fs::write(
            self.dir.join("ai-conversations.json"),
            serde_json::to_string_pretty(&*self.conversations.read()).unwrap_or_default(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{self, MockHttp};

    fn cfg(provider: &str) -> AIConfig {
        AIConfig { provider: provider.into(), api_key: "sk-test".into(), ..Default::default() }
    }

    fn msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage { role: role.into(), content: content.into() }
    }

    fn engine(tag: &str, http: Arc<dyn HttpClient>) -> Arc<AiChat> {
        Arc::new(AiChat::new(http, test_util::tempdir(tag)))
    }

    fn cleanup(chat: &AiChat) {
        let _ = std::fs::remove_dir_all(&chat.dir);
    }

    // ── build_request ──

    #[test]
    fn build_openai_compat_request() {
        let api = build_request(&cfg("openai"), &[msg("user", "hi")]).unwrap();
        assert_eq!(api.url, "https://api.openai.com/v1/chat/completions");
        assert!(api.headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer sk-test"));
        assert_eq!(api.body["model"], serde_json::json!("gpt-3.5-turbo"));
        assert_eq!(api.body["messages"][0]["content"], serde_json::json!("hi"));
        // f32 0.7 序列化为 0.699999988079071，用近似比较
        assert!((api.body["temperature"].as_f64().unwrap() - 0.7).abs() < 1e-6);
    }

    #[test]
    fn build_compat_variants_share_shape() {
        // openai/custom/deepseek 同构，仅默认 base 不同；base_url 可覆盖
        for (provider, default_base) in
            [("custom", "https://api.openai.com/v1"), ("deepseek", "https://api.deepseek.com")]
        {
            let api = build_request(&cfg(provider), &[]).unwrap();
            assert_eq!(api.url, format!("{default_base}/chat/completions"));
        }
        let mut c = cfg("deepseek");
        c.base_url = Some("https://proxy.example.com/v1".into());
        let api = build_request(&c, &[]).unwrap();
        assert_eq!(api.url, "https://proxy.example.com/v1/chat/completions");

        // github copilot 额外两个编辑器头
        let api = build_request(&cfg("github"), &[]).unwrap();
        assert_eq!(api.url, "https://api.githubcopilot.com/chat/completions");
        assert!(api.headers.iter().any(|(k, _)| k == "Editor-Version"));
        assert!(api.headers.iter().any(|(k, _)| k == "Editor-Plugin-Version"));
    }

    #[test]
    fn build_anthropic_request() {
        let api = build_request(&cfg("anthropic"), &[msg("user", "hi")]).unwrap();
        assert_eq!(api.url, "https://api.anthropic.com/v1/messages");
        assert!(api.headers.iter().any(|(k, v)| k == "x-api-key" && v == "sk-test"));
        assert!(api.headers.iter().any(|(k, v)| k == "anthropic-version" && v == "2023-06-01"));
        assert_eq!(api.body["max_tokens"], serde_json::json!(2000));
        assert!(api.body.get("temperature").is_none(), "anthropic 请求不带 temperature");
    }

    #[test]
    fn build_gemini_request() {
        let api = build_request(&cfg("gemini"), &[msg("user", "hi"), msg("assistant", "hey")]).unwrap();
        assert!(api.url.starts_with("https://generativelanguage.googleapis.com/v1beta/models/gpt-3.5-turbo:generateContent?key=sk-test"));
        assert_eq!(api.body["contents"][0]["parts"][0]["text"], serde_json::json!("hi"));
        assert_eq!(api.body["contents"][1]["parts"][0]["text"], serde_json::json!("hey"));
    }

    #[test]
    fn build_ollama_request() {
        let api = build_request(&cfg("ollama"), &[msg("user", "hi")]).unwrap();
        assert_eq!(api.url, "http://localhost:11434/api/chat");
        assert_eq!(api.body["stream"], serde_json::json!(false));
    }

    #[test]
    fn build_unknown_provider_errors() {
        assert!(build_request(&cfg("azure"), &[]).is_err());
    }

    // ── parse_response ──

    #[test]
    fn parse_compat_response() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"你好"}}]}"#.as_bytes();
        assert_eq!(parse_response(&cfg("openai"), body).unwrap(), "你好");
        assert_eq!(parse_response(&cfg("deepseek"), body).unwrap(), "你好");
    }

    #[test]
    fn parse_anthropic_response() {
        let body = br#"{"content":[{"type":"text","text":"hi"}]}"#;
        assert_eq!(parse_response(&cfg("anthropic"), body).unwrap(), "hi");
    }

    #[test]
    fn parse_gemini_response() {
        let body = br#"{"candidates":[{"content":{"parts":[{"text":"hi"}]}}]}"#;
        assert_eq!(parse_response(&cfg("gemini"), body).unwrap(), "hi");
    }

    #[test]
    fn parse_ollama_response() {
        let body = br#"{"message":{"role":"assistant","content":"hi"}}"#;
        assert_eq!(parse_response(&cfg("ollama"), body).unwrap(), "hi");
    }

    #[test]
    fn parse_garbage_errors() {
        assert!(parse_response(&cfg("openai"), b"not json").is_err());
        assert!(parse_response(&cfg("openai"), br#"{"unexpected":true}"#).is_err());
    }

    // ── 引擎行为 ──

    #[test]
    fn send_message_appends_both_and_renames_title() {
        let chat = engine("flow", MockHttp::arc(r#"{"choices":[{"message":{"content":"收到"}}]}"#.as_bytes(), 200));
        chat.save_config(cfg("openai")).unwrap();
        let reply = futures::executor::block_on(chat.send_message("介绍一下 Rust 语言的特点")).unwrap();
        assert_eq!(reply, "收到");
        let conv = chat.current_conversation().unwrap();
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[0].role, "user");
        assert_eq!(conv.messages[1].role, "assistant");
        assert_eq!(conv.title, "介绍一下 Rust 语言的特点");
        // 会话已持久化（重新加载可见）
        let convs = std::fs::read_to_string(chat.dir.join("ai-conversations.json")).unwrap();
        assert!(convs.contains("介绍一下 Rust 语言的特点"));
        cleanup(&chat);
    }

    #[test]
    fn send_message_without_key_errors() {
        let chat = engine("nokey", MockHttp::arc(&[], 200));
        assert!(futures::executor::block_on(chat.send_message("hi")).is_err());
        cleanup(&chat);
    }

    #[test]
    fn http_error_includes_status_and_body() {
        let chat = engine("httperr", MockHttp::arc(br#"{"error":"invalid key"}"#, 401));
        chat.save_config(cfg("openai")).unwrap();
        let err = futures::executor::block_on(chat.send_message("hi")).unwrap_err();
        let text = format!("{err:#}");
        assert!(text.contains("401"), "{text}");
        assert!(text.contains("invalid key"), "{text}");
        cleanup(&chat);
    }

    #[test]
    fn conversation_crud() {
        let chat = engine("crud", MockHttp::arc(&[], 200));
        let a = chat.create_conversation(DEFAULT_TITLE.into());
        let b = chat.create_conversation("b".into());
        assert_eq!(chat.current_conversation().unwrap().id, b, "最新创建为当前");
        chat.switch_conversation(a.clone());
        assert_eq!(chat.current_conversation().unwrap().id, a);
        chat.delete_conversation(&a);
        assert!(chat.current_conversation().is_none(), "删除当前对话后清空");
        assert_eq!(chat.conversations().len(), 1);
        cleanup(&chat);
    }
}
