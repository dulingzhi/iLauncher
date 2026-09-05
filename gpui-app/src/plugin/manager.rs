//! 插件管理器：注册 / 查询扇出 / 执行分发 / 禁用过滤
//! （对齐 src-tauri/src/plugin/mod.rs PluginManager 语义）。
//! 无 gpui 依赖，可单元测试。
//!
//! 相对 Tauri 版的偏离：
//!   - 同步扇出（trait 已同步，见 mod.rs 头注释）
//!   - 禁用列表由 set_disabled_plugins 注入（Tauri 版每次查询现读 storage 配置；
//!     GPUI 版由 main 启动时从设置注册表加载一次，设置页变更后再热更新）
//!   - 沙盒权限静态表只登记已迁移插件（calculator / web_search），其余插件随迁移补充

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use parking_lot::{Mutex, RwLock};

use crate::audit::AuditLogger;
use crate::plugin::calculator::CalculatorPlugin;
use crate::plugin::sandbox::{
    NetworkScope, PluginPermission, SandboxConfig, SandboxManager, SecurityLevel,
};
use crate::plugin::web_search::WebSearchPlugin;
use crate::plugin::{ExecuteOutcome, Plugin, PluginMetadata, QueryResult};
use crate::search::Entry;

/// 插件管理器
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    sandbox: Arc<SandboxManager>,
    disabled: RwLock<HashSet<String>>,
}

impl PluginManager {
    /// 创建并注册内置插件 + 沙盒权限表（audit_logger 注入沙盒审计管道）
    pub fn new(audit_logger: Arc<Mutex<AuditLogger>>) -> Self {
        let sandbox = Arc::new(SandboxManager::new(audit_logger));
        Self::register_sandbox_configs(&sandbox);

        let mut manager = Self { plugins: Vec::new(), sandbox, disabled: RwLock::new(HashSet::new()) };
        manager.register(Box::new(CalculatorPlugin::new(manager.sandbox.clone())));
        manager.register(Box::new(WebSearchPlugin::new(manager.sandbox.clone())));
        manager
    }

    /// 内置插件沙盒权限表（对齐 Tauri configure_sandbox_permissions 中对应条目）
    fn register_sandbox_configs(sandbox: &Arc<SandboxManager>) {
        // 计算器：纯本地计算，沙盒级 + 剪贴板（复制结果）
        sandbox.register(SandboxConfig {
            plugin_id: "calculator".to_string(),
            security_level: SecurityLevel::Sandboxed,
            custom_permissions: Some(
                [PluginPermission::ClipboardAccess, PluginPermission::SystemInfoRead]
                    .into_iter()
                    .collect(),
            ),
            enabled: true,
            timeout_ms: Some(1000),
            max_memory_mb: Some(50),
        });
        // 网页搜索：受限级 + 全网访问（多引擎）
        sandbox.register(SandboxConfig {
            plugin_id: "web_search".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some(
                [
                    PluginPermission::NetworkAccess(NetworkScope::All),
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                ]
                .into_iter()
                .collect(),
            ),
            enabled: true,
            timeout_ms: Some(3000),
            max_memory_mb: Some(50),
        });
    }

    /// 注册插件
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// 注入禁用列表（启动时从设置加载；设置页变更后热更新）
    pub fn set_disabled_plugins(&self, ids: Vec<String>) {
        *self.disabled.write() = ids.into_iter().collect();
    }

    /// 查询扇出：跳过禁用插件，单个插件失败只告警不影响其他（Tauri 同款语义）。
    /// 返回 (plugin_id, 结果) 已盖章列表，按 score 降序
    fn query_stamped(&self, input: &str) -> Vec<(String, QueryResult)> {
        let disabled = self.disabled.read().clone();
        let ctx = crate::plugin::QueryContext::new(input);
        let mut results = Vec::new();
        for plugin in &self.plugins {
            let plugin_id = plugin.metadata().id.clone();
            if disabled.contains(&plugin_id) {
                continue;
            }
            match plugin.query(&ctx) {
                Ok(rs) => results.extend(rs.into_iter().map(|r| (plugin_id.clone(), r))),
                Err(e) => eprintln!("⚠ 插件 {} 查询失败: {e:#}", plugin.metadata().name),
            }
        }
        results.sort_by_key(|(_, r)| -r.score);
        results
    }

    /// 查询并映射为搜索列表条目（文件结果在前、插件结果在后的顺序由 Launcher 保证）
    pub fn query_entries(&self, input: &str, limit: usize) -> Vec<Entry> {
        if input.trim().is_empty() || limit == 0 {
            return Vec::new();
        }
        self.query_stamped(input)
            .into_iter()
            .take(limit)
            .filter_map(|(plugin_id, qr)| {
                let action_id = qr.default_action_id()?.to_string();
                Some(Entry::plugin_result(
                    qr.title,
                    qr.subtitle,
                    qr.score as i64,
                    qr.icon.unwrap_or_default(),
                    action_id,
                    qr.id,
                    plugin_id,
                ))
            })
            .collect()
    }

    /// 执行动作：按 plugin_id 分发（Tauri 同款；找不到插件报错）
    pub fn execute(&self, plugin_id: &str, result_id: &str, action_id: &str) -> Result<ExecuteOutcome> {
        for plugin in &self.plugins {
            if plugin.metadata().id == plugin_id {
                return plugin.execute(result_id, action_id);
            }
        }
        Err(anyhow!("Plugin '{}' not found", plugin_id))
    }

    /// 获取所有插件元数据
    pub fn get_plugins(&self) -> Vec<PluginMetadata> {
        self.plugins.iter().map(|p| p.metadata().clone()).collect()
    }

    /// 沙盒管理器（设置页展示权限用）
    pub fn sandbox(&self) -> Arc<SandboxManager> {
        self.sandbox.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{ExecuteOutcome, QueryContext};

    fn test_manager() -> PluginManager {
        PluginManager::new(Arc::new(Mutex::new(AuditLogger::in_memory(100))))
    }

    #[test]
    fn registers_builtin_plugins_with_sandbox() {
        let manager = test_manager();
        let ids: Vec<String> = manager.get_plugins().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["calculator".to_string(), "web_search".to_string()]);
        assert_eq!(manager.sandbox().registered_count(), 2);
    }

    #[test]
    fn query_merges_and_sorts_by_score_desc() {
        let manager = test_manager();
        // 计算器表达式分数（1000）应排在网页搜索（100）前
        let entries = manager.query_entries("1+1", 10);
        assert!(!entries.is_empty());
        assert_eq!(entries[0].name, "2");
        assert!(entries.iter().all(|e| e.score <= entries[0].score));
    }

    #[test]
    fn disabled_plugin_skipped() {
        let manager = test_manager();
        manager.set_disabled_plugins(vec!["calculator".to_string()]);
        let entries = manager.query_entries("1+1", 10);
        assert!(entries.is_empty() || entries.iter().all(|e| e.name != "2"));
    }

    #[test]
    fn empty_query_returns_empty() {
        let manager = test_manager();
        assert!(manager.query_entries("   ", 10).is_empty());
        assert!(manager.query_entries("1+1", 0).is_empty());
    }

    #[test]
    fn query_entries_maps_origin_fields() {
        let manager = test_manager();
        let entries = manager.query_entries("1+1", 10);
        assert_eq!(entries.len(), 1);
        match &entries[0].origin {
            crate::search::EntryOrigin::Plugin { plugin_id, result_id, action_id, icon } => {
                assert_eq!(plugin_id, "calculator");
                assert_eq!(result_id, "2");
                assert_eq!(action_id, "copy");
                assert_eq!(icon, &Some("🧮".to_string()));
            }
            other => panic!("期望插件来源，得到 {other:?}"),
        }
    }

    #[test]
    fn execute_dispatches_to_plugin() {
        let manager = test_manager();
        let outcome = manager.execute("calculator", "42", "copy").unwrap();
        assert_eq!(outcome, ExecuteOutcome::Copy("42".to_string()));
        assert!(manager.execute("ghost", "x", "copy").is_err());
        // calculator 不认识 open 动作
        assert!(manager.execute("calculator", "42", "open").is_err());
    }

    #[test]
    fn execute_denied_permission_propagates_error() {
        // 权限拒绝路径：calculator 无网络权限 → 域名校验拒绝
        let manager = test_manager();
        assert!(manager
            .sandbox()
            .validate_network_access("calculator", "evil.com")
            .is_err());
    }

    #[test]
    fn query_context_roundtrip() {
        let ctx = QueryContext::new("g rust");
        assert_eq!(ctx.search, "g rust");
    }
}
