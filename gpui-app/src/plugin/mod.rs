//! 插件框架：trait + 基础类型（对齐 src-tauri/src/plugin/mod.rs 与 core::types 语义）。
//! 无 gpui 依赖，可单元测试。
//!
//! 相对 Tauri 版的刻意偏离（在各处注释重复说明）：
//!   - trait 方法同步：GPUI 搜索跑在后台执行器的同步路径上，插件以内部可变性管理
//!     自身状态（Tauri 版 async_trait + tokio）
//!   - execute 返回 [`ExecuteOutcome`] 而非 `Result<()>`：副作用（打开 URL / 写剪贴板）
//!     上移到 Launcher 层真正执行，插件保持纯函数、无需 gpui 即可单测
//!   - 类型裁剪：PluginMetadata 去掉 WoxImage / commands / settings / supported_os /
//!     plugin_type（GPUI 版暂无消费方）；QueryContext 只保留 search（无 Selection 路径）

mod calculator;
mod installer;
mod manager;
mod sandbox;
mod store;
mod web_search;

pub use installer::{InstalledPlugin, PluginInstaller, PluginRegistry};
pub use manager::PluginManager;
pub use store::{PluginListItem, PluginStore, SearchParams};

use anyhow::Result;

/// 插件元数据（Tauri core::types::PluginMetadata 的常用子集）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    /// emoji 图标（Tauri 用 WoxImage 结构，GPUI 列表直接渲染 emoji 字符）
    pub icon: String,
    pub trigger_keywords: Vec<String>,
}

impl PluginMetadata {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            author: "iLauncher".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            icon: "🧩".to_string(),
            trigger_keywords: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn with_trigger_keywords(mut self, keywords: Vec<String>) -> Self {
        self.trigger_keywords = keywords;
        self
    }
}

/// 查询上下文（Tauri QueryContext 只保留 search；trigger_keyword/command 语义
/// 由各插件在自己的 query 实现内匹配，与 Tauri 版插件行为一致）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryContext {
    pub search: String,
}

impl QueryContext {
    pub fn new(search: impl Into<String>) -> Self {
        Self { search: search.into() }
    }
}

/// 结果动作（Tauri Action 的子集：去掉 icon / hotkey / prevent_hide，GPUI 版无消费方）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginAction {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

impl PluginAction {
    /// 默认动作（Enter 直接执行的那个）
    pub fn default_action(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), is_default: true }
    }
}

/// 查询结果（plugin_id 不在结果体内携带——由 PluginManager 在分发时盖章，
/// 与 search::Entry 的 origin 对应）
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: Option<String>,
    pub score: i32,
    pub actions: Vec<PluginAction>,
}

impl QueryResult {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: String::new(),
            icon: None,
            score: 0,
            actions: Vec::new(),
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn with_score(mut self, score: i32) -> Self {
        self.score = score;
        self
    }

    pub fn with_action(mut self, action: PluginAction) -> Self {
        self.actions.push(action);
        self
    }

    /// 默认动作 id（无显式默认时取第一个动作）
    pub fn default_action_id(&self) -> Option<&str> {
        self.actions
            .iter()
            .find(|a| a.is_default)
            .or_else(|| self.actions.first())
            .map(|a| a.id.as_str())
    }
}

/// 执行副作用的声明式描述：插件只声明"想做什么"，Launcher 层真正执行
/// （opener 打开 / 系统剪贴板写入）。Tauri 版 execute 直接做副作用（async），
/// GPUI 版上移到 UI 层，换取插件无 gpui 依赖、可单测
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteOutcome {
    /// 打开 URL 或文件路径
    Open(String),
    /// 复制文本到系统剪贴板
    Copy(String),
}

/// 插件特征（同步；见模块头注释）
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> &PluginMetadata;
    fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>>;
    fn execute(&self, result_id: &str, action_id: &str) -> Result<ExecuteOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_builder_defaults() {
        let m = PluginMetadata::new("calc", "Calculator");
        assert_eq!(m.id, "calc");
        assert_eq!(m.author, "iLauncher");
        assert_eq!(m.version, "1.0.0");
        let m = m.with_description("数学计算").with_icon("🧮");
        assert_eq!(m.description, "数学计算");
        assert_eq!(m.icon, "🧮");
    }

    #[test]
    fn query_result_default_action_fallback_to_first() {
        let r = QueryResult::new("1", "1+1").with_action(PluginAction {
            id: "copy".to_string(),
            name: "复制".to_string(),
            is_default: false,
        });
        // 无显式默认 → 回退第一个动作
        assert_eq!(r.default_action_id(), Some("copy"));
        let r = r.with_action(PluginAction::default_action("open", "打开"));
        assert_eq!(r.default_action_id(), Some("open"));
    }
}
