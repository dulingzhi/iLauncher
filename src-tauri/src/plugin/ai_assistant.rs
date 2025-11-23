// AI 助手插件 - 支持 ChatGPT/Claude 对话

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub provider: String, // "openai" or "anthropic"
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
    pub temperature: f32,
    pub max_tokens: usize,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            api_key: String::new(),
            model: "gpt-3.5-turbo".to_string(),
            base_url: None,
            temperature: 0.7,
            max_tokens: 2000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // system, user, assistant
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub messages: Vec<ChatMessage>,
    pub timestamp: i64,
}

pub struct AIAssistantPlugin {
    metadata: PluginMetadata,
    config: Arc<RwLock<AIConfig>>,
    conversations: Arc<RwLock<Vec<Conversation>>>,
    current_conversation: Arc<RwLock<Option<String>>>, // current conversation ID
    client: Client,
    matcher: SkimMatcherV2,
}

impl AIAssistantPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "ai_assistant".to_string(),
                name: "AI Assistant".to_string(),
                description: "Chat with AI (ChatGPT/Claude)".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("🤖"),
                trigger_keywords: vec![
                    "ai".to_string(),
                    "gpt".to_string(),
                    "chat".to_string(),
                    "ask".to_string(),
                ],
                commands: vec![],
                settings: vec![
                    SettingDefinition {
                        r#type: "select".to_string(),
                        key: Some("provider".to_string()),
                        label: Some("AI Provider (openai/anthropic)".to_string()),
                        value: Some(serde_json::json!("openai")),
                    },
                    SettingDefinition {
                        r#type: "password".to_string(),
                        key: Some("api_key".to_string()),
                        label: Some("API Key (encrypted storage)".to_string()),
                        value: Some(serde_json::json!("")),
                    },
                    SettingDefinition {
                        r#type: "text".to_string(),
                        key: Some("model".to_string()),
                        label: Some("Model (e.g. gpt-3.5-turbo)".to_string()),
                        value: Some(serde_json::json!("gpt-3.5-turbo")),
                    },
                ],
                supported_os: vec![
                    "windows".to_string(),
                    "macos".to_string(),
                    "linux".to_string(),
                ],
                plugin_type: PluginType::Native,
            },
            config: Arc::new(RwLock::new(AIConfig::default())),
            conversations: Arc::new(RwLock::new(Vec::new())),
            current_conversation: Arc::new(RwLock::new(None)),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("Failed to build HTTP client for AI Assistant"),
            matcher: SkimMatcherV2::default(),
        }
    }

    /// 加载配置
    pub async fn load_config(&self, config: AIConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
        tracing::info!("AI Assistant config loaded: provider={}", cfg.provider);
    }

    /// 获取配置
    pub async fn get_config(&self) -> AIConfig {
        self.config.read().await.clone()
    }

    /// 创建新对话
    pub async fn create_conversation(&self, title: String) -> String {
        let conv = Conversation {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            messages: vec![],
            timestamp: chrono::Local::now().timestamp(),
        };

        let id = conv.id.clone();
        let mut convs = self.conversations.write().await;
        convs.insert(0, conv);

        // 设为当前对话
        let mut current = self.current_conversation.write().await;
        *current = Some(id.clone());

        tracing::info!("Created new conversation: {}", id);
        id
    }

    /// 发送消息到 AI
    pub async fn send_message(&self, message: String) -> Result<String> {
        let config = self.config.read().await.clone();

        if config.api_key.is_empty() {
            return Err(anyhow::anyhow!("API key not configured"));
        }

        // 获取或创建当前对话
        let conv_id = {
            let mut current = self.current_conversation.write().await;
            if current.is_none() {
                *current = Some(self.create_conversation("New Chat".to_string()).await);
            }
            current.clone().unwrap()
        };

        // 添加用户消息
        {
            let mut convs = self.conversations.write().await;
            if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
                conv.messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: message.clone(),
                });
            }
        }

        // 调用 AI API
        let response = match config.provider.as_str() {
            "openai" => self.call_openai_api(&config, &conv_id).await?,
            "anthropic" => self.call_anthropic_api(&config, &conv_id).await?,
            "github" => self.call_github_copilot_api(&config, &conv_id).await?,
            "deepseek" => self.call_deepseek_api(&config, &conv_id).await?,
            "gemini" => self.call_gemini_api(&config, &conv_id).await?,
            "ollama" => self.call_ollama_api(&config, &conv_id).await?,
            "custom" => self.call_openai_api(&config, &conv_id).await?, // Use OpenAI-compatible format
            _ => return Err(anyhow::anyhow!("Unknown provider: {}", config.provider)),
        };

        // 添加 AI 响应
        {
            let mut convs = self.conversations.write().await;
            if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
                conv.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: response.clone(),
                });

                // 更新标题（使用第一条用户消息）
                if conv.title == "New Chat" && !conv.messages.is_empty() {
                    conv.title = message[..message.len().min(30)].to_string();
                }
            }
        }

        Ok(response)
    }

    /// 调用 OpenAI API
    async fn call_openai_api(&self, config: &AIConfig, conv_id: &str) -> Result<String> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        // 获取对话历史
        let messages = {
            let convs = self.conversations.read().await;
            convs
                .iter()
                .find(|c| c.id == conv_id)
                .map(|c| c.messages.clone())
                .unwrap_or_default()
        };

        #[derive(Serialize)]
        struct OpenAIRequest {
            model: String,
            messages: Vec<ChatMessage>,
            temperature: f32,
            max_tokens: usize,
        }

        #[derive(Deserialize)]
        struct OpenAIResponse {
            choices: Vec<OpenAIChoice>,
        }

        #[derive(Deserialize)]
        struct OpenAIChoice {
            message: ChatMessage,
        }

        let request = OpenAIRequest {
            model: config.model.clone(),
            messages,
            temperature: config.temperature,
            max_tokens: config.max_tokens,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("OpenAI API error: {}", error_text));
        }

        let result: OpenAIResponse = response.json().await?;
        Ok(result
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }

    /// 调用 Anthropic API (Claude)
    async fn call_anthropic_api(&self, config: &AIConfig, conv_id: &str) -> Result<String> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());

        // 获取对话历史
        let messages = {
            let convs = self.conversations.read().await;
            convs
                .iter()
                .find(|c| c.id == conv_id)
                .map(|c| c.messages.clone())
                .unwrap_or_default()
        };

        #[derive(Serialize)]
        struct AnthropicRequest {
            model: String,
            messages: Vec<ChatMessage>,
            max_tokens: usize,
        }

        #[derive(Deserialize)]
        struct AnthropicResponse {
            content: Vec<AnthropicContent>,
        }

        #[derive(Deserialize)]
        struct AnthropicContent {
            text: String,
        }

        let request = AnthropicRequest {
            model: config.model.clone(),
            messages,
            max_tokens: config.max_tokens,
        };

        let response = self
            .client
            .post(format!("{}/messages", base_url))
            .header("x-api-key", &config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Anthropic API error: {}", error_text));
        }

        let result: AnthropicResponse = response.json().await?;
        Ok(result
            .content
            .first()
            .map(|c| c.text.clone())
            .unwrap_or_default())
    }

    /// 获取对话列表
    pub async fn get_conversations(&self) -> Vec<Conversation> {
        self.conversations.read().await.clone()
    }

    /// 切换对话
    pub async fn switch_conversation(&self, conv_id: String) {
        let mut current = self.current_conversation.write().await;
        *current = Some(conv_id);
    }

    /// 删除对话
    pub async fn delete_conversation(&self, conv_id: String) {
        let mut convs = self.conversations.write().await;
        convs.retain(|c| c.id != conv_id);

        // 如果删除的是当前对话，清空当前对话
        let mut current = self.current_conversation.write().await;
        if current.as_ref() == Some(&conv_id) {
            *current = None;
        }
    }
}

#[async_trait]
impl Plugin for AIAssistantPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let search = ctx.search.trim();

        // 检查 API key 配置
        let config = self.config.read().await;
        if config.api_key.is_empty() {
            return Ok(vec![QueryResult {
                id: "config".to_string(),
                title: "AI Assistant - Not Configured".to_string(),
                subtitle: "Click to configure API key in settings".to_string(),
                icon: WoxImage::emoji("⚙️"),
                preview: None,
                score: 100,
                context_data: serde_json::Value::Null,
                group: Some("AI".to_string()),
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![Action {
                    id: "open_settings".to_string(),
                    name: "Open Settings".to_string(),
                    icon: None,
                    is_default: true,
                    prevent_hide: false,
                    hotkey: None,
                }],
            }]);
        }

        let mut results = Vec::new();

        // 如果有搜索词，显示"询问 AI"选项
        if !search.is_empty() {
            results.push(QueryResult {
                id: "ask".to_string(),
                title: format!("Ask AI: {}", search),
                subtitle: format!("Send to {} {}", config.provider, config.model),
                icon: WoxImage::emoji("💬"),
                preview: None,
                score: 100,
                context_data: serde_json::to_value(search)?,
                group: Some("AI".to_string()),
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![Action {
                    id: "send".to_string(),
                    name: "Send Message".to_string(),
                    icon: None,
                    is_default: true,
                    prevent_hide: true, // 不隐藏窗口，等待响应
                    hotkey: None,
                }],
            });
        }

        // 显示对话历史
        let conversations = self.conversations.read().await;
        for conv in conversations.iter().take(5) {
            let last_message = conv
                .messages
                .last()
                .map(|m| {
                    let preview = &m.content[..m.content.len().min(50)];
                    format!("{}: {}", m.role, preview)
                })
                .unwrap_or_else(|| "Empty conversation".to_string());

            let score = if search.is_empty() {
                90
            } else {
                self.matcher
                    .fuzzy_match(&conv.title, search)
                    .unwrap_or(0)
            };

            if score > 0 {
                results.push(QueryResult {
                    id: conv.id.clone(),
                    title: conv.title.clone(),
                    subtitle: last_message,
                    icon: WoxImage::emoji("💭"),
                    preview: Some(Preview::Text(
                        conv.messages
                            .iter()
                            .map(|m| format!("{}: {}", m.role, m.content))
                            .collect::<Vec<_>>()
                            .join("\n\n"),
                    )),
                    score: score as i32,
                    context_data: serde_json::to_value(&conv.id)?,
                    group: Some("Conversations".to_string()),
                    plugin_id: self.metadata.id.clone(),
                    refreshable: false,
                    actions: vec![
                        Action {
                            id: "open".to_string(),
                            name: "Open Conversation".to_string(),
                            icon: None,
                            is_default: true,
                            prevent_hide: false,
                            hotkey: None,
                        },
                        Action {
                            id: "delete".to_string(),
                            name: "Delete".to_string(),
                            icon: None,
                            is_default: false,
                            prevent_hide: false,
                            hotkey: None,
                        },
                    ],
                });
            }
        }

        Ok(results)
    }

    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        match action_id {
            "send" => {
                // 发送消息（实际处理由前端完成，这里只是占位）
                tracing::info!("Sending message to AI: {}", result_id);
            }
            "open" => {
                // 打开对话
                self.switch_conversation(result_id.to_string()).await;
            }
            "delete" => {
                // 删除对话
                self.delete_conversation(result_id.to_string()).await;
            }
            "open_settings" => {
                // 打开设置（由前端处理）
            }
            _ => {}
        }

        Ok(())
    }
}

impl AIAssistantPlugin {
    /// 调用 GitHub Copilot API
    async fn call_github_copilot_api(&self, config: &AIConfig, conv_id: &str) -> Result<String> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.githubcopilot.com".to_string());

        let messages = {
            let convs = self.conversations.read().await;
            convs
                .iter()
                .find(|c| c.id == conv_id)
                .map(|c| c.messages.clone())
                .unwrap_or_default()
        };

        #[derive(Serialize)]
        struct CopilotRequest {
            messages: Vec<ChatMessage>,
            model: String,
            temperature: f32,
            max_tokens: usize,
        }

        #[derive(Deserialize)]
        struct CopilotResponse {
            choices: Vec<CopilotChoice>,
        }

        #[derive(Deserialize)]
        struct CopilotChoice {
            message: ChatMessage,
        }

        let request = CopilotRequest {
            messages,
            model: config.model.clone(),
            temperature: config.temperature,
            max_tokens: config.max_tokens,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .header("Editor-Version", "vscode/1.85.0")
            .header("Editor-Plugin-Version", "copilot/1.145.0")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("GitHub Copilot API error: {}", error_text));
        }

        let result: CopilotResponse = response.json().await?;
        Ok(result
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }

    /// 调用 DeepSeek API (OpenAI compatible)
    async fn call_deepseek_api(&self, config: &AIConfig, conv_id: &str) -> Result<String> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.deepseek.com".to_string());

        let messages = {
            let convs = self.conversations.read().await;
            convs
                .iter()
                .find(|c| c.id == conv_id)
                .map(|c| c.messages.clone())
                .unwrap_or_default()
        };

        #[derive(Serialize)]
        struct DeepSeekRequest {
            model: String,
            messages: Vec<ChatMessage>,
            temperature: f32,
            max_tokens: usize,
        }

        #[derive(Deserialize)]
        struct DeepSeekResponse {
            choices: Vec<DeepSeekChoice>,
        }

        #[derive(Deserialize)]
        struct DeepSeekChoice {
            message: ChatMessage,
        }

        let request = DeepSeekRequest {
            model: config.model.clone(),
            messages,
            temperature: config.temperature,
            max_tokens: config.max_tokens,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("DeepSeek API error: {}", error_text));
        }

        let result: DeepSeekResponse = response.json().await?;
        Ok(result
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }

    /// 调用 Google Gemini API
    async fn call_gemini_api(&self, config: &AIConfig, conv_id: &str) -> Result<String> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string());

        let messages = {
            let convs = self.conversations.read().await;
            convs
                .iter()
                .find(|c| c.id == conv_id)
                .map(|c| c.messages.clone())
                .unwrap_or_default()
        };

        // Gemini uses different message format
        #[derive(Serialize, Deserialize)]
        struct GeminiContent {
            parts: Vec<GeminiPart>,
        }

        #[derive(Serialize, Deserialize)]
        struct GeminiPart {
            text: String,
        }

        #[derive(Serialize)]
        struct GeminiRequest {
            contents: Vec<GeminiContent>,
        }

        #[derive(Deserialize)]
        struct GeminiResponse {
            candidates: Vec<GeminiCandidate>,
        }

        #[derive(Deserialize)]
        struct GeminiCandidate {
            content: GeminiContent,
        }

        let contents: Vec<GeminiContent> = messages
            .iter()
            .map(|m| GeminiContent {
                parts: vec![GeminiPart {
                    text: m.content.clone(),
                }],
            })
            .collect();

        let request = GeminiRequest { contents };

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            base_url, config.model, config.api_key
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Gemini API error: {}", error_text));
        }

        let result: GeminiResponse = response.json().await?;
        Ok(result
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.clone())
            .unwrap_or_default())
    }

    /// 调用 Ollama API (Local)
    async fn call_ollama_api(&self, config: &AIConfig, conv_id: &str) -> Result<String> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());

        let messages = {
            let convs = self.conversations.read().await;
            convs
                .iter()
                .find(|c| c.id == conv_id)
                .map(|c| c.messages.clone())
                .unwrap_or_default()
        };

        #[derive(Serialize)]
        struct OllamaRequest {
            model: String,
            messages: Vec<ChatMessage>,
            stream: bool,
        }

        #[derive(Deserialize)]
        struct OllamaResponse {
            message: ChatMessage,
        }

        let request = OllamaRequest {
            model: config.model.clone(),
            messages,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", base_url))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Ollama API error: {}", error_text));
        }

        let result: OllamaResponse = response.json().await?;
        Ok(result.message.content)
    }
}
