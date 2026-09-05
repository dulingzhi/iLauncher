//! 插件沙盒：权限模型 + 资源访问控制（对齐 src-tauri/src/plugin/sandbox.rs 语义）。
//! 无 gpui 依赖，可单元测试。
//!
//! 相对 Tauri 版的刻意偏离：
//!   - 审计 logger 由构造方注入共享实例：Tauri 版 SandboxManager 自持独立 logger
//!     （事件写进黑洞，生产从未读取）；GPUI 版事件直接落全局审计管道，查看器实时可见
//!   - 不迁移 SandboxedExecution 超时包装器：Tauri 版全程 dead_code，生产无使用方
//!   - 不迁移审计条目 getter：全局 AuditLogger 已暴露同款接口，迁移即重复
//!   - 同步 parking_lot 锁（GPUI 版无 tokio 运行时依赖）

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use crate::audit::{AuditEventType, AuditLogger, AuditSeverity};

/// 插件权限类型（与 Tauri 版一一对应）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PluginPermission {
    /// 文件系统读取（指定目录）
    FileSystemRead(PathBuf),
    /// 文件系统写入（指定目录）
    FileSystemWrite(PathBuf),
    /// 网络访问（指定域名或全部）
    NetworkAccess(NetworkScope),
    /// 执行外部程序
    ExecuteProgram,
    /// 剪贴板访问
    ClipboardAccess,
    /// 系统信息读取
    SystemInfoRead,
    /// 进程管理
    ProcessManagement,
    /// 窗口管理
    WindowManagement,
    /// 注册表访问（Windows）
    RegistryAccess,
    /// 环境变量访问
    EnvironmentAccess,
}

/// 网络访问范围
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NetworkScope {
    /// 无网络访问
    None,
    /// 特定域名
    Domain(String),
    /// 全部网络访问
    All,
}

/// 插件安全级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecurityLevel {
    /// 系统级（完全信任，内置插件）
    System,
    /// 信任级（经过验证的第三方插件）
    Trusted,
    /// 受限级（未验证的第三方插件）
    Restricted,
    /// 沙盒级（完全隔离，最小权限）
    Sandboxed,
}

impl SecurityLevel {
    /// 获取默认权限集（与 Tauri 版逐条一致）
    pub fn default_permissions(&self) -> HashSet<PluginPermission> {
        match self {
            SecurityLevel::System => [
                PluginPermission::FileSystemRead(PathBuf::from("/")),
                PluginPermission::FileSystemWrite(PathBuf::from("/")),
                PluginPermission::NetworkAccess(NetworkScope::All),
                PluginPermission::ExecuteProgram,
                PluginPermission::ClipboardAccess,
                PluginPermission::SystemInfoRead,
                PluginPermission::ProcessManagement,
                PluginPermission::WindowManagement,
                PluginPermission::RegistryAccess,
                PluginPermission::EnvironmentAccess,
            ]
            .into_iter()
            .collect(),
            SecurityLevel::Trusted => [
                PluginPermission::FileSystemRead(PathBuf::from("/")),
                PluginPermission::NetworkAccess(NetworkScope::All),
                PluginPermission::ClipboardAccess,
                PluginPermission::SystemInfoRead,
                PluginPermission::ExecuteProgram,
            ]
            .into_iter()
            .collect(),
            SecurityLevel::Restricted => {
                [PluginPermission::SystemInfoRead, PluginPermission::ClipboardAccess]
                    .into_iter()
                    .collect()
            }
            SecurityLevel::Sandboxed => [PluginPermission::SystemInfoRead].into_iter().collect(),
        }
    }
}

/// 插件沙盒配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub plugin_id: String,
    pub security_level: SecurityLevel,
    /// 自定义权限（覆盖默认权限）
    pub custom_permissions: Option<HashSet<PluginPermission>>,
    /// 是否启用沙盒
    pub enabled: bool,
    /// 超时限制（毫秒）
    pub timeout_ms: Option<u64>,
    /// 最大内存使用（MB）
    pub max_memory_mb: Option<u64>,
}

impl SandboxConfig {
    /// 获取有效权限集
    pub fn effective_permissions(&self) -> HashSet<PluginPermission> {
        if let Some(ref custom) = self.custom_permissions {
            custom.clone()
        } else {
            self.security_level.default_permissions()
        }
    }
}

/// 插件沙盒管理器
pub struct SandboxManager {
    configs: RwLock<HashMap<String, SandboxConfig>>,
    /// 共享审计 logger：每次权限检查写入事件（Tauri 版写自持 logger 成黑洞，此处纠正）
    audit_logger: Arc<Mutex<AuditLogger>>,
}

impl SandboxManager {
    /// 注入共享审计 logger（事件实时落全局管道，审计查看器可见）
    pub fn new(audit_logger: Arc<Mutex<AuditLogger>>) -> Self {
        Self { configs: RwLock::new(HashMap::new()), audit_logger }
    }

    /// 注册插件沙盒配置
    pub fn register(&self, config: SandboxConfig) {
        self.configs.write().insert(config.plugin_id.clone(), config);
    }

    /// 检查权限（检查即审计：允许 Info / 拒绝 Warning，语义与 Tauri 版一致）
    pub fn check_permission(&self, plugin_id: &str, permission: &PluginPermission) -> Result<()> {
        let configs = self.configs.read();
        let config = configs
            .get(plugin_id)
            .ok_or_else(|| anyhow!("Plugin '{}' not registered in sandbox", plugin_id))?;

        // 如果沙盒未启用（系统插件），直接允许（仍记审计）
        if !config.enabled {
            self.audit_logger.lock().log(
                AuditEventType::PermissionCheck {
                    plugin_id: plugin_id.to_string(),
                    permission: format!("{:?}", permission),
                    allowed: true,
                },
                AuditSeverity::Info,
            );
            return Ok(());
        }
        drop(configs);

        let effective_perms = {
            let configs = self.configs.read();
            configs.get(plugin_id).expect("checked above").effective_permissions()
        };

        let mut allowed = false;
        match permission {
            PluginPermission::FileSystemRead(path) | PluginPermission::FileSystemWrite(path) => {
                // 路径前缀匹配（Tauri 同款语义）
                for perm in &effective_perms {
                    match perm {
                        PluginPermission::FileSystemRead(allowed_path)
                        | PluginPermission::FileSystemWrite(allowed_path)
                            if path.starts_with(allowed_path) => {
                                allowed = true;
                                break;
                            }
                        _ => {}
                    }
                }
                self.audit_logger.lock().log(
                    AuditEventType::FileAccess {
                        plugin_id: plugin_id.to_string(),
                        path: path.display().to_string(),
                        write: matches!(permission, PluginPermission::FileSystemWrite(_)),
                        allowed,
                    },
                    if allowed { AuditSeverity::Info } else { AuditSeverity::Warning },
                );
            }
            PluginPermission::NetworkAccess(scope) => {
                for perm in &effective_perms {
                    if let PluginPermission::NetworkAccess(allowed_scope) = perm {
                        match (scope, allowed_scope) {
                            (_, NetworkScope::All) => {
                                allowed = true;
                                break;
                            }
                            (NetworkScope::Domain(domain), NetworkScope::Domain(allowed_domain))
                                if domain == allowed_domain => {
                                    allowed = true;
                                    break;
                                }
                            _ => {}
                        }
                    }
                }
                let domain = match scope {
                    NetworkScope::All => "all".to_string(),
                    NetworkScope::Domain(d) => d.clone(),
                    NetworkScope::None => "none".to_string(),
                };
                self.audit_logger.lock().log(
                    AuditEventType::NetworkAccess { plugin_id: plugin_id.to_string(), domain, allowed },
                    if allowed { AuditSeverity::Info } else { AuditSeverity::Warning },
                );
            }
            _ => {
                allowed = effective_perms.contains(permission);
                self.audit_logger.lock().log(
                    AuditEventType::PermissionCheck {
                        plugin_id: plugin_id.to_string(),
                        permission: format!("{:?}", permission),
                        allowed,
                    },
                    if allowed { AuditSeverity::Info } else { AuditSeverity::Warning },
                );
            }
        }

        if !allowed {
            return Err(anyhow!("Permission denied: {:?} for plugin '{}'", permission, plugin_id));
        }
        Ok(())
    }

    /// 验证网络访问
    pub fn validate_network_access(&self, plugin_id: &str, domain: &str) -> Result<()> {
        self.check_permission(
            plugin_id,
            &PluginPermission::NetworkAccess(NetworkScope::Domain(domain.to_string())),
        )
    }

    /// 已注册插件数
    pub fn registered_count(&self) -> usize {
        self.configs.read().len()
    }
}

impl Default for SandboxManager {
    /// 独立内存审计的默认实例（测试/工具场景；生产应走 new() 注入共享 logger）
    fn default() -> Self {
        Self::new(Arc::new(Mutex::new(AuditLogger::in_memory(1000))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 受限沙盒 + 指定权限集的测试配置（builder 已按"无生产使用方即删"原则移除）
    fn restricted_with(plugin_id: &str, perms: Vec<PluginPermission>) -> SandboxConfig {
        SandboxConfig {
            plugin_id: plugin_id.to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some(perms.into_iter().collect()),
            enabled: true,
            timeout_ms: None,
            max_memory_mb: None,
        }
    }

    fn logger_with_entries() -> (Arc<Mutex<AuditLogger>>, Arc<SandboxManager>) {
        let logger = Arc::new(Mutex::new(AuditLogger::in_memory(100)));
        let manager = Arc::new(SandboxManager::new(logger.clone()));
        (logger, manager)
    }

    #[test]
    fn security_level_default_permissions() {
        let system_perms = SecurityLevel::System.default_permissions();
        assert!(system_perms.contains(&PluginPermission::ExecuteProgram));

        let sandboxed_perms = SecurityLevel::Sandboxed.default_permissions();
        assert!(!sandboxed_perms.contains(&PluginPermission::ExecuteProgram));
        assert!(sandboxed_perms.contains(&PluginPermission::SystemInfoRead));
    }

    #[test]
    fn permission_check_allowed_and_denied() {
        let (_, manager) = logger_with_entries();
        manager.register(restricted_with(
            "test_plugin",
            vec![PluginPermission::ExecuteProgram],
        ));

        assert!(manager.check_permission("test_plugin", &PluginPermission::ExecuteProgram).is_ok());
        assert!(manager.check_permission("test_plugin", &PluginPermission::ProcessManagement).is_err());
    }

    #[test]
    fn permission_check_unregistered_errors() {
        let (_, manager) = logger_with_entries();
        assert!(manager.check_permission("ghost", &PluginPermission::SystemInfoRead).is_err());
    }

    #[test]
    fn system_level_plugin_bypasses_but_audits() {
        let (logger, manager) = logger_with_entries();
        manager.register(SandboxConfig {
            plugin_id: "core".to_string(),
            security_level: SecurityLevel::System,
            custom_permissions: None,
            enabled: false, // 系统插件不走沙盒
            timeout_ms: None,
            max_memory_mb: None,
        });
        // 未启用沙盒：任何权限都放行
        assert!(manager.check_permission("core", &PluginPermission::ExecuteProgram).is_ok());
        // 但仍写审计（黑洞纠正的验证点）
        assert_eq!(logger.lock().len(), 1);
    }

    #[test]
    fn file_access_path_prefix() {
        let (_, manager) = logger_with_entries();
        let home = PathBuf::from("C:\\Users\\test");
        manager.register(restricted_with(
            "fs_plugin",
            vec![PluginPermission::FileSystemRead(home.clone())],
        ));
        assert!(manager
            .check_permission("fs_plugin", &PluginPermission::FileSystemRead(home.join("docs\\a.txt")))
            .is_ok());
        assert!(manager
            .check_permission("fs_plugin", &PluginPermission::FileSystemRead(PathBuf::from("D:\\other")))
            .is_err());
    }

    #[test]
    fn network_access_domain_matching() {
        let (_, manager) = logger_with_entries();
        manager.register(restricted_with(
            "net_plugin",
            vec![PluginPermission::NetworkAccess(NetworkScope::Domain("api.example.com".into()))],
        ));
        assert!(manager.validate_network_access("net_plugin", "api.example.com").is_ok());
        assert!(manager.validate_network_access("net_plugin", "evil.com").is_err());
    }

    #[test]
    fn every_check_writes_audit_event() {
        let (logger, manager) = logger_with_entries();
        manager.register(restricted_with("audited", vec![PluginPermission::SystemInfoRead]));
        // 1 允许 + 2 拒绝 = 3 条事件，severity 随结果
        let _ = manager.check_permission("audited", &PluginPermission::SystemInfoRead);
        let _ = manager.check_permission("audited", &PluginPermission::ExecuteProgram);
        let _ = manager.validate_network_access("audited", "x.com");
        let logger = logger.lock();
        assert_eq!(logger.len(), 3);
        assert_eq!(logger.entries()[0].severity, AuditSeverity::Info);
        assert_eq!(logger.entries()[1].severity, AuditSeverity::Warning);
        assert_eq!(logger.entries()[2].severity, AuditSeverity::Warning);
    }
}
