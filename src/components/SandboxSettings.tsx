import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Shield, Lock, Unlock, AlertTriangle, Check, X, FileText } from 'lucide-react';
import { AuditLogViewer } from './AuditLogViewer';

interface SandboxConfig {
  plugin_id: string;
  security_level: 'System' | 'Trusted' | 'Restricted' | 'Sandboxed';
  enabled: boolean;
  timeout_ms?: number;
  max_memory_mb?: number;
  custom_permissions?: string[];
}

interface SecurityLevelInfo {
  name: string;
  color: string;
  description: string;
  icon: React.ReactNode;
}

const SECURITY_LEVELS: Record<string, SecurityLevelInfo> = {
  System: {
    name: '系统级',
    color: 'text-green-500',
    description: '完全信任，无限制（内置插件）',
    icon: <Shield className="w-5 h-5" />,
  },
  Trusted: {
    name: '信任级',
    color: 'text-blue-500',
    description: '经过验证的第三方插件',
    icon: <Check className="w-5 h-5" />,
  },
  Restricted: {
    name: '受限级',
    color: 'text-yellow-500',
    description: '未验证的第三方插件',
    icon: <AlertTriangle className="w-5 h-5" />,
  },
  Sandboxed: {
    name: '沙盒级',
    color: 'text-red-500',
    description: '完全隔离，最小权限',
    icon: <Lock className="w-5 h-5" />,
  },
};

interface SandboxSettingsProps {
  pluginId: string;
  pluginName: string;
}

export const SandboxSettings: React.FC<SandboxSettingsProps> = ({ pluginId, pluginName }) => {
  const [config, setConfig] = useState<SandboxConfig | null>(null);
  const [permissions, setPermissions] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'config' | 'audit'>('config');

  useEffect(() => {
    loadConfig();
  }, [pluginId]);

  const loadConfig = async () => {
    try {
      setLoading(true);
      const cfg = await invoke<SandboxConfig | null>('get_sandbox_config', {
        pluginId,
      });
      setConfig(cfg);

      if (cfg) {
        const perms = await invoke<string[]>('get_plugin_permissions', {
          pluginId,
        });
        setPermissions(perms);
      }
    } catch (error) {
      console.error('Failed to load sandbox config:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleToggleSandbox = async () => {
    if (!config) return;

    const newConfig = { ...config, enabled: !config.enabled };
    try {
      await invoke('update_sandbox_config', { config: newConfig });
      setConfig(newConfig);
    } catch (error) {
      console.error('Failed to update sandbox config:', error);
    }
  };

  const handleSecurityLevelChange = async (newLevel: string) => {
    if (!config) return;

    const newConfig = { ...config, security_level: newLevel as any };
    try {
      await invoke('update_sandbox_config', { config: newConfig });
      setConfig(newConfig);
      await loadConfig(); // 重新加载权限列表
    } catch (error) {
      console.error('Failed to update security level:', error);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center p-8">
        <div className="animate-spin w-8 h-8 border-4 border-primary border-t-transparent rounded-full"></div>
      </div>
    );
  }

  if (!config) {
    return (
      <div className="p-6 text-center" style={{ color: 'var(--color-text-secondary)' }}>
        未找到该插件的沙盒配置
      </div>
    );
  }

  const levelInfo = SECURITY_LEVELS[config.security_level];

  return (
    <div className="p-6 space-y-6">
      {/* 标签页导航 */}
      <div className="flex gap-2 border-b" style={{ borderColor: 'var(--color-border)' }}>
        <button
          onClick={() => setActiveTab('config')}
          className={`px-4 py-2 font-medium transition-colors ${
            activeTab === 'config'
              ? 'border-b-2 border-primary'
              : 'opacity-60 hover:opacity-100'
          }`}
          style={{ color: 'var(--color-text-primary)' }}
        >
          🔒 沙盒配置
        </button>
        <button
          onClick={() => setActiveTab('audit')}
          className={`px-4 py-2 font-medium transition-colors ${
            activeTab === 'audit'
              ? 'border-b-2 border-primary'
              : 'opacity-60 hover:opacity-100'
          }`}
          style={{ color: 'var(--color-text-primary)' }}
        >
          <FileText className="w-4 h-4 inline mr-1" />
          审计日志
        </button>
      </div>

      {/* 配置标签页 */}
      {activeTab === 'config' && (
        <>
          {/* 标题 */}
          <div className="flex items-center justify-between">
            <div>
              <h3
                className="text-xl font-bold flex items-center gap-2"
                style={{ color: 'var(--color-text-primary)' }}
              >
                <Shield className="w-6 h-6" />
                沙盒隔离配置
              </h3>
              <p style={{ color: 'var(--color-text-muted)' }} className="text-sm mt-1">
                插件: {pluginName}
              </p>
            </div>

        {/* 启用/禁用开关 */}
        <button
          onClick={handleToggleSandbox}
          className={`flex items-center gap-2 px-4 py-2 rounded-lg font-medium transition-colors ${
            config.enabled
              ? 'bg-green-500 text-white hover:bg-green-600'
              : 'bg-gray-500 text-white hover:bg-gray-600'
          }`}
        >
          {config.enabled ? (
            <>
              <Lock className="w-4 h-4" />
              已启用
            </>
          ) : (
            <>
              <Unlock className="w-4 h-4" />
              已禁用
            </>
          )}
        </button>
      </div>

      {/* 安全级别选择 */}
      <div>
        <h4
          className="font-medium mb-3"
          style={{ color: 'var(--color-text-primary)' }}
        >
          安全级别
        </h4>
        <div className="grid grid-cols-2 gap-3">
          {Object.entries(SECURITY_LEVELS).map(([key, info]) => (
            <button
              key={key}
              onClick={() => handleSecurityLevelChange(key)}
              disabled={config.security_level === 'System'}
              className={`p-4 rounded-lg border-2 transition-all text-left ${
                config.security_level === key
                  ? 'border-primary'
                  : 'border-transparent hover:border-border'
              } ${
                config.security_level === 'System' && key !== 'System'
                  ? 'opacity-50 cursor-not-allowed'
                  : 'cursor-pointer'
              }`}
              style={{
                backgroundColor: 'var(--color-surface)',
              }}
            >
              <div className="flex items-start gap-3">
                <div className={info.color}>{info.icon}</div>
                <div className="flex-1">
                  <div
                    className="font-medium"
                    style={{ color: 'var(--color-text-primary)' }}
                  >
                    {info.name}
                  </div>
                  <div
                    className="text-sm mt-1"
                    style={{ color: 'var(--color-text-muted)' }}
                  >
                    {info.description}
                  </div>
                </div>
                {config.security_level === key && (
                  <Check className="w-5 h-5 text-primary" />
                )}
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* 当前级别信息 */}
      <div
        className="p-4 rounded-lg"
        style={{ backgroundColor: 'var(--color-hover)' }}
      >
        <div className="flex items-center gap-2 mb-2">
          <div className={levelInfo.color}>{levelInfo.icon}</div>
          <span
            className="font-medium"
            style={{ color: 'var(--color-text-primary)' }}
          >
            当前级别: {levelInfo.name}
          </span>
        </div>
        <p
          className="text-sm"
          style={{ color: 'var(--color-text-secondary)' }}
        >
          {levelInfo.description}
        </p>
      </div>

      {/* 超时和内存限制 */}
      {config.enabled && (
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: 'var(--color-text-secondary)' }}
            >
              超时限制 (毫秒)
            </label>
            <div
              className="px-3 py-2 rounded-lg"
              style={{
                backgroundColor: 'var(--color-surface)',
                color: 'var(--color-text-primary)',
              }}
            >
              {config.timeout_ms || '无限制'}
            </div>
          </div>
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: 'var(--color-text-secondary)' }}
            >
              内存限制 (MB)
            </label>
            <div
              className="px-3 py-2 rounded-lg"
              style={{
                backgroundColor: 'var(--color-surface)',
                color: 'var(--color-text-primary)',
              }}
            >
              {config.max_memory_mb || '无限制'}
            </div>
          </div>
        </div>
      )}

      {/* 权限列表 */}
      <div>
        <h4
          className="font-medium mb-3"
          style={{ color: 'var(--color-text-primary)' }}
        >
          当前权限 ({permissions.length})
        </h4>
        <div className="space-y-2 max-h-64 overflow-y-auto">
          {permissions.length === 0 ? (
            <div
              className="text-center py-4"
              style={{ color: 'var(--color-text-muted)' }}
            >
              无权限
            </div>
          ) : (
            permissions.map((perm, index) => (
              <div
                key={index}
                className="px-3 py-2 rounded-lg flex items-center gap-2"
                style={{ backgroundColor: 'var(--color-surface)' }}
              >
                <Check className="w-4 h-4 text-green-500" />
                <span
                  className="text-sm font-mono"
                  style={{ color: 'var(--color-text-secondary)' }}
                >
                  {perm}
                </span>
              </div>
            ))
          )}
        </div>
      </div>

      {/* 警告提示 */}
      {config.security_level === 'System' && (
        <div
          className="p-4 rounded-lg flex items-start gap-3"
          style={{ backgroundColor: 'rgba(34, 197, 94, 0.1)' }}
        >
          <Shield className="w-5 h-5 text-green-500 flex-shrink-0 mt-0.5" />
          <div>
            <div className="font-medium text-green-600 mb-1">系统级插件</div>
            <div className="text-sm text-green-700">
              这是内置插件，拥有完全权限且不受沙盒限制。无需修改安全配置。
            </div>
          </div>
        </div>
      )}

      {config.enabled && config.security_level !== 'System' && (
        <div
          className="p-4 rounded-lg flex items-start gap-3"
          style={{ backgroundColor: 'rgba(59, 130, 246, 0.1)' }}
        >
          <AlertTriangle className="w-5 h-5 text-blue-500 flex-shrink-0 mt-0.5" />
          <div>
            <div className="font-medium text-blue-600 mb-1">沙盒已启用</div>
            <div className="text-sm text-blue-700">
              该插件运行在沙盒环境中，只能访问已授权的资源。如果插件功能异常，请检查权限配置。
            </div>
          </div>
        </div>
      )}
        </>
      )}

      {/* 审计日志标签页 */}
      {activeTab === 'audit' && (
        <AuditLogViewer pluginId={pluginId} />
      )}
    </div>
  );
};
