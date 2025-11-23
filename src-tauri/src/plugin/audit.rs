// 插件沙盒审计日志系统

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// 审计事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    /// 权限检查
    PermissionCheck {
        plugin_id: String,
        permission: String,
        allowed: bool,
    },
    /// 文件访问
    FileAccess {
        plugin_id: String,
        path: String,
        write: bool,
        allowed: bool,
    },
    /// 网络访问
    NetworkAccess {
        plugin_id: String,
        domain: String,
        allowed: bool,
    },
    /// 程序执行
    ProgramExecution {
        plugin_id: String,
        program: String,
        allowed: bool,
    },
    /// 沙盒违规尝试
    ViolationAttempt {
        plugin_id: String,
        violation_type: String,
        details: String,
    },
    /// 配置变更
    ConfigChange {
        plugin_id: String,
        old_level: String,
        new_level: String,
    },
}

/// 审计日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub severity: AuditSeverity,
}

/// 审计严重程度
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditSeverity {
    Info,
    Warning,
    Critical,
}

/// 审计日志管理器
pub struct AuditLogger {
    entries: Arc<Mutex<Vec<AuditLogEntry>>>,
    max_entries: usize,
}

impl AuditLogger {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            max_entries,
        }
    }

    /// 记录审计事件
    pub fn log(&self, event_type: AuditEventType, severity: AuditSeverity) {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            event_type: event_type.clone(),
            severity,
        };

        let mut entries = self.entries.lock().unwrap();
        entries.push(entry);

        // 保持日志大小限制
        if entries.len() > self.max_entries {
            entries.remove(0);
        }

        // 记录到 tracing 日志
        match severity {
            AuditSeverity::Info => {
                tracing::info!("🔍 Audit: {:?}", event_type);
            }
            AuditSeverity::Warning => {
                tracing::warn!("⚠️ Audit Warning: {:?}", event_type);
            }
            AuditSeverity::Critical => {
                tracing::error!("🚨 Audit Critical: {:?}", event_type);
            }
        }
    }

    /// 获取所有日志条目
    pub fn get_entries(&self) -> Vec<AuditLogEntry> {
        self.entries.lock().unwrap().clone()
    }

    /// 获取特定插件的日志
    pub fn get_plugin_entries(&self, plugin_id: &str) -> Vec<AuditLogEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|entry| match &entry.event_type {
                AuditEventType::PermissionCheck { plugin_id: id, .. }
                | AuditEventType::FileAccess { plugin_id: id, .. }
                | AuditEventType::NetworkAccess { plugin_id: id, .. }
                | AuditEventType::ProgramExecution { plugin_id: id, .. }
                | AuditEventType::ViolationAttempt { plugin_id: id, .. }
                | AuditEventType::ConfigChange { plugin_id: id, .. } => id == plugin_id,
            })
            .cloned()
            .collect()
    }

    /// 获取违规尝试
    pub fn get_violations(&self) -> Vec<AuditLogEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|entry| matches!(entry.event_type, AuditEventType::ViolationAttempt { .. }))
            .cloned()
            .collect()
    }

    /// 清空日志
    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
        tracing::info!("🔍 Audit log cleared");
    }

    /// 导出日志为 JSON
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        let entries = self.get_entries();
        serde_json::to_string_pretty(&entries)
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> AuditStatistics {
        let entries = self.entries.lock().unwrap();
        
        let mut stats = AuditStatistics::default();
        
        for entry in entries.iter() {
            match &entry.event_type {
                AuditEventType::PermissionCheck { allowed, .. } => {
                    stats.total_checks += 1;
                    if !allowed {
                        stats.denied_checks += 1;
                    }
                }
                AuditEventType::FileAccess { allowed, .. } => {
                    stats.file_accesses += 1;
                    if !allowed {
                        stats.denied_file_accesses += 1;
                    }
                }
                AuditEventType::NetworkAccess { allowed, .. } => {
                    stats.network_accesses += 1;
                    if !allowed {
                        stats.denied_network_accesses += 1;
                    }
                }
                AuditEventType::ViolationAttempt { .. } => {
                    stats.violations += 1;
                }
                _ => {}
            }
        }
        
        stats
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// 审计统计信息
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AuditStatistics {
    pub total_checks: usize,
    pub denied_checks: usize,
    pub file_accesses: usize,
    pub denied_file_accesses: usize,
    pub network_accesses: usize,
    pub denied_network_accesses: usize,
    pub violations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_logger() {
        let logger = AuditLogger::new(10);
        
        logger.log(
            AuditEventType::PermissionCheck {
                plugin_id: "test".to_string(),
                permission: "FileRead".to_string(),
                allowed: true,
            },
            AuditSeverity::Info,
        );
        
        let entries = logger.get_entries();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_audit_logger_max_entries() {
        let logger = AuditLogger::new(5);
        
        for i in 0..10 {
            logger.log(
                AuditEventType::PermissionCheck {
                    plugin_id: format!("test_{}", i),
                    permission: "FileRead".to_string(),
                    allowed: true,
                },
                AuditSeverity::Info,
            );
        }
        
        let entries = logger.get_entries();
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_get_plugin_entries() {
        let logger = AuditLogger::new(100);
        
        logger.log(
            AuditEventType::PermissionCheck {
                plugin_id: "plugin1".to_string(),
                permission: "FileRead".to_string(),
                allowed: true,
            },
            AuditSeverity::Info,
        );
        
        logger.log(
            AuditEventType::PermissionCheck {
                plugin_id: "plugin2".to_string(),
                permission: "FileRead".to_string(),
                allowed: false,
            },
            AuditSeverity::Warning,
        );
        
        let plugin1_entries = logger.get_plugin_entries("plugin1");
        assert_eq!(plugin1_entries.len(), 1);
        
        let plugin2_entries = logger.get_plugin_entries("plugin2");
        assert_eq!(plugin2_entries.len(), 1);
    }

    #[test]
    fn test_statistics() {
        let logger = AuditLogger::new(100);
        
        logger.log(
            AuditEventType::PermissionCheck {
                plugin_id: "test".to_string(),
                permission: "FileRead".to_string(),
                allowed: true,
            },
            AuditSeverity::Info,
        );
        
        logger.log(
            AuditEventType::PermissionCheck {
                plugin_id: "test".to_string(),
                permission: "FileWrite".to_string(),
                allowed: false,
            },
            AuditSeverity::Warning,
        );
        
        let stats = logger.get_statistics();
        assert_eq!(stats.total_checks, 2);
        assert_eq!(stats.denied_checks, 1);
    }
}
