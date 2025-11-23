// 审计日志相关命令

use crate::plugin::audit::{AuditLogEntry, AuditStatistics};
use crate::plugin::sandbox::SandboxManager;
use std::sync::Arc;
use tauri::State;

/// 获取所有审计日志
#[tauri::command]
pub async fn get_audit_log(
    sandbox_manager: State<'_, Arc<SandboxManager>>,
) -> Result<Vec<AuditLogEntry>, String> {
    Ok(sandbox_manager.get_audit_entries())
}

/// 获取指定插件的审计日志
#[tauri::command]
pub async fn get_plugin_audit_log(
    plugin_id: String,
    sandbox_manager: State<'_, Arc<SandboxManager>>,
) -> Result<Vec<AuditLogEntry>, String> {
    Ok(sandbox_manager.get_plugin_audit_entries(&plugin_id))
}

/// 获取所有违规尝试
#[tauri::command]
pub async fn get_violations(
    sandbox_manager: State<'_, Arc<SandboxManager>>,
) -> Result<Vec<AuditLogEntry>, String> {
    Ok(sandbox_manager.get_violations())
}

/// 获取审计统计信息
#[tauri::command]
pub async fn get_audit_statistics(
    sandbox_manager: State<'_, Arc<SandboxManager>>,
) -> Result<AuditStatistics, String> {
    Ok(sandbox_manager.get_audit_statistics())
}

/// 清空审计日志
#[tauri::command]
pub async fn clear_audit_log(
    sandbox_manager: State<'_, Arc<SandboxManager>>,
) -> Result<(), String> {
    sandbox_manager.clear_audit_log();
    Ok(())
}

/// 导出审计日志为 JSON
#[tauri::command]
pub async fn export_audit_log(
    sandbox_manager: State<'_, Arc<SandboxManager>>,
) -> Result<String, String> {
    sandbox_manager.export_audit_log()
        .map_err(|e| e.to_string())
}
