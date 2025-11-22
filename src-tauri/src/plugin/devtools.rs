// 开发者工具插件 - JSON/Base64/Hash/URL等工具

use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use serde_json::Value;
use std::fmt::Write as _;

pub struct DevToolsPlugin {
    metadata: PluginMetadata,
}

impl DevToolsPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "devtools".to_string(),
                name: "开发工具".to_string(),
                description: "JSON格式化、Base64编解码、哈希计算、URL编解码等".to_string(),
                icon: WoxImage::Emoji("🔧".to_string()),
                version: "1.0.0".to_string(),
                author: "iLauncher".to_string(),
                trigger_keywords: vec![
                    "json".to_string(),
                    "base64".to_string(),
                    "hash".to_string(),
                    "md5".to_string(),
                    "sha256".to_string(),
                    "url".to_string(),
                    "uuid".to_string(),
                ],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "linux".to_string(), "macos".to_string()],
                plugin_type: PluginType::Native,
            },
        }
    }

    fn format_json(&self, input: &str) -> Result<String> {
        let value: Value = serde_json::from_str(input)?;
        Ok(serde_json::to_string_pretty(&value)?)
    }

    fn minify_json(&self, input: &str) -> Result<String> {
        let value: Value = serde_json::from_str(input)?;
        Ok(serde_json::to_string(&value)?)
    }

    fn base64_encode(&self, input: &str) -> String {
        general_purpose::STANDARD.encode(input.as_bytes())
    }

    fn base64_decode(&self, input: &str) -> Result<String> {
        let decoded = general_purpose::STANDARD.decode(input)?;
        Ok(String::from_utf8(decoded)?)
    }

    fn calculate_md5(&self, input: &str) -> String {
        format!("{:x}", md5::compute(input.as_bytes()))
    }

    fn calculate_sha256(&self, input: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        let mut output = String::new();
        for byte in result.iter() {
            write!(&mut output, "{:02x}", byte).unwrap();
        }
        output
    }

    fn url_encode(&self, input: &str) -> String {
        urlencoding::encode(input).to_string()
    }

    fn url_decode(&self, input: &str) -> String {
        urlencoding::decode(input).unwrap_or_default().to_string()
    }

    fn generate_uuid(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[async_trait]
impl crate::plugin::Plugin for DevToolsPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim();
        
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        // JSON 格式化
        if query.starts_with("json ") {
            let input = &query[5..];
            
            if !input.is_empty() {
                // 尝试格式化
                match self.format_json(input) {
                    Ok(formatted) => {
                        results.push(QueryResult {
                            id: formatted.clone(),
                            plugin_id: self.metadata.id.clone(),
                            title: "JSON 格式化结果".to_string(),
                            subtitle: format!("{} 行", formatted.lines().count()),
                            icon: WoxImage::emoji("📋".to_string()),
                            score: 100,
                            context_data: serde_json::json!({"type": "json_format", "value": formatted}),
                            actions: vec![
                                Action {
                                    id: "copy".to_string(),
                                    name: "复制".to_string(),
                                    icon: None,
                                    is_default: true,
                                    hotkey: None,
                                    prevent_hide: false,
                                },
                            ],
                            preview: Some(Preview::Text(formatted)),
                            refreshable: false,
                            group: None,
                        });
                    }
                    Err(e) => {
                        results.push(QueryResult {
                            id: "error".to_string(),
                            plugin_id: self.metadata.id.clone(),
                            title: "JSON 解析失败".to_string(),
                            subtitle: format!("错误: {}", e),
                            icon: WoxImage::emoji("❌".to_string()),
                            score: 100,
                            context_data: serde_json::Value::Null,
                            actions: vec![],
                            preview: None,
                            refreshable: false,
                            group: None,
                        });
                    }
                }

                // 尝试压缩
                if let Ok(minified) = self.minify_json(input) {
                    results.push(QueryResult {
                        id: minified.clone(),
                        plugin_id: self.metadata.id.clone(),
                        title: "JSON 压缩结果".to_string(),
                        subtitle: format!("{} 字符", minified.len()),
                        icon: WoxImage::emoji("🗜️".to_string()),
                        score: 90,
                        context_data: serde_json::json!({"type": "json_minify", "value": minified}),
                        actions: vec![
                            Action {
                                id: "copy".to_string(),
                                name: "复制".to_string(),
                                icon: None,
                                is_default: true,
                                hotkey: None,
                                prevent_hide: false,
                            },
                        ],
                        preview: None,
                        refreshable: false,
                        group: None,
                    });
                }
            }
        }

        // Base64 编码
        if query.starts_with("base64 ") {
            let input = &query[7..];
            if !input.is_empty() {
                let encoded = self.base64_encode(input);
                results.push(QueryResult {
                    id: encoded.clone(),
                    plugin_id: self.metadata.id.clone(),
                    title: encoded.clone(),
                    subtitle: "Base64 编码结果".to_string(),
                    icon: WoxImage::emoji("🔐".to_string()),
                    score: 100,
                    context_data: serde_json::json!({"type": "base64_encode", "value": encoded}),
                    actions: vec![
                        Action {
                            id: "copy".to_string(),
                            name: "复制".to_string(),
                            icon: None,
                            is_default: true,
                            hotkey: None,
                            prevent_hide: false,
                        },
                    ],
                    preview: None,
                    refreshable: false,
                    group: None,
                });

                // 同时尝试解码（如果输入看起来像base64）
                if let Ok(decoded) = self.base64_decode(input) {
                    results.push(QueryResult {
                        id: decoded.clone(),
                        plugin_id: self.metadata.id.clone(),
                        title: decoded.clone(),
                        subtitle: "Base64 解码结果".to_string(),
                        icon: WoxImage::emoji("🔓".to_string()),
                        score: 90,
                        context_data: serde_json::json!({"type": "base64_decode", "value": decoded}),
                        actions: vec![
                            Action {
                                id: "copy".to_string(),
                                name: "复制".to_string(),
                                icon: None,
                                is_default: true,
                                hotkey: None,
                                prevent_hide: false,
                            },
                        ],
                        preview: None,
                        refreshable: false,
                        group: None,
                    });
                }
            }
        }

        // MD5 哈希
        if query.starts_with("md5 ") || query.starts_with("hash ") {
            let input = if query.starts_with("md5 ") { &query[4..] } else { &query[5..] };
            if !input.is_empty() {
                let hash = self.calculate_md5(input);
                results.push(QueryResult {
                    id: hash.clone(),
                    plugin_id: self.metadata.id.clone(),
                    title: hash.clone(),
                    subtitle: "MD5 哈希".to_string(),
                    icon: WoxImage::emoji("#️⃣".to_string()),
                    score: 100,
                    context_data: serde_json::json!({"type": "md5", "value": hash}),
                    actions: vec![
                        Action {
                            id: "copy".to_string(),
                            name: "复制".to_string(),
                            icon: None,
                            is_default: true,
                            hotkey: None,
                            prevent_hide: false,
                        },
                    ],
                    preview: None,
                    refreshable: false,
                    group: None,
                });
            }
        }

        // SHA256 哈希
        if query.starts_with("sha256 ") {
            let input = &query[7..];
            if !input.is_empty() {
                let hash = self.calculate_sha256(input);
                results.push(QueryResult {
                    id: hash.clone(),
                    plugin_id: self.metadata.id.clone(),
                    title: hash.clone(),
                    subtitle: "SHA256 哈希".to_string(),
                    icon: WoxImage::emoji("#️⃣".to_string()),
                    score: 100,
                    context_data: serde_json::json!({"type": "sha256", "value": hash}),
                    actions: vec![
                        Action {
                            id: "copy".to_string(),
                            name: "复制".to_string(),
                            icon: None,
                            is_default: true,
                            hotkey: None,
                            prevent_hide: false,
                        },
                    ],
                    preview: None,
                    refreshable: false,
                    group: None,
                });
            }
        }

        // URL 编解码
        if query.starts_with("url ") {
            let input = &query[4..];
            if !input.is_empty() {
                // 编码
                let encoded = self.url_encode(input);
                results.push(QueryResult {
                    id: encoded.clone(),
                    plugin_id: self.metadata.id.clone(),
                    title: encoded.clone(),
                    subtitle: "URL 编码".to_string(),
                    icon: WoxImage::emoji("🔗".to_string()),
                    score: 100,
                    context_data: serde_json::json!({"type": "url_encode", "value": encoded}),
                    actions: vec![
                        Action {
                            id: "copy".to_string(),
                            name: "复制".to_string(),
                            icon: None,
                            is_default: true,
                            hotkey: None,
                            prevent_hide: false,
                        },
                    ],
                    preview: None,
                    refreshable: false,
                    group: None,
                });

                // 解码
                let decoded = self.url_decode(input);
                if decoded != input {
                    results.push(QueryResult {
                        id: decoded.clone(),
                        plugin_id: self.metadata.id.clone(),
                        title: decoded.clone(),
                        subtitle: "URL 解码".to_string(),
                        icon: WoxImage::emoji("🔓".to_string()),
                        score: 90,
                        context_data: serde_json::json!({"type": "url_decode", "value": decoded}),
                        actions: vec![
                            Action {
                                id: "copy".to_string(),
                                name: "复制".to_string(),
                                icon: None,
                                is_default: true,
                                hotkey: None,
                                prevent_hide: false,
                            },
                        ],
                        preview: None,
                        refreshable: false,
                        group: None,
                    });
                }
            }
        }

        // UUID 生成
        if query == "uuid" || query == "uuid " {
            let uuid = self.generate_uuid();
            results.push(QueryResult {
                id: uuid.clone(),
                plugin_id: self.metadata.id.clone(),
                title: uuid.clone(),
                subtitle: "生成 UUID v4".to_string(),
                icon: WoxImage::emoji("🆔".to_string()),
                score: 100,
                context_data: serde_json::json!({"type": "uuid", "value": uuid}),
                actions: vec![
                    Action {
                        id: "copy".to_string(),
                        name: "复制".to_string(),
                        icon: None,
                        is_default: true,
                        hotkey: None,
                        prevent_hide: false,
                    },
                ],
                preview: None,
                refreshable: true,
                group: None,
            });
        }

        Ok(results)
    }

    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        match action_id {
            "copy" => {
                use arboard::Clipboard;
                let mut clipboard = Clipboard::new()?;
                clipboard.set_text(result_id)?;
                tracing::info!("Copied to clipboard: {}", result_id);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unknown action: {}", action_id)),
        }
    }
}
