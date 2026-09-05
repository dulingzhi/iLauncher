//! 插件沙盒审计日志：对齐旧版 audit 插件的事件模型与统计语义。
//! 两处刻意偏离（注释标明）：时间戳用 Unix 秒（无 chrono 依赖，UTC 展示复用
//! preview::format_unix_utc）；增加 JSONL 持久化——旧版纯内存、重启即丢，
//! 启动器常驻场景审计日志应可回溯（PluginManager 落地后事件源接入此处）。
//!
//! 无 gpui 依赖，全部单测。

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

/// 内存容量上限（与 旧版默认一致）
pub const DEFAULT_MAX_ENTRIES: usize = 1000;

/// 审计事件类型（与 旧版六类一致）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl AuditEventType {
    /// 事件归属插件 id
    pub fn plugin_id(&self) -> &str {
        match self {
            AuditEventType::PermissionCheck { plugin_id, .. }
            | AuditEventType::FileAccess { plugin_id, .. }
            | AuditEventType::NetworkAccess { plugin_id, .. }
            | AuditEventType::ProgramExecution { plugin_id, .. }
            | AuditEventType::ViolationAttempt { plugin_id, .. }
            | AuditEventType::ConfigChange { plugin_id, .. } => plugin_id,
        }
    }

    /// 单行摘要（查看器列表用）
    pub fn summarize(&self) -> String {
        match self {
            AuditEventType::PermissionCheck {
                plugin_id,
                permission,
                allowed,
            } => format!("{plugin_id} 权限检查 {permission} → {}", verdict(*allowed)),
            AuditEventType::FileAccess {
                plugin_id,
                path,
                write,
                allowed,
            } => format!(
                "{plugin_id} 文件{} {} → {}",
                if *write { "写入" } else { "读取" },
                path,
                verdict(*allowed),
            ),
            AuditEventType::NetworkAccess {
                plugin_id,
                domain,
                allowed,
            } => format!("{plugin_id} 网络访问 {domain} → {}", verdict(*allowed)),
            AuditEventType::ProgramExecution {
                plugin_id,
                program,
                allowed,
            } => format!("{plugin_id} 执行 {program} → {}", verdict(*allowed)),
            AuditEventType::ViolationAttempt {
                plugin_id,
                violation_type,
                details,
            } => format!("{plugin_id} 违规尝试 {violation_type}: {details}"),
            AuditEventType::ConfigChange {
                plugin_id,
                old_level,
                new_level,
            } => format!("{plugin_id} 配置 {old_level} → {new_level}"),
        }
    }
}

fn verdict(allowed: bool) -> &'static str {
    if allowed { "允许" } else { "拒绝" }
}

/// 审计严重程度
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditSeverity {
    Info,
    Warning,
    Critical,
}

impl AuditSeverity {
    pub fn label(&self) -> &'static str {
        match self {
            AuditSeverity::Info => "信息",
            AuditSeverity::Warning => "警告",
            AuditSeverity::Critical => "严重",
        }
    }
}

/// 审计日志条目（timestamp = Unix 秒，UTC）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: u64,
    pub event_type: AuditEventType,
    pub severity: AuditSeverity,
}

/// 审计统计（字段与 旧版一致）
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditStatistics {
    pub total_checks: usize,
    pub denied_checks: usize,
    pub file_accesses: usize,
    pub denied_file_accesses: usize,
    pub network_accesses: usize,
    pub denied_network_accesses: usize,
    pub violations: usize,
}

impl AuditStatistics {
    /// 查看器状态栏单行摘要
    pub fn summarize(&self) -> String {
        format!(
            "权限检查 {}（拒 {}）· 文件 {}（拒 {}）· 网络 {}（拒 {}）· 违规 {}",
            self.total_checks,
            self.denied_checks,
            self.file_accesses,
            self.denied_file_accesses,
            self.network_accesses,
            self.denied_network_accesses,
            self.violations,
        )
    }
}

/// 审计日志管理器：内存 + 可选 JSONL 持久化
pub struct AuditLogger {
    entries: Vec<AuditLogEntry>,
    max_entries: usize,
    persist_path: Option<PathBuf>,
}

impl AuditLogger {
    pub fn in_memory(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: max_entries.max(1),
            persist_path: None,
        }
    }

    /// 带 JSONL 持久化：加载既有日志（坏行跳过），后续追加写
    pub fn with_persist(path: &Path, max_entries: usize) -> Result<Self> {
        let mut logger = Self::in_memory(max_entries);
        logger.persist_path = Some(path.to_path_buf());
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Ok(entry) = serde_json::from_str::<AuditLogEntry>(line) {
                    logger.entries.push(entry);
                }
            }
            // 载入即截断到容量（防文件膨胀）
            let overflow = logger.entries.len().saturating_sub(logger.max_entries);
            if overflow > 0 {
                logger.entries.drain(..overflow);
                logger.persist_rewrite();
            }
        }
        Ok(logger)
    }

    /// 记录审计事件（容量截断最旧 + 追加持久化）
    pub fn log(&mut self, event_type: AuditEventType, severity: AuditSeverity) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.entries.push(AuditLogEntry {
            timestamp,
            event_type,
            severity,
        });
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.persist_append();
    }

    pub fn entries(&self) -> &[AuditLogEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 违规尝试（新→旧）
    pub fn violations(&self) -> Vec<&AuditLogEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| matches!(e.event_type, AuditEventType::ViolationAttempt { .. }))
            .collect()
    }

    /// 清空（连同持久化文件）
    pub fn clear(&mut self) {
        self.entries.clear();
        if let Some(path) = &self.persist_path {
            let _ = std::fs::write(path, "");
        }
    }

    /// 导出全部日志为 pretty JSON
    pub fn export_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.entries).context("审计日志序列化失败")
    }

    pub fn statistics(&self) -> AuditStatistics {
        let mut stats = AuditStatistics::default();
        for entry in &self.entries {
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

    fn persist_append(&self) {
        let Some(path) = &self.persist_path else { return };
        let Some(last) = self.entries.last() else { return };
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path)
            && let Ok(line) = serde_json::to_string(last) {
                use std::io::Write as _;
                let _ = writeln!(f, "{line}");
            }
    }

    fn persist_rewrite(&self) {
        let Some(path) = &self.persist_path else { return };
        if let Ok(mut f) = std::fs::File::create(path) {
            use std::io::Write as _;
            for entry in &self.entries {
                if let Ok(line) = serde_json::to_string(entry) {
                    let _ = writeln!(f, "{line}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        crate::test_util::tempdir(&format!("audit_{tag}")).join("audit.jsonl")
    }

    fn perm_check(plugin: &str, allowed: bool) -> AuditEventType {
        AuditEventType::PermissionCheck {
            plugin_id: plugin.into(),
            permission: "FileRead".into(),
            allowed,
        }
    }

    #[test]
    fn log_and_capacity_truncates_oldest() {
        let mut logger = AuditLogger::in_memory(3);
        for i in 0..5 {
            logger.log(perm_check(&format!("p{i}"), true), AuditSeverity::Info);
        }
        assert_eq!(logger.len(), 3);
        // 最旧的 p0/p1 被截掉
        assert_eq!(logger.entries()[0].event_type.plugin_id(), "p2");
    }

    #[test]
    fn plugin_filter_and_violations() {
        let mut logger = AuditLogger::in_memory(100);
        logger.log(perm_check("a", true), AuditSeverity::Info);
        logger.log(
            AuditEventType::ViolationAttempt {
                plugin_id: "b".into(),
                violation_type: "escape".into(),
                details: "tried /etc".into(),
            },
            AuditSeverity::Critical,
        );
        logger.log(perm_check("b", false), AuditSeverity::Warning);

        assert_eq!(logger.violations().len(), 1);
        let v = logger.violations();
        assert_eq!(v[0].severity, AuditSeverity::Critical);
        // plugin_id 提取（查看器搜索过滤用）
        assert_eq!(perm_check("b", false).plugin_id(), "b");
        assert!(matches!(
            logger.entries()[2].event_type,
            AuditEventType::PermissionCheck { ref plugin_id, .. } if plugin_id == "b"
        ));
    }

    #[test]
    fn statistics_match_legacy_semantics() {
        let mut logger = AuditLogger::in_memory(100);
        logger.log(perm_check("t", true), AuditSeverity::Info);
        logger.log(perm_check("t", false), AuditSeverity::Warning);
        logger.log(
            AuditEventType::FileAccess {
                plugin_id: "t".into(),
                path: "C:\\x".into(),
                write: true,
                allowed: false,
            },
            AuditSeverity::Warning,
        );
        logger.log(
            AuditEventType::NetworkAccess {
                plugin_id: "t".into(),
                domain: "example.com".into(),
                allowed: true,
            },
            AuditSeverity::Info,
        );
        logger.log(
            AuditEventType::ViolationAttempt {
                plugin_id: "t".into(),
                violation_type: "v".into(),
                details: "d".into(),
            },
            AuditSeverity::Critical,
        );
        let s = logger.statistics();
        assert_eq!(s.total_checks, 2);
        assert_eq!(s.denied_checks, 1);
        assert_eq!(s.file_accesses, 1);
        assert_eq!(s.denied_file_accesses, 1);
        assert_eq!(s.network_accesses, 1);
        assert_eq!(s.denied_network_accesses, 0);
        assert_eq!(s.violations, 1);
        assert!(s.summarize().contains("违规 1"));
    }

    #[test]
    fn persist_roundtrip_and_clear() {
        let path = temp_path("persist");
        {
            let mut logger = AuditLogger::with_persist(&path, 100).unwrap();
            logger.log(perm_check("x", true), AuditSeverity::Info);
            logger.log(perm_check("y", false), AuditSeverity::Warning);
        }
        let logger = AuditLogger::with_persist(&path, 100).unwrap();
        assert_eq!(logger.len(), 2);
        assert_eq!(logger.entries()[0].event_type.plugin_id(), "x");

        let mut logger = logger;
        logger.clear();
        assert!(logger.entries().is_empty());
        // 清空后文件也为空，重载仍是空
        let reloaded = AuditLogger::with_persist(&path, 100).unwrap();
        assert!(reloaded.entries().is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn persist_skips_bad_lines_and_caps_on_load() {
        let path = temp_path("badlines");
        std::fs::write(&path, "{\"broken\": true}\n").unwrap();
        let mut logger = AuditLogger::with_persist(&path, 2).unwrap();
        assert!(logger.entries().is_empty());
        logger.log(perm_check("a", true), AuditSeverity::Info);
        logger.log(perm_check("b", true), AuditSeverity::Info);
        logger.log(perm_check("c", true), AuditSeverity::Info);
        // 容量 2：文件里只留 b/c
        let reloaded = AuditLogger::with_persist(&path, 2).unwrap();
        assert_eq!(reloaded.len(), 2);
        assert_eq!(reloaded.entries()[0].event_type.plugin_id(), "b");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn summarize_covers_all_event_variants() {
        let cases = [
            (
                perm_check("p", true),
                "p 权限检查 FileRead → 允许",
            ),
            (
                AuditEventType::FileAccess {
                    plugin_id: "p".into(),
                    path: "C:\\a.txt".into(),
                    write: false,
                    allowed: false,
                },
                "p 文件读取 C:\\a.txt → 拒绝",
            ),
            (
                AuditEventType::NetworkAccess {
                    plugin_id: "p".into(),
                    domain: "cdn.x.com".into(),
                    allowed: true,
                },
                "p 网络访问 cdn.x.com → 允许",
            ),
            (
                AuditEventType::ProgramExecution {
                    plugin_id: "ilauncher-core".into(),
                    program: "C:\\app.exe".into(),
                    allowed: true,
                },
                "ilauncher-core 执行 C:\\app.exe → 允许",
            ),
            (
                AuditEventType::ViolationAttempt {
                    plugin_id: "p".into(),
                    violation_type: "fs-escape".into(),
                    details: "path traversal".into(),
                },
                "p 违规尝试 fs-escape: path traversal",
            ),
            (
                AuditEventType::ConfigChange {
                    plugin_id: "p".into(),
                    old_level: "standard".into(),
                    new_level: "strict".into(),
                },
                "p 配置 standard → strict",
            ),
        ];
        for (event, expected) in cases {
            assert_eq!(event.summarize(), expected);
        }
        assert_eq!(perm_check("p", true).plugin_id(), "p");
    }

    #[test]
    fn export_json_parses_back() {
        let mut logger = AuditLogger::in_memory(10);
        logger.log(perm_check("z", true), AuditSeverity::Info);
        let json = logger.export_json().unwrap();
        let parsed: Vec<AuditLogEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].event_type.plugin_id(), "z");
    }
}
