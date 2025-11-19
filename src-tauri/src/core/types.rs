// Rust 核心类型定义

use serde::{Deserialize, Serialize};

/// 查询上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryContext {
    pub query_type: QueryType,
    pub trigger_keyword: String,
    pub command: Option<String>,
    pub search: String,
    pub raw_query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryType {
    Input,
    Selection,
}

/// 查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: WoxImage,
    pub score: i32,
    pub plugin_id: String,
    pub context_data: serde_json::Value,
    pub actions: Vec<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<Preview>,
    pub refreshable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl QueryResult {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            subtitle: String::new(),
            icon: WoxImage::Emoji("🔍".to_string()),
            score: 0,
            plugin_id: String::new(),
            context_data: serde_json::Value::Null,
            actions: vec![],
            preview: None,
            refreshable: false,
            group: None,
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    pub fn with_icon(mut self, icon: WoxImage) -> Self {
        self.icon = icon;
        self
    }

    pub fn with_score(mut self, score: i32) -> Self {
        self.score = score;
        self
    }

    pub fn with_action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }
}

/// 操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<WoxImage>,
    pub is_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    pub prevent_hide: bool,
}

impl Action {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            icon: None,
            is_default: false,
            hotkey: None,
            prevent_hide: false,
        }
    }

    pub fn default(mut self) -> Self {
        self.is_default = true;
        self
    }

    pub fn prevent_hide(mut self) -> Self {
        self.prevent_hide = true;
        self
    }
}

/// 图标类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum WoxImage {
    Svg(String),
    File(String),
    Url(String),
    Base64(String),
    Emoji(String),
    SystemIcon(String),
}

impl WoxImage {
    pub fn emoji(emoji: impl Into<String>) -> Self {
        Self::Emoji(emoji.into())
    }

    pub fn file(path: impl Into<String>) -> Self {
        Self::File(path.into())
    }

    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }
}

/// 预览
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Preview {
    Text(String),
    Markdown(String),
    Image(String),
    Html(String),
    File(String),
}

/// 插件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub icon: WoxImage,
    pub trigger_keywords: Vec<String>,
    pub commands: Vec<Command>,
    pub settings: Vec<SettingDefinition>,
    pub supported_os: Vec<String>,
    pub plugin_type: PluginType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub command: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingDefinition {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginType {
    Native,
    Python,
    NodeJS,
    Script,
}
