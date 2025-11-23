// 插件系统

pub mod calculator;
pub mod app_search;
pub mod file_search;
pub mod web_search;
pub mod clipboard;
pub mod unit_converter;
pub mod settings;
pub mod browser;
pub mod process;
pub mod translator;
pub mod devtools;
pub mod git_projects;
pub mod system_commands;
pub mod execution_history;
pub mod window_manager;
pub mod sandbox;
pub mod audit;
pub mod ai_assistant;
pub mod plugin_installer; // 插件安装器
pub mod plugin_store;     // 插件商店

use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// 插件特征
#[async_trait]
pub trait Plugin: Send + Sync {
    /// 获取插件元数据
    fn metadata(&self) -> &PluginMetadata;
    
    /// 查询
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>>;
    
    /// 执行动作
    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()>;
}

/// 插件管理器
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    sandbox_manager: Arc<sandbox::SandboxManager>,
}

impl PluginManager {
    pub async fn new() -> Self {
        Self::new_with_mft_override(None).await
    }
    
    /// 创建插件管理器（可选覆盖 MFT 状态）
    pub async fn new_with_mft_override(mft_override: Option<bool>) -> Self {
        // 初始化沙盒管理器
        let sandbox_manager = Arc::new(sandbox::SandboxManager::new());
        
        // 🔒 配置插件沙盒权限
        Self::configure_sandbox_permissions(&sandbox_manager);
        
        // 加载插件配置（从存储管理器）
        let storage = match crate::storage::StorageManager::new() {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!("Failed to create storage manager for plugin config");
                let mut manager = Self { 
                    plugins: Vec::new(),
                    sandbox_manager,
                };
                Self::register_default_plugins(&mut manager).await;
                return manager;
            }
        };
        
        let file_search_config = storage.get_plugin_config("file_search").await.ok();
        let configured_use_mft = file_search_config
            .as_ref()
            .and_then(|cfg| cfg.get("use_mft"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // 默认启用
        
        // 🔥 如果有覆盖值，使用覆盖值；否则使用配置值
        let use_mft = mft_override.unwrap_or(configured_use_mft);
        
        // 🔥 如果覆盖值与配置值不同，记录日志
        if let Some(override_val) = mft_override {
            if override_val != configured_use_mft {
                tracing::info!("🔄 MFT mode overridden: config={}, actual={}", configured_use_mft, override_val);
            }
        }
        
        let mut manager = Self {
            plugins: Vec::new(),
            sandbox_manager,
        };
        
        // 注册插件
        manager.register(Box::new(calculator::CalculatorPlugin::new()));
        manager.register(Box::new(web_search::WebSearchPlugin::new()));
        manager.register(Box::new(unit_converter::UnitConverterPlugin::new()));
        manager.register(Box::new(settings::SettingsPlugin::new()));
        manager.register(Box::new(settings::PluginManagerPlugin::new()));
        manager.register(Box::new(system_commands::SystemCommandPlugin::new()));
        manager.register(Box::new(window_manager::WindowManagerPlugin::new()));
        
        // 创建运行历史插件
        let data_dir = crate::utils::paths::get_data_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let exec_history_path = data_dir.join("execution_history.json");
        manager.register(Box::new(execution_history::ExecutionHistoryPlugin::new(
            exec_history_path.to_string_lossy().to_string()
        )));
        
        let clipboard = clipboard::ClipboardPlugin::new();
        clipboard.init().await;
        manager.register(Box::new(clipboard));
        
        let app_search = app_search::AppSearchPlugin::new();
        app_search.init().await;
        manager.register(Box::new(app_search));
        
        let browser = browser::BrowserPlugin::new();
        browser.init().await;
        manager.register(Box::new(browser));
        
        manager.register(Box::new(process::ProcessPlugin::new()));
        manager.register(Box::new(translator::TranslatorPlugin::new()));
        manager.register(Box::new(devtools::DevToolsPlugin::new()));
        
        let git_projects = git_projects::GitProjectsPlugin::new();
        git_projects.init().await;
        manager.register(Box::new(git_projects));
        
        // 使用插件配置初始化文件搜索插件
        let file_search = file_search::FileSearchPlugin::new_with_config(use_mft);
        file_search.init().await;
        manager.register(Box::new(file_search));
        
        // AI 助手插件
        manager.register(Box::new(ai_assistant::AIAssistantPlugin::new()));
        
        manager
    }
    
    async fn register_default_plugins(manager: &mut Self) {
        manager.register(Box::new(calculator::CalculatorPlugin::new()));
        manager.register(Box::new(web_search::WebSearchPlugin::new()));
        manager.register(Box::new(unit_converter::UnitConverterPlugin::new()));
        manager.register(Box::new(settings::SettingsPlugin::new()));
        manager.register(Box::new(settings::PluginManagerPlugin::new()));
        manager.register(Box::new(system_commands::SystemCommandPlugin::new()));
        manager.register(Box::new(window_manager::WindowManagerPlugin::new()));
        
        // 创建运行历史插件
        let data_dir = crate::utils::paths::get_data_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let exec_history_path = data_dir.join("execution_history.json");
        manager.register(Box::new(execution_history::ExecutionHistoryPlugin::new(
            exec_history_path.to_string_lossy().to_string()
        )));
        
        let clipboard = clipboard::ClipboardPlugin::new();
        clipboard.init().await;
        manager.register(Box::new(clipboard));
        
        let app_search = app_search::AppSearchPlugin::new();
        app_search.init().await;
        manager.register(Box::new(app_search));
        
        let browser = browser::BrowserPlugin::new();
        browser.init().await;
        manager.register(Box::new(browser));
        
        manager.register(Box::new(process::ProcessPlugin::new()));
        manager.register(Box::new(translator::TranslatorPlugin::new()));
        manager.register(Box::new(devtools::DevToolsPlugin::new()));
        
        let git_projects = git_projects::GitProjectsPlugin::new();
        git_projects.init().await;
        manager.register(Box::new(git_projects));
        
        let file_search = file_search::FileSearchPlugin::new();
        file_search.init().await;
        manager.register(Box::new(file_search));
        
        // AI 助手插件
        manager.register(Box::new(ai_assistant::AIAssistantPlugin::new()));
    }
    
    /// 注册插件
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }
    
    /// 查询所有插件
    pub async fn query(&self, input: &str) -> Result<Vec<QueryResult>> {
        let ctx = QueryContext {
            query_type: QueryType::Input,
            trigger_keyword: String::new(),
            command: None,
            search: input.to_string(),
            raw_query: input.to_string(),
        };
        
        let mut file_search_results = Vec::new();
        let mut other_results = Vec::new();
        
        for plugin in &self.plugins {
            match plugin.query(&ctx).await {
                Ok(mut results) => {
                    // 🔹 将文件搜索和应用搜索结果分开存放
                    if plugin.metadata().id == "file_search" || plugin.metadata().id == "app_search" {
                        file_search_results.append(&mut results);
                    } else {
                        other_results.append(&mut results);
                    }
                }
                Err(e) => {
                    tracing::warn!("Plugin {} query failed: {}", plugin.metadata().name, e);
                }
            }
        }
        
        // 分别按分数排序
        file_search_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        other_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        
        // 文件搜索结果放前面，其他插件结果放后面
        let mut all_results = file_search_results;
        all_results.extend(other_results);
        
        Ok(all_results)
    }
    
    /// 执行动作
    pub async fn execute(&self, result_id: &str, action_id: &str, plugin_id: &str) -> Result<()> {
        tracing::info!("PluginManager::execute - plugin_id: {}, action_id: {}, result_id: {}", plugin_id, action_id, result_id);
        
        // 根据 plugin_id 查找对应的插件
        for plugin in &self.plugins {
            if plugin.metadata().id == plugin_id {
                tracing::info!("Found matching plugin: {}", plugin.metadata().name);
                return plugin.execute(result_id, action_id).await;
            }
        }
        
        Err(anyhow::anyhow!("Plugin '{}' not found", plugin_id))
    }
    
    /// 获取所有插件元数据
    pub fn get_plugins(&self) -> Vec<PluginMetadata> {
        self.plugins.iter().map(|p| p.metadata().clone()).collect()
    }
    
    /// 获取运行历史插件
    pub fn get_execution_history_plugin(&self) -> Option<&execution_history::ExecutionHistoryPlugin> {
        for plugin in &self.plugins {
            if plugin.metadata().id == "execution-history" {
                // 使用unsafe downcast - 我们知道这是ExecutionHistoryPlugin
                let ptr = plugin.as_ref() as *const dyn Plugin as *const execution_history::ExecutionHistoryPlugin;
                return unsafe { Some(&*ptr) };
            }
        }
        None
    }
    
    /// 获取 AI 助手插件
    pub fn get_ai_plugin(&self) -> Option<&ai_assistant::AIAssistantPlugin> {
        for plugin in &self.plugins {
            if plugin.metadata().id == "ai_assistant" {
                let ptr = plugin.as_ref() as *const dyn Plugin as *const ai_assistant::AIAssistantPlugin;
                return unsafe { Some(&*ptr) };
            }
        }
        None
    }
    
    /// 获取沙盒管理器
    pub fn sandbox_manager(&self) -> &Arc<sandbox::SandboxManager> {
        &self.sandbox_manager
    }
    
    /// 验证插件权限
    pub fn validate_permission(&self, plugin_id: &str, permission: &sandbox::PluginPermission) -> Result<()> {
        self.sandbox_manager.check_permission(plugin_id, permission)
    }
    
    /// 配置所有插件的沙盒权限
    fn configure_sandbox_permissions(sandbox_manager: &Arc<sandbox::SandboxManager>) {
        use sandbox::{SandboxConfig, PluginPermission, NetworkScope};
        use std::path::PathBuf;
        
        tracing::info!("🔒 Configuring plugin sandbox permissions...");
        
        // ===== 系统级插件 (完全信任) =====
        
        // 1. 文件搜索 - 需要全盘访问
        sandbox_manager.register(
            SandboxConfig::system("file_search")
        );
        
        // 2. 应用搜索 - 需要执行程序
        sandbox_manager.register(
            SandboxConfig::system("app_search")
        );
        
        // 3. 系统命令 - 需要系统级权限
        sandbox_manager.register(
            SandboxConfig::system("system_commands")
        );
        
        // 4. 进程管理器 - 需要进程管理权限
        sandbox_manager.register(
            SandboxConfig::system("process")
        );
        
        // 5. 窗口管理器 - 需要窗口管理权限
        sandbox_manager.register(
            SandboxConfig::system("window_manager")
        );
        
        // 6. 剪贴板历史 - 需要监控剪贴板
        sandbox_manager.register(
            SandboxConfig::system("clipboard")
        );
        
        // 7. 设置插件 - 需要修改配置
        sandbox_manager.register(
            SandboxConfig::system("settings")
        );
        
        // 8. 插件管理器 - 需要管理其他插件
        sandbox_manager.register(
            SandboxConfig::system("plugin_manager")
        );
        
        // 9. 执行历史 - 需要读写历史文件
        sandbox_manager.register(
            SandboxConfig::system("execution-history")
        );
        
        // ===== 受信任级插件 =====
        
        // 10. 浏览器数据搜索 - 需要读取浏览器配置目录
        let home_dir = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| String::from("."));
        
        sandbox_manager.register(
            SandboxConfig {
                plugin_id: "browser".to_string(),
                security_level: sandbox::SecurityLevel::Trusted,
                custom_permissions: Some(vec![
                    PluginPermission::FileSystemRead(PathBuf::from(&home_dir)),
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                ].into_iter().collect()),
                enabled: true,
                timeout_ms: Some(10000), // 10秒超时（数据库查询可能较慢）
                max_memory_mb: Some(200),
            }
        );
        
        // 11. Git 项目搜索 - 需要扫描项目目录
        sandbox_manager.register(
            SandboxConfig {
                plugin_id: "git_projects".to_string(),
                security_level: sandbox::SecurityLevel::Trusted,
                custom_permissions: Some(vec![
                    PluginPermission::FileSystemRead(PathBuf::from(&home_dir)),
                    PluginPermission::ExecuteProgram, // 打开 VSCode
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                ].into_iter().collect()),
                enabled: true,
                timeout_ms: Some(5000),
                max_memory_mb: Some(150),
            }
        );
        
        // ===== 受限级插件 (默认第三方插件级别) =====
        
        // 12. 翻译插件 - 需要网络访问
        sandbox_manager.register(
            SandboxConfig {
                plugin_id: "translator".to_string(),
                security_level: sandbox::SecurityLevel::Restricted,
                custom_permissions: Some(vec![
                    PluginPermission::NetworkAccess(NetworkScope::Domain("translate.google.com".to_string())),
                    PluginPermission::NetworkAccess(NetworkScope::Domain("translate.googleapis.com".to_string())),
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                ].into_iter().collect()),
                enabled: true,
                timeout_ms: Some(8000), // 网络请求可能较慢
                max_memory_mb: Some(100),
            }
        );
        
        // 13. 网页搜索 - 需要网络访问
        sandbox_manager.register(
            SandboxConfig {
                plugin_id: "web_search".to_string(),
                security_level: sandbox::SecurityLevel::Restricted,
                custom_permissions: Some(vec![
                    PluginPermission::NetworkAccess(NetworkScope::All), // 搜索多个引擎
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                ].into_iter().collect()),
                enabled: true,
                timeout_ms: Some(3000),
                max_memory_mb: Some(50),
            }
        );
        
        // ===== 沙盒级插件 (最小权限) =====
        
        // 14. 计算器 - 纯本地计算，无需额外权限
        sandbox_manager.register(
            SandboxConfig {
                plugin_id: "calculator".to_string(),
                security_level: sandbox::SecurityLevel::Sandboxed,
                custom_permissions: Some(vec![
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                ].into_iter().collect()),
                enabled: true,
                timeout_ms: Some(1000),
                max_memory_mb: Some(50),
            }
        );
        
        // 15. 单位转换 - 纯本地计算
        sandbox_manager.register(
            SandboxConfig {
                plugin_id: "unit_converter".to_string(),
                security_level: sandbox::SecurityLevel::Sandboxed,
                custom_permissions: Some(vec![
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                ].into_iter().collect()),
                enabled: true,
                timeout_ms: Some(1000),
                max_memory_mb: Some(50),
            }
        );
        
        // 16. 开发工具 - 本地工具（JSON、Base64、Hash等）
        sandbox_manager.register(
            SandboxConfig {
                plugin_id: "devtools".to_string(),
                security_level: sandbox::SecurityLevel::Sandboxed,
                custom_permissions: Some(vec![
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                ].into_iter().collect()),
                enabled: true,
                timeout_ms: Some(2000),
                max_memory_mb: Some(50),
            }
        );
        
        // 17. AI 助手 - 需要网络访问 AI API
        sandbox_manager.register(
            SandboxConfig {
                plugin_id: "ai_assistant".to_string(),
                security_level: sandbox::SecurityLevel::Restricted,
                custom_permissions: Some(vec![
                    PluginPermission::NetworkAccess(NetworkScope::Domain("api.openai.com".to_string())),
                    PluginPermission::NetworkAccess(NetworkScope::Domain("api.anthropic.com".to_string())),
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                ].into_iter().collect()),
                enabled: true,
                timeout_ms: Some(60000), // AI 响应可能需要更长时间
                max_memory_mb: Some(200),
            }
        );
        
        tracing::info!("✅ Configured sandbox permissions for {} plugins", 17);
    }
}
