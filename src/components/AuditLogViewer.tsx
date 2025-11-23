import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

// 审计事件类型
interface AuditLogEntry {
  timestamp: string;
  event_type: AuditEventType;
  severity: "Info" | "Warning" | "Critical";
}

type AuditEventType =
  | { PermissionCheck: { plugin_id: string; permission: string; allowed: boolean } }
  | { FileAccess: { plugin_id: string; path: string; write: boolean; allowed: boolean } }
  | { NetworkAccess: { plugin_id: string; domain: string; allowed: boolean } }
  | { ProgramExecution: { plugin_id: string; program: string; allowed: boolean } }
  | { ViolationAttempt: { plugin_id: string; violation_type: string; details: string } }
  | { ConfigChange: { plugin_id: string; old_level: string; new_level: string } };

// 审计统计
interface AuditStatistics {
  total_checks: number;
  denied_checks: number;
  file_accesses: number;
  denied_file_accesses: number;
  network_accesses: number;
  denied_network_accesses: number;
  violations: number;
}

interface AuditLogViewerProps {
  pluginId?: string; // 如果指定，只显示该插件的日志
}

export function AuditLogViewer({ pluginId }: AuditLogViewerProps) {
  const [entries, setEntries] = useState<AuditLogEntry[]>([]);
  const [statistics, setStatistics] = useState<AuditStatistics | null>(null);
  const [filter, setFilter] = useState<"all" | "violations">("all");
  const [loading, setLoading] = useState(false);

  const loadAuditLog = async () => {
    setLoading(true);
    try {
      let logs: AuditLogEntry[];
      
      if (filter === "violations") {
        logs = await invoke<AuditLogEntry[]>("get_violations");
      } else if (pluginId) {
        logs = await invoke<AuditLogEntry[]>("get_plugin_audit_log", { pluginId });
      } else {
        logs = await invoke<AuditLogEntry[]>("get_audit_log");
      }

      setEntries(logs);

      // 加载统计信息
      const stats = await invoke<AuditStatistics>("get_audit_statistics");
      setStatistics(stats);
    } catch (error) {
      console.error("Failed to load audit log:", error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadAuditLog();
  }, [pluginId, filter]);

  const clearLog = async () => {
    if (!confirm("确定要清空审计日志吗？")) return;
    
    try {
      await invoke("clear_audit_log");
      await loadAuditLog();
    } catch (error) {
      console.error("Failed to clear audit log:", error);
    }
  };

  const exportLog = async () => {
    try {
      const json = await invoke<string>("export_audit_log");
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `audit-log-${new Date().toISOString()}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      console.error("Failed to export audit log:", error);
    }
  };

  const getSeverityColor = (severity: string) => {
    switch (severity) {
      case "Info": return "text-blue-400";
      case "Warning": return "text-yellow-400";
      case "Critical": return "text-red-400";
      default: return "text-gray-400";
    }
  };

  const getSeverityIcon = (severity: string) => {
    switch (severity) {
      case "Info": return "ℹ️";
      case "Warning": return "⚠️";
      case "Critical": return "🚨";
      default: return "📝";
    }
  };

  const renderEventType = (eventType: AuditEventType) => {
    if ("PermissionCheck" in eventType) {
      const { plugin_id, permission, allowed } = eventType.PermissionCheck;
      return (
        <div className="space-y-1">
          <div className="font-medium">权限检查</div>
          <div className="text-sm text-gray-400">
            插件: <span className="text-blue-400">{plugin_id}</span>
          </div>
          <div className="text-sm text-gray-400">
            权限: <span className="text-purple-400">{permission}</span>
          </div>
          <div className="text-sm">
            结果: <span className={allowed ? "text-green-400" : "text-red-400"}>
              {allowed ? "✓ 允许" : "✗ 拒绝"}
            </span>
          </div>
        </div>
      );
    }

    if ("FileAccess" in eventType) {
      const { plugin_id, path, write, allowed } = eventType.FileAccess;
      return (
        <div className="space-y-1">
          <div className="font-medium">文件访问</div>
          <div className="text-sm text-gray-400">
            插件: <span className="text-blue-400">{plugin_id}</span>
          </div>
          <div className="text-sm text-gray-400">
            路径: <span className="text-yellow-400 break-all">{path}</span>
          </div>
          <div className="text-sm text-gray-400">
            操作: {write ? "📝 写入" : "📖 读取"}
          </div>
          <div className="text-sm">
            结果: <span className={allowed ? "text-green-400" : "text-red-400"}>
              {allowed ? "✓ 允许" : "✗ 拒绝"}
            </span>
          </div>
        </div>
      );
    }

    if ("NetworkAccess" in eventType) {
      const { plugin_id, domain, allowed } = eventType.NetworkAccess;
      return (
        <div className="space-y-1">
          <div className="font-medium">网络访问</div>
          <div className="text-sm text-gray-400">
            插件: <span className="text-blue-400">{plugin_id}</span>
          </div>
          <div className="text-sm text-gray-400">
            域名: <span className="text-cyan-400">{domain}</span>
          </div>
          <div className="text-sm">
            结果: <span className={allowed ? "text-green-400" : "text-red-400"}>
              {allowed ? "✓ 允许" : "✗ 拒绝"}
            </span>
          </div>
        </div>
      );
    }

    if ("ViolationAttempt" in eventType) {
      const { plugin_id, violation_type, details } = eventType.ViolationAttempt;
      return (
        <div className="space-y-1 border-l-4 border-red-500 pl-3">
          <div className="font-medium text-red-400">安全违规尝试</div>
          <div className="text-sm text-gray-400">
            插件: <span className="text-red-400">{plugin_id}</span>
          </div>
          <div className="text-sm text-gray-400">
            类型: <span className="text-orange-400">{violation_type}</span>
          </div>
          <div className="text-sm text-gray-400">
            详情: <span className="text-gray-300">{details}</span>
          </div>
        </div>
      );
    }

    if ("ConfigChange" in eventType) {
      const { plugin_id, old_level, new_level } = eventType.ConfigChange;
      return (
        <div className="space-y-1">
          <div className="font-medium">配置变更</div>
          <div className="text-sm text-gray-400">
            插件: <span className="text-blue-400">{plugin_id}</span>
          </div>
          <div className="text-sm text-gray-400">
            变更: <span className="text-yellow-400">{old_level}</span>
            {" → "}
            <span className="text-green-400">{new_level}</span>
          </div>
        </div>
      );
    }

    return <div>未知事件类型</div>;
  };

  return (
    <div className="space-y-4 p-4">
      {/* 标题和操作 */}
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold text-white">
          {pluginId ? `插件审计日志: ${pluginId}` : "全局审计日志"}
        </h2>
        <div className="flex gap-2">
          <button
            onClick={loadAuditLog}
            disabled={loading}
            className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 rounded text-sm transition-colors disabled:opacity-50"
          >
            {loading ? "加载中..." : "刷新"}
          </button>
          <button
            onClick={exportLog}
            className="px-3 py-1.5 bg-green-600 hover:bg-green-700 rounded text-sm transition-colors"
          >
            导出 JSON
          </button>
          <button
            onClick={clearLog}
            className="px-3 py-1.5 bg-red-600 hover:bg-red-700 rounded text-sm transition-colors"
          >
            清空日志
          </button>
        </div>
      </div>

      {/* 统计信息 */}
      {statistics && (
        <div className="grid grid-cols-4 gap-4 p-4 bg-surface/50 rounded-lg">
          <div className="text-center">
            <div className="text-2xl font-bold text-white">{statistics.total_checks}</div>
            <div className="text-sm text-gray-400">权限检查</div>
          </div>
          <div className="text-center">
            <div className="text-2xl font-bold text-yellow-400">{statistics.denied_checks}</div>
            <div className="text-sm text-gray-400">拒绝次数</div>
          </div>
          <div className="text-center">
            <div className="text-2xl font-bold text-cyan-400">{statistics.network_accesses}</div>
            <div className="text-sm text-gray-400">网络访问</div>
          </div>
          <div className="text-center">
            <div className="text-2xl font-bold text-red-400">{statistics.violations}</div>
            <div className="text-sm text-gray-400">违规尝试</div>
          </div>
        </div>
      )}

      {/* 过滤器 */}
      <div className="flex gap-2">
        <button
          onClick={() => setFilter("all")}
          className={`px-4 py-2 rounded transition-colors ${
            filter === "all"
              ? "bg-primary text-white"
              : "bg-surface/50 text-gray-400 hover:bg-surface"
          }`}
        >
          全部事件
        </button>
        <button
          onClick={() => setFilter("violations")}
          className={`px-4 py-2 rounded transition-colors ${
            filter === "violations"
              ? "bg-red-600 text-white"
              : "bg-surface/50 text-gray-400 hover:bg-surface"
          }`}
        >
          违规尝试
        </button>
      </div>

      {/* 日志条目 */}
      <div className="space-y-2 max-h-[600px] overflow-y-auto">
        {entries.length === 0 ? (
          <div className="text-center py-12 text-gray-400">
            {loading ? "加载中..." : "暂无审计日志"}
          </div>
        ) : (
          entries.map((entry, index) => (
            <div
              key={index}
              className="p-4 bg-surface/30 hover:bg-surface/50 rounded-lg transition-colors border-l-4"
              style={{
                borderColor: entry.severity === "Critical" ? "#ef4444" :
                            entry.severity === "Warning" ? "#f59e0b" : "#3b82f6"
              }}
            >
              <div className="flex items-start justify-between mb-2">
                <div className="flex items-center gap-2">
                  <span className="text-lg">{getSeverityIcon(entry.severity)}</span>
                  <span className={`text-sm font-medium ${getSeverityColor(entry.severity)}`}>
                    {entry.severity}
                  </span>
                </div>
                <div className="text-xs text-gray-500">
                  {new Date(entry.timestamp).toLocaleString("zh-CN")}
                </div>
              </div>
              {renderEventType(entry.event_type)}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
