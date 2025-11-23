// 插件沙盒隔离系统
// 提供权限管理、资源访问控制、执行环境隔离

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use super::audit::{AuditLogger, AuditEventType, AuditSeverity};

/// 插件权限类型
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
    /// 获取默认权限集
    pub fn default_permissions(&self) -> HashSet<PluginPermission> {
        match self {
            SecurityLevel::System => {
                // 系统插件拥有所有权限
                vec![
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
                .collect()
            }
            SecurityLevel::Trusted => {
                // 信任插件有较多权限，但限制敏感操作
                vec![
                    PluginPermission::FileSystemRead(PathBuf::from("/")),
                    PluginPermission::NetworkAccess(NetworkScope::All),
                    PluginPermission::ClipboardAccess,
                    PluginPermission::SystemInfoRead,
                    PluginPermission::ExecuteProgram,
                ]
                .into_iter()
                .collect()
            }
            SecurityLevel::Restricted => {
                // 受限插件只能访问基本功能
                vec![
                    PluginPermission::SystemInfoRead,
                    PluginPermission::ClipboardAccess,
                ]
                .into_iter()
                .collect()
            }
            SecurityLevel::Sandboxed => {
                // 沙盒插件最小权限
                vec![PluginPermission::SystemInfoRead]
                    .into_iter()
                    .collect()
            }
        }
    }
}

/// 插件沙盒配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// 插件 ID
    pub plugin_id: String,
    /// 安全级别
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
    /// 创建系统级配置（内置插件）
    pub fn system(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            security_level: SecurityLevel::System,
            custom_permissions: None,
            enabled: false, // 系统插件不需要沙盒
            timeout_ms: None,
            max_memory_mb: None,
        }
    }

    /// 创建受限级配置（默认）
    pub fn restricted(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: None,
            enabled: true,
            timeout_ms: Some(5000), // 5秒超时
            max_memory_mb: Some(100), // 100MB 内存限制
        }
    }

    /// 获取有效权限集
    pub fn effective_permissions(&self) -> HashSet<PluginPermission> {
        if let Some(ref custom) = self.custom_permissions {
            custom.clone()
        } else {
            self.security_level.default_permissions()
        }
    }

    /// 添加权限
    pub fn with_permission(mut self, permission: PluginPermission) -> Self {
        let mut perms = self.effective_permissions();
        perms.insert(permission);
        self.custom_permissions = Some(perms);
        self
    }
}

/// 插件沙盒管理器
pub struct SandboxManager {
    configs: Arc<RwLock<std::collections::HashMap<String, SandboxConfig>>>,
    audit_logger: Arc<AuditLogger>,
}

impl SandboxManager {
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(std::collections::HashMap::new())),
            audit_logger: Arc::new(AuditLogger::default()),
        }
    }

    /// 注册插件沙盒配置
    pub fn register(&self, config: SandboxConfig) {
        let plugin_id = config.plugin_id.clone();
        let mut configs = self.configs.write().unwrap();
        configs.insert(plugin_id.clone(), config);
        tracing::info!("🔒 Sandbox registered for plugin: {}", plugin_id);
    }

    /// 检查权限
    pub fn check_permission(&self, plugin_id: &str, permission: &PluginPermission) -> Result<()> {
        let configs = self.configs.read().unwrap();
        
        let config = configs.get(plugin_id)
            .ok_or_else(|| anyhow!("Plugin '{}' not registered in sandbox", plugin_id))?;

        // 如果沙盒未启用（系统插件），直接允许
        if !config.enabled {
            // 记录审计日志
            self.audit_logger.log(
                AuditEventType::PermissionCheck {
                    plugin_id: plugin_id.to_string(),
                    permission: format!("{:?}", permission),
                    allowed: true,
                },
                AuditSeverity::Info,
            );
            return Ok(());
        }

        let effective_perms = config.effective_permissions();
        let mut allowed = false;

        // 检查权限
        match permission {
            PluginPermission::FileSystemRead(path) | PluginPermission::FileSystemWrite(path) => {
                // 检查是否有对应权限，并且路径在允许范围内
                for perm in &effective_perms {
                    match perm {
                        PluginPermission::FileSystemRead(allowed_path) 
                        | PluginPermission::FileSystemWrite(allowed_path) => {
                            if path.starts_with(allowed_path) {
                                allowed = true;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                
                // 记录文件访问审计
                self.audit_logger.log(
                    AuditEventType::FileAccess {
                        plugin_id: plugin_id.to_string(),
                        path: path.display().to_string(),
                        write: matches!(permission, PluginPermission::FileSystemWrite(_)),
                        allowed,
                    },
                    if allowed { AuditSeverity::Info } else { AuditSeverity::Warning },
                );
                
                if !allowed {
                    return Err(anyhow!("Permission denied: {:?} for plugin '{}'", permission, plugin_id));
                }
            }
            PluginPermission::NetworkAccess(scope) => {
                for perm in &effective_perms {
                    if let PluginPermission::NetworkAccess(allowed_scope) = perm {
                        match (scope, allowed_scope) {
                            (_, NetworkScope::All) => {
                                allowed = true;
                                break;
                            }
                            (NetworkScope::Domain(domain), NetworkScope::Domain(allowed_domain)) => {
                                if domain == allowed_domain {
                                    allowed = true;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                
                // 记录网络访问审计
                let domain = match scope {
                    NetworkScope::All => "all".to_string(),
                    NetworkScope::Domain(d) => d.clone(),
                    NetworkScope::None => "none".to_string(),
                };
                self.audit_logger.log(
                    AuditEventType::NetworkAccess {
                        plugin_id: plugin_id.to_string(),
                        domain,
                        allowed,
                    },
                    if allowed { AuditSeverity::Info } else { AuditSeverity::Warning },
                );
                
                if !allowed {
                    return Err(anyhow!("Permission denied: {:?} for plugin '{}'", permission, plugin_id));
                }
            }
            _ => {
                allowed = effective_perms.contains(permission);
                
                // 记录权限检查
                self.audit_logger.log(
                    AuditEventType::PermissionCheck {
                        plugin_id: plugin_id.to_string(),
                        permission: format!("{:?}", permission),
                        allowed,
                    },
                    if allowed { AuditSeverity::Info } else { AuditSeverity::Warning },
                );
                
                if !allowed {
                    return Err(anyhow!("Permission denied: {:?} for plugin '{}'", permission, plugin_id));
                }
            }
        }
        Ok(())
    }

    /// 验证文件访问
    pub fn validate_file_access(&self, plugin_id: &str, path: &Path, write: bool) -> Result<()> {
        let permission = if write {
            PluginPermission::FileSystemWrite(path.to_path_buf())
        } else {
            PluginPermission::FileSystemRead(path.to_path_buf())
        };
        
        self.check_permission(plugin_id, &permission)
    }

    /// 验证网络访问
    pub fn validate_network_access(&self, plugin_id: &str, domain: &str) -> Result<()> {
        self.check_permission(
            plugin_id,
            &PluginPermission::NetworkAccess(NetworkScope::Domain(domain.to_string())),
        )
    }

    /// 验证程序执行
    pub fn validate_program_execution(&self, plugin_id: &str) -> Result<()> {
        self.check_permission(plugin_id, &PluginPermission::ExecuteProgram)
    }

    /// 获取插件配置
    pub fn get_config(&self, plugin_id: &str) -> Option<SandboxConfig> {
        let configs = self.configs.read().unwrap();
        configs.get(plugin_id).cloned()
    }

    /// 更新插件配置
    pub fn update_config(&self, config: SandboxConfig) {
        let old_config = self.configs.read().unwrap().get(&config.plugin_id).cloned();
        
        // 记录配置变更审计
        if let Some(old) = old_config {
            self.audit_logger.log(
                AuditEventType::ConfigChange {
                    plugin_id: config.plugin_id.clone(),
                    old_level: format!("{:?}", old.security_level),
                    new_level: format!("{:?}", config.security_level),
                },
                AuditSeverity::Info,
            );
        }
        
        let mut configs = self.configs.write().unwrap();
        configs.insert(config.plugin_id.clone(), config);
    }
    
    /// 获取审计日志
    pub fn get_audit_entries(&self) -> Vec<super::audit::AuditLogEntry> {
        self.audit_logger.get_entries()
    }
    
    /// 获取指定插件的审计日志
    pub fn get_plugin_audit_entries(&self, plugin_id: &str) -> Vec<super::audit::AuditLogEntry> {
        self.audit_logger.get_plugin_entries(plugin_id)
    }
    
    /// 获取所有违规尝试
    pub fn get_violations(&self) -> Vec<super::audit::AuditLogEntry> {
        self.audit_logger.get_violations()
    }
    
    /// 获取审计统计信息
    pub fn get_audit_statistics(&self) -> super::audit::AuditStatistics {
        self.audit_logger.get_statistics()
    }
    
    /// 清空审计日志
    pub fn clear_audit_log(&self) {
        self.audit_logger.clear();
    }
    
    /// 导出审计日志为 JSON
    pub fn export_audit_log(&self) -> Result<String> {
        self.audit_logger.export_json()
            .map_err(|e| anyhow!("Failed to export audit log: {}", e))
    }
}

impl Default for SandboxManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 沙盒执行包装器
pub struct SandboxedExecution<T> {
    plugin_id: String,
    manager: Arc<SandboxManager>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> SandboxedExecution<T> {
    pub fn new(plugin_id: String, manager: Arc<SandboxManager>) -> Self {
        Self {
            plugin_id,
            manager,
            _phantom: std::marker::PhantomData,
        }
    }

    /// 在沙盒环境中执行函数
    pub async fn execute<F, Fut>(&self, func: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let config = self.manager.get_config(&self.plugin_id);

        // 如果没有配置或沙盒未启用，直接执行
        if config.is_none() || !config.as_ref().unwrap().enabled {
            return func().await;
        }

        let config = config.unwrap();

        // 应用超时限制
        if let Some(timeout_ms) = config.timeout_ms {
            match tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                func(),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(anyhow!(
                    "Plugin '{}' execution timeout ({}ms)",
                    self.plugin_id,
                    timeout_ms
                )),
            }
        } else {
            func().await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_levels() {
        let system_perms = SecurityLevel::System.default_permissions();
        assert!(system_perms.contains(&PluginPermission::ExecuteProgram));

        let sandboxed_perms = SecurityLevel::Sandboxed.default_permissions();
        assert!(!sandboxed_perms.contains(&PluginPermission::ExecuteProgram));
    }

    #[test]
    fn test_permission_check() {
        let manager = SandboxManager::new();
        
        let config = SandboxConfig::restricted("test_plugin")
            .with_permission(PluginPermission::ExecuteProgram);
        
        manager.register(config);

        assert!(manager.check_permission("test_plugin", &PluginPermission::ExecuteProgram).is_ok());
        assert!(manager.check_permission("test_plugin", &PluginPermission::ProcessManagement).is_err());
    }

    #[tokio::test]
    async fn test_sandboxed_execution() {
        let manager = Arc::new(SandboxManager::new());
        
        let config = SandboxConfig::restricted("test_plugin");
        manager.register(config);

        let executor = SandboxedExecution::<i32>::new("test_plugin".to_string(), manager);

        let result = executor
            .execute(|| async { Ok(42) })
            .await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_timeout() {
        let manager = Arc::new(SandboxManager::new());
        
        let config = SandboxConfig {
            plugin_id: "slow_plugin".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: None,
            enabled: true,
            timeout_ms: Some(100),
            max_memory_mb: None,
        };
        manager.register(config);

        let executor = SandboxedExecution::<()>::new("slow_plugin".to_string(), manager);

        let result = executor
            .execute(|| async {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                Ok(())
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout"));
    }
}
