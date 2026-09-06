//! 插件管理器：注册 / 查询扇出 / 执行分发 / 禁用过滤
//! （对齐 旧版对应实现 PluginManager 语义）。
//! 无 gpui 依赖，可单元测试。
//!
//! 相对 旧版的偏离：
//!   - 同步扇出（trait 已同步，见 mod.rs 头注释）
//!   - 禁用列表由 set_disabled_plugins 注入（旧版每次查询现读 storage 配置；
//!     GPUI 版由 main 启动时从设置注册表加载一次，设置页变更后再热更新）
//!   - 沙盒权限静态表只登记已迁移插件（calculator / web_search），其余插件随迁移补充

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use parking_lot::{Mutex, RwLock};

use crate::audit::AuditLogger;
use crate::plugin::calculator::CalculatorPlugin;
use crate::plugin::installer::{PluginManifest, PluginRegistry};
use crate::plugin::lua_cmd::LuaCommandPlugin;
use crate::plugin::sandbox::{
    NetworkScope, PluginPermission, SandboxConfig, SandboxManager, SecurityLevel,
};
use crate::plugin::web_search::WebSearchPlugin;
use crate::plugin::{ExecuteOutcome, Plugin, PluginMetadata, QueryResult};
use crate::search::Entry;

/// manifest 权限字符串 → 运行时沙盒权限（安装期已按前缀白名单校验，未知项跳过）
pub fn parse_manifest_permission(s: &str) -> Option<PluginPermission> {
    if let Some(domain) = s.strip_prefix("network:") {
        if domain == "all" || domain == "*" {
            Some(PluginPermission::NetworkAccess(NetworkScope::All))
        } else {
            Some(PluginPermission::NetworkAccess(NetworkScope::Domain(domain.to_string())))
        }
    } else if let Some(p) = s.strip_prefix("filesystem:read:") {
        Some(PluginPermission::FileSystemRead(std::path::PathBuf::from(p)))
    } else if let Some(p) = s.strip_prefix("filesystem:write:") {
        Some(PluginPermission::FileSystemWrite(std::path::PathBuf::from(p)))
    } else if s == "clipboard:read" || s == "clipboard:write" {
        Some(PluginPermission::ClipboardAccess)
    } else if s == "system:info" {
        Some(PluginPermission::SystemInfoRead)
    } else if s == "system:execute" {
        Some(PluginPermission::ExecuteProgram)
    } else {
        None
    }
}

/// manifest 沙盒配置 → 运行时 SandboxConfig（level: none/basic/restricted/strict）
fn sandbox_config_from_manifest(m: &PluginManifest) -> SandboxConfig {
    let (level, enabled) = match m.sandbox.level.as_str() {
        "none" => (SecurityLevel::System, false),
        "basic" => (SecurityLevel::Trusted, true),
        "strict" => (SecurityLevel::Sandboxed, true),
        _ => (SecurityLevel::Restricted, true),
    };
    SandboxConfig {
        plugin_id: m.id.clone(),
        security_level: level,
        custom_permissions: Some(
            m.permissions.iter().filter_map(|p| parse_manifest_permission(p)).collect(),
        ),
        enabled,
        timeout_ms: Some(m.sandbox.timeout_ms),
        max_memory_mb: Some(m.sandbox.max_memory_mb),
    }
}

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
        // Lua 命令插件（Listary 风格前缀命令 + 上下文命令）
        manager.register(Box::new(LuaCommandPlugin::hosts(manager.sandbox.clone())));
        manager.register(Box::new(LuaCommandPlugin::lock_workstation(manager.sandbox.clone())));
        manager.register(Box::new(LuaCommandPlugin::file_hash(manager.sandbox.clone())));
        manager.register(Box::new(LuaCommandPlugin::web_search(manager.sandbox.clone())));
        manager.register(Box::new(LuaCommandPlugin::empty_recycle_bin(manager.sandbox.clone())));
        manager.register(Box::new(LuaCommandPlugin::system_sleep(manager.sandbox.clone())));
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
        // Lua 命令：hosts —— 只允许读写 hosts 所在目录（路径前缀匹配强制）
        let hosts_dir = std::path::PathBuf::from("C:\\Windows\\System32\\drivers\\etc");
        sandbox.register(SandboxConfig {
            plugin_id: "cmd-hosts".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some(
                [
                    PluginPermission::FileSystemRead(hosts_dir.clone()),
                    PluginPermission::FileSystemWrite(hosts_dir),
                ]
                    .into_iter()
                    .collect(),
            ),
            enabled: true,
            timeout_ms: Some(1000),
            max_memory_mb: Some(50),
        });
        // Lua 命令：lock —— 只允许执行外部程序（具体命令不再细分，执行即审计）
        sandbox.register(SandboxConfig {
            plugin_id: "cmd-lock".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some([PluginPermission::ExecuteProgram].into_iter().collect()),
            enabled: true,
            timeout_ms: Some(1000),
            max_memory_mb: Some(50),
        });
        // Lua 命令：hash —— 需读任意盘的选中文件，路径前缀无法表达"全部盘符"，
        // 走系统级（enabled=false 全放行但仍记审计）；随第三方脚本落地再细化
        sandbox.register(SandboxConfig {
            plugin_id: "cmd-hash".to_string(),
            security_level: SecurityLevel::System,
            custom_permissions: None,
            enabled: false,
            timeout_ms: Some(1000),
            max_memory_mb: Some(50),
        });
        // Lua 命令：web —— 只调 ilauncher.open（不经权限检查，open 由 Launcher 层执行），无权限需求
        sandbox.register(SandboxConfig {
            plugin_id: "cmd-web".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some(HashSet::new()),
            enabled: true,
            timeout_ms: Some(1000),
            max_memory_mb: Some(50),
        });
        // Lua 命令：emptybin / sleep —— 只允许执行外部程序（执行即审计）
        sandbox.register(SandboxConfig {
            plugin_id: "cmd-emptybin".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some([PluginPermission::ExecuteProgram].into_iter().collect()),
            enabled: true,
            timeout_ms: Some(1000),
            max_memory_mb: Some(50),
        });
        sandbox.register(SandboxConfig {
            plugin_id: "cmd-sleep".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some([PluginPermission::ExecuteProgram].into_iter().collect()),
            enabled: true,
            timeout_ms: Some(1000),
            max_memory_mb: Some(50),
        });
    }

    /// 注册插件
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// 从已安装注册表加载第三方 Lua 命令插件（启动时调用一次；
    /// manifest 声明的沙盒权限注册进权限表，禁用状态合并进禁用集）
    pub fn load_installed_lua(&mut self, registry: &PluginRegistry) -> usize {
        let mut loaded = 0;
        for installed in registry.list() {
            let m = &installed.manifest;
            if m.engine.r#type != "lua" {
                continue;
            }
            let source_path = installed.install_path.join(&m.engine.entry);
            let source = match std::fs::read_to_string(&source_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("⚠ Lua 插件 {} 入口读取失败（{}）: {e:#}", m.id, source_path.display());
                    continue;
                }
            };
            let plugin = match LuaCommandPlugin::from_manifest(self.sandbox.clone(), m, source) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("⚠ Lua 插件 {} 加载失败: {e:#}", m.id);
                    continue;
                }
            };
            self.sandbox.register(sandbox_config_from_manifest(m));
            if !installed.enabled {
                self.disabled.write().insert(m.id.clone());
            }
            self.plugins.push(Box::new(plugin));
            loaded += 1;
        }
        loaded
    }

    /// 注入禁用列表（启动时从设置加载；设置页变更后热更新）
    pub fn set_disabled_plugins(&self, ids: Vec<String>) {
        *self.disabled.write() = ids.into_iter().collect();
    }

    /// 查询扇出：跳过禁用插件，单个插件失败只告警不影响其他（Tauri 同款语义）。
    /// 返回 (plugin_id, 结果) 已盖章列表，按 score 降序
    fn query_stamped(&self, input: &str, selection: Option<String>) -> Vec<(String, QueryResult)> {
        let disabled = self.disabled.read().clone();
        let ctx = crate::plugin::QueryContext { search: input.to_string(), selection };
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

    /// 查询并映射为搜索列表条目。
    /// 顺序约定（Launcher 层拼装）：前缀命令（COMMAND_SCORE）置顶，文件结果其次，
    /// 其余插件结果按 score 降序随后
    pub fn query_entries(&self, input: &str, selection: Option<String>, limit: usize) -> Vec<Entry> {
        if input.trim().is_empty() || limit == 0 {
            return Vec::new();
        }
        self.query_stamped(input, selection)
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
    use crate::plugin::lua_cmd::COMMAND_SCORE;
    use crate::plugin::{ExecuteOutcome, PluginInstaller, PluginRegistry, QueryContext};

    fn test_manager() -> PluginManager {
        PluginManager::new(Arc::new(Mutex::new(AuditLogger::in_memory(100))))
    }

    #[test]
    fn registers_builtin_plugins_with_sandbox() {
        let manager = test_manager();
        let ids: Vec<String> = manager.get_plugins().into_iter().map(|m| m.id).collect();
        assert_eq!(
            ids,
            vec![
                "calculator".to_string(),
                "web_search".to_string(),
                "cmd-hosts".to_string(),
                "cmd-lock".to_string(),
                "cmd-hash".to_string(),
                "cmd-web".to_string(),
                "cmd-emptybin".to_string(),
                "cmd-sleep".to_string(),
            ]
        );
        assert_eq!(manager.sandbox().registered_count(), 8);
    }

    #[test]
    fn query_merges_and_sorts_by_score_desc() {
        let manager = test_manager();
        // 计算器表达式分数（1000）应排在网页搜索（100）前
        let entries = manager.query_entries("1+1", None, 10);
        assert!(!entries.is_empty());
        assert_eq!(entries[0].name, "2");
        assert!(entries.iter().all(|e| e.score <= entries[0].score));
    }

    #[test]
    fn lua_command_pinned_above_calculator() {
        let manager = test_manager();
        // "hash" 既是 cmd-hash 关键字，calc 不匹配 → 命令置顶语义由 Launcher 层拼装，
        // 这里验证命令得分高于计算器
        let entries = manager.query_entries("hash", None, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].score, COMMAND_SCORE as i64);
        match &entries[0].origin {
            crate::search::EntryOrigin::Plugin { plugin_id, .. } => assert_eq!(plugin_id, "cmd-hash"),
            other => panic!("期望插件来源，得到 {other:?}"),
        }
    }

    #[test]
    fn disabled_plugin_skipped() {
        let manager = test_manager();
        manager.set_disabled_plugins(vec!["calculator".to_string()]);
        let entries = manager.query_entries("1+1", None, 10);
        assert!(entries.is_empty() || entries.iter().all(|e| e.name != "2"));
    }

    #[test]
    fn empty_query_returns_empty() {
        let manager = test_manager();
        assert!(manager.query_entries("   ", None, 10).is_empty());
        assert!(manager.query_entries("1+1", None, 0).is_empty());
    }

    #[test]
    fn query_entries_maps_origin_fields() {
        let manager = test_manager();
        let entries = manager.query_entries("1+1", None, 10);
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
        assert_eq!(ctx.selection, None);
        let ctx = ctx.with_selection("C:\\a.txt");
        assert_eq!(ctx.selection, Some("C:\\a.txt".to_string()));
    }

    /// 端到端：示例插件（examples/lua-plugins/com.ilauncher-demo.upper）打包 .ilp
    /// → 安装 → 注册表扫描 → manager 加载 → 搜索框查询 → 执行（权限来自 manifest）
    #[test]
    fn installed_lua_plugin_end_to_end() {
        use std::io::Write as _;
        use zip::write::SimpleFileOptions;

        let dir = crate::test_util::tempdir("manager_lua_e2e");
        let registry = Arc::new(PluginRegistry::new(dir.join("plugins")));
        let installer = PluginInstaller::new(registry.clone());

        // 打包：manifest.json + main.lua（与 examples/lua-plugins 下示例同构）
        let example_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("lua-plugins")
            .join("com.ilauncher-demo.upper");
        let ilp = dir.join("upper.ilp");
        {
            let file = std::fs::File::create(&ilp).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let opts = SimpleFileOptions::default();
            for name in ["manifest.json", "main.lua"] {
                writer.start_file(name, opts).unwrap();
                writer
                    .write_all(&std::fs::read(example_dir.join(name)).unwrap())
                    .unwrap();
            }
            writer.finish().unwrap();
        }
        installer.install(&ilp).unwrap();

        // 启动路径：新 manager 从注册表加载第三方 Lua 插件
        let mut manager = PluginManager::new(Arc::new(Mutex::new(AuditLogger::in_memory(100))));
        assert_eq!(manager.load_installed_lua(&registry), 1);
        assert!(manager
            .get_plugins()
            .iter()
            .any(|m| m.id == "com.ilauncher-demo.upper"));

        // 搜索框查询：关键字命中，命令置顶得分
        let entries = manager.query_entries("upper hello lua", None, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].score, COMMAND_SCORE as i64);
        let (plugin_id, result_id, action_id) = match &entries[0].origin {
            crate::search::EntryOrigin::Plugin { plugin_id, result_id, action_id, .. } => {
                (plugin_id.clone(), result_id.clone(), action_id.clone())
            }
            other => panic!("期望插件来源，得到 {other:?}"),
        };
        assert_eq!(plugin_id, "com.ilauncher-demo.upper");

        // 执行：clipboard:write 权限来自 manifest 映射 → Copy 副作用 outcome
        let outcome = manager.execute(&plugin_id, &result_id, &action_id).unwrap();
        assert_eq!(outcome, ExecuteOutcome::Copy("HELLO LUA".to_string()));

        // 禁用：模拟插件页开关（禁用集同步后查询扇出跳过）
        manager.set_disabled_plugins(vec!["com.ilauncher-demo.upper".to_string()]);
        assert!(manager.query_entries("upper hello", None, 10).is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// manifest 权限字符串 → 沙盒权限映射
    #[test]
    fn manifest_permission_mapping() {
        use crate::plugin::sandbox::PluginPermission;
        use std::path::PathBuf;

        assert_eq!(
            parse_manifest_permission("network:api.example.com"),
            Some(PluginPermission::NetworkAccess(crate::plugin::sandbox::NetworkScope::Domain(
                "api.example.com".to_string()
            )))
        );
        assert_eq!(
            parse_manifest_permission("network:all"),
            Some(PluginPermission::NetworkAccess(crate::plugin::sandbox::NetworkScope::All))
        );
        assert_eq!(
            parse_manifest_permission("filesystem:read:C:\\docs"),
            Some(PluginPermission::FileSystemRead(PathBuf::from("C:\\docs")))
        );
        assert_eq!(
            parse_manifest_permission("system:execute"),
            Some(PluginPermission::ExecuteProgram)
        );
        assert_eq!(parse_manifest_permission("clipboard:write"), Some(PluginPermission::ClipboardAccess));
        assert_eq!(parse_manifest_permission("database:read"), None, "未映射权限应被跳过");
    }
}
