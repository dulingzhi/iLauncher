// 翻译插件

use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranslationResult {
    source_lang: String,
    target_lang: String,
    source_text: String,
    translated_text: String,
    engine: String,
}

pub struct TranslatorPlugin {
    metadata: PluginMetadata,
    // 简单的本地词典（可扩展为文件加载）
    dictionary: HashMap<String, String>,
}

impl TranslatorPlugin {
    pub fn new() -> Self {
        let mut dictionary = HashMap::new();
        
        // 添加一些常用编程术语（示例）
        dictionary.insert("hello".to_string(), "你好".to_string());
        dictionary.insert("world".to_string(), "世界".to_string());
        dictionary.insert("computer".to_string(), "计算机".to_string());
        dictionary.insert("program".to_string(), "程序".to_string());
        dictionary.insert("code".to_string(), "代码".to_string());
        dictionary.insert("plugin".to_string(), "插件".to_string());
        dictionary.insert("search".to_string(), "搜索".to_string());
        dictionary.insert("file".to_string(), "文件".to_string());
        dictionary.insert("folder".to_string(), "文件夹".to_string());
        dictionary.insert("error".to_string(), "错误".to_string());
        dictionary.insert("success".to_string(), "成功".to_string());
        
        Self {
            metadata: PluginMetadata {
                id: "translator".to_string(),
                name: "翻译".to_string(),
                description: "文本翻译（本地词典 + 在线API）".to_string(),
                icon: WoxImage::Emoji("🌍".to_string()),
                version: "1.0.0".to_string(),
                author: "iLauncher".to_string(),
                trigger_keywords: vec!["trans".to_string(), "tr".to_string(), "翻译".to_string()],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "linux".to_string(), "macos".to_string()],
                plugin_type: PluginType::Native,
            },
            dictionary,
        }
    }

    fn detect_language(&self, text: &str) -> String {
        // 简单的语言检测：如果包含中文字符则为中文，否则为英文
        if text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) {
            "zh".to_string()
        } else {
            "en".to_string()
        }
    }

    async fn translate_local(&self, text: &str, source_lang: &str, target_lang: &str) -> Option<TranslationResult> {
        // 尝试本地词典
        if source_lang == "en" && target_lang == "zh" {
            if let Some(translation) = self.dictionary.get(&text.to_lowercase()) {
                return Some(TranslationResult {
                    source_lang: source_lang.to_string(),
                    target_lang: target_lang.to_string(),
                    source_text: text.to_string(),
                    translated_text: translation.clone(),
                    engine: "本地词典".to_string(),
                });
            }
        }
        None
    }

    async fn translate_online(&self, text: &str, source_lang: &str, target_lang: &str) -> Result<TranslationResult> {
        // 使用免费的翻译API（LibreTranslate或其他免费服务）
        // 这里先返回一个占位结果，你可以后续添加真实的API调用
        
        // 尝试使用 Google Translate 的非官方接口
        let client = reqwest::Client::new();
        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
            source_lang,
            target_lang,
            urlencoding::encode(text)
        );

        match client.get(&url).send().await {
            Ok(response) => {
                if let Ok(body) = response.text().await {
                    // 简单解析返回的JSON（实际格式比较复杂）
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(translations) = json[0].as_array() {
                            let mut result = String::new();
                            for item in translations {
                                if let Some(text) = item[0].as_str() {
                                    result.push_str(text);
                                }
                            }
                            if !result.is_empty() {
                                return Ok(TranslationResult {
                                    source_lang: source_lang.to_string(),
                                    target_lang: target_lang.to_string(),
                                    source_text: text.to_string(),
                                    translated_text: result,
                                    engine: "Google Translate".to_string(),
                                });
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Online translation failed: {}", e);
            }
        }

        // 如果在线翻译失败，返回提示信息
        Ok(TranslationResult {
            source_lang: source_lang.to_string(),
            target_lang: target_lang.to_string(),
            source_text: text.to_string(),
            translated_text: format!("翻译服务暂时不可用：{}", text),
            engine: "离线模式".to_string(),
        })
    }
}

#[async_trait]
impl crate::plugin::Plugin for TranslatorPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim();
        
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // 检查触发词
        let text = if query.starts_with("trans ") {
            &query[6..]
        } else if query.starts_with("tr ") {
            &query[3..]
        } else if query.starts_with("翻译 ") {
            &query["翻译 ".len()..]
        } else {
            return Ok(Vec::new());
        };

        if text.is_empty() {
            return Ok(vec![QueryResult {
                id: "help".to_string(),
                plugin_id: self.metadata.id.clone(),
                title: "翻译".to_string(),
                subtitle: "输入要翻译的文本，例如：trans hello 或 tr 你好".to_string(),
                icon: WoxImage::emoji("💡".to_string()),
                score: 100,
                context_data: serde_json::Value::Null,
                actions: vec![],
                preview: None,
                refreshable: false,
                group: None,
            }]);
        }

        let mut results = Vec::new();

        // 检测源语言
        let source_lang = self.detect_language(text);
        let target_lang = if source_lang == "zh" { "en" } else { "zh" };

        // 尝试本地词典
        if let Some(local_result) = self.translate_local(text, &source_lang, &target_lang).await {
            results.push(QueryResult {
                id: local_result.translated_text.clone(),
                plugin_id: self.metadata.id.clone(),
                title: local_result.translated_text.clone(),
                subtitle: format!("📚 {} | {} → {}", local_result.engine, source_lang.to_uppercase(), target_lang.to_uppercase()),
                icon: WoxImage::emoji("📖".to_string()),
                score: 100,
                context_data: serde_json::to_value(&local_result)?,
                actions: vec![
                    Action {
                        id: "copy".to_string(),
                        name: "复制翻译结果".to_string(),
                        icon: None,
                        is_default: true,
                        hotkey: None,
                        prevent_hide: false,
                    },
                ],
                preview: None,
                refreshable: false,
                group: Some("本地".to_string()),
            });
        }

        // 在线翻译（异步）
        match self.translate_online(text, &source_lang, &target_lang).await {
            Ok(online_result) => {
                results.push(QueryResult {
                    id: online_result.translated_text.clone(),
                    plugin_id: self.metadata.id.clone(),
                    title: online_result.translated_text.clone(),
                    subtitle: format!("🌐 {} | {} → {}", online_result.engine, source_lang.to_uppercase(), target_lang.to_uppercase()),
                    icon: WoxImage::emoji("🌐".to_string()),
                    score: 90,
                    context_data: serde_json::to_value(&online_result)?,
                    actions: vec![
                        Action {
                            id: "copy".to_string(),
                            name: "复制翻译结果".to_string(),
                            icon: None,
                            is_default: true,
                            hotkey: None,
                            prevent_hide: false,
                        },
                    ],
                    preview: None,
                    refreshable: false,
                    group: Some("在线".to_string()),
                });
            }
            Err(e) => {
                tracing::warn!("Translation failed: {}", e);
            }
        }

        if results.is_empty() {
            results.push(QueryResult {
                id: "no_result".to_string(),
                plugin_id: self.metadata.id.clone(),
                title: "翻译失败".to_string(),
                subtitle: "请检查网络连接或稍后重试".to_string(),
                icon: WoxImage::emoji("⚠️".to_string()),
                score: 0,
                context_data: serde_json::Value::Null,
                actions: vec![],
                preview: None,
                refreshable: false,
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
                tracing::info!("Copied translation to clipboard: {}", result_id);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unknown action: {}", action_id)),
        }
    }
}
