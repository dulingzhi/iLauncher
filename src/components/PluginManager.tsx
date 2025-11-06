import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Settings, Power, PowerOff, RefreshCw, Download, X, Save, AlertCircle } from 'lucide-react';
import { useConfigStore } from '../store/useConfigStore';

interface PluginMetadata {
  id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  icon: { Emoji: string } | { Url: string } | { File: string };
  trigger_keywords: string[];
  supported_os: string[];
  plugin_type: string;
  settings: Array<{
    type: string;
    key?: string;
    label?: string;
    value?: any;
  }>;
}

interface PluginManagerProps {
  onClose: () => void;
}

interface PluginConfig {
  [key: string]: any;
}

export const PluginManager: React.FC<PluginManagerProps> = ({ onClose }) => {
  const [plugins, setPlugins] = useState<PluginMetadata[]>([]);
  const [pluginStatuses, setPluginStatuses] = useState<Map<string, boolean>>(new Map());
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [configPlugin, setConfigPlugin] = useState<PluginMetadata | null>(null);
  const { config, saveConfig } = useConfigStore();

  useEffect(() => {
    loadPlugins();
  }, []);

  // ESC 键监听
  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        // 只需要切换回搜索视图，不需要隐藏窗口
        // 窗口会在失焦时自动隐藏
        onClose();
      }
    };
    
    window.addEventListener('keydown', handleEsc);
    return () => window.removeEventListener('keydown', handleEsc);
  }, [onClose]);

  const loadPlugins = async () => {
    try {
      const result = await invoke<PluginMetadata[]>('get_plugins');
      setPlugins(result);
      
      // 使用全局配置获取启用状态
      if (config) {
        const statusMap = new Map<string, boolean>();
        
        result.forEach(plugin => {
          const isDisabled = config.plugins.disabled_plugins.includes(plugin.id);
          statusMap.set(plugin.id, !isDisabled);
        });
        
        setPluginStatuses(statusMap);
      }
    } catch (error) {
      console.error('Failed to load plugins:', error);
    } finally {
      setLoading(false);
    }
  };

  const togglePlugin = async (pluginId: string) => {
    if (!config) return;
    
    try {
      const isCurrentlyDisabled = config.plugins.disabled_plugins.includes(pluginId);
      
      const updatedConfig = { ...config };
      if (isCurrentlyDisabled) {
        // 启用插件：从禁用列表移除
        updatedConfig.plugins.disabled_plugins = config.plugins.disabled_plugins.filter(
          (id: string) => id !== pluginId
        );
      } else {
        // 禁用插件：添加到禁用列表
        updatedConfig.plugins.disabled_plugins = [...config.plugins.disabled_plugins, pluginId];
      }
      
      await saveConfig(updatedConfig);
      
      // 更新本地状态
      const newStatuses = new Map(pluginStatuses);
      newStatuses.set(pluginId, !isCurrentlyDisabled);
      setPluginStatuses(newStatuses);
      
    } catch (error) {
      console.error('Failed to toggle plugin:', error);
      alert('Failed to toggle plugin: ' + error);
    }
  };

  const refreshPlugins = async () => {
    setRefreshing(true);
    await loadPlugins();
    setTimeout(() => setRefreshing(false), 500);
  };

  const getIconEmoji = (icon: PluginMetadata['icon']): string => {
    if ('Emoji' in icon) return icon.Emoji;
    return '🔌';
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full bg-[#1e1e1e]">
        <div className="text-gray-300 text-sm">Loading plugins...</div>
      </div>
    );
  }

  const enabledCount = Array.from(pluginStatuses.values()).filter(Boolean).length;
  const disabledCount = plugins.length - enabledCount;

  return (
    <div className="h-full bg-[#1e1e1e] text-gray-200 flex flex-col">
      {/* 顶部标题栏 */}
      <div className="flex items-center justify-between px-6 py-3 bg-[#2d2d30] border-b border-[#3e3e42]">
        <div className="flex items-center gap-3">
          <h1 className="text-base font-semibold text-gray-100">Plugin Manager</h1>
          <button
            onClick={refreshPlugins}
            disabled={refreshing}
            className={`p-1.5 hover:bg-[#3e3e42] rounded transition-colors ${refreshing ? 'animate-spin' : ''}`}
            title="Refresh plugins"
          >
            <RefreshCw className="w-4 h-4 text-gray-400" />
          </button>
        </div>
        <button
          onClick={async () => {
            await invoke('hide_app');
            onClose();
          }}
          className="px-3 py-1 text-xs text-gray-400 hover:text-gray-100 hover:bg-[#3e3e42] rounded transition-colors"
        >
          Close (Esc)
        </button>
      </div>

      {/* 主体内容 */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-5xl mx-auto space-y-4">
          {/* 统计卡片 */}
          <div className="grid grid-cols-3 gap-4 mb-6">
            <div className="bg-[#252526] border border-[#3e3e42] rounded-lg p-4">
              <div className="text-2xl font-bold text-blue-400">{plugins.length}</div>
              <div className="text-xs text-gray-500 mt-1">Total Plugins</div>
            </div>
            <div className="bg-[#252526] border border-[#3e3e42] rounded-lg p-4">
              <div className="text-2xl font-bold text-green-400">{enabledCount}</div>
              <div className="text-xs text-gray-500 mt-1">Enabled</div>
            </div>
            <div className="bg-[#252526] border border-[#3e3e42] rounded-lg p-4">
              <div className="text-2xl font-bold text-red-400">{disabledCount}</div>
              <div className="text-xs text-gray-500 mt-1">Disabled</div>
            </div>
          </div>

          {/* 插件列表 */}
          <div className="space-y-3">
            {plugins.map((plugin) => {
              const isEnabled = pluginStatuses.get(plugin.id) ?? true;
              
              return (
                <div
                  key={plugin.id}
                  className={`bg-[#252526] rounded-lg p-4 border transition-all ${
                    isEnabled
                      ? 'border-[#3e3e42] hover:border-[#555]'
                      : 'border-[#3e3e42] opacity-60'
                  }`}
                >
                  <div className="flex items-start gap-4">
                    <div className={`text-3xl ${!isEnabled && 'grayscale opacity-50'}`}>
                      {getIconEmoji(plugin.icon)}
                    </div>
                    
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-2 flex-wrap">
                        <h3 className="text-base font-semibold text-gray-100">{plugin.name}</h3>
                        <span className="text-xs bg-[#3e3e42] px-2 py-0.5 rounded text-gray-400">
                          v{plugin.version}
                        </span>
                        <span className="text-xs bg-blue-600/20 text-blue-400 px-2 py-0.5 rounded">
                          {plugin.plugin_type}
                        </span>
                        {!isEnabled && (
                          <span className="text-xs bg-red-600/20 text-red-400 px-2 py-0.5 rounded">
                            Disabled
                          </span>
                        )}
                      </div>
                      
                      <p className="text-gray-400 text-sm mb-2 line-clamp-2">{plugin.description}</p>
                      
                      <div className="flex items-center gap-3 text-xs text-gray-500">
                        <span>👤 {plugin.author}</span>
                        <span className="truncate">🆔 {plugin.id}</span>
                      </div>
                      
                      {plugin.trigger_keywords.length > 0 && (
                        <div className="mt-2 flex gap-1 flex-wrap">
                          {plugin.trigger_keywords.slice(0, 5).map((keyword) => (
                            <span
                              key={keyword}
                              className="text-xs bg-[#3e3e42] px-2 py-0.5 rounded text-gray-400"
                            >
                              {keyword}
                            </span>
                          ))}
                          {plugin.trigger_keywords.length > 5 && (
                            <span className="text-xs text-gray-500">
                              +{plugin.trigger_keywords.length - 5} more
                            </span>
                          )}
                        </div>
                      )}
                    </div>
                    
                    {/* 操作按钮 */}
                    <div className="flex flex-col gap-2">
                      <button
                        onClick={() => togglePlugin(plugin.id)}
                        className={`px-3 py-1.5 rounded text-xs font-medium transition-colors flex items-center gap-1.5 ${
                          isEnabled
                            ? 'bg-green-600/20 text-green-400 hover:bg-green-600/30 border border-green-600/30'
                            : 'bg-gray-700 text-gray-400 hover:bg-gray-600 border border-gray-600'
                        }`}
                      >
                        {isEnabled ? (
                          <>
                            <Power className="w-3 h-3" />
                            Enabled
                          </>
                        ) : (
                          <>
                            <PowerOff className="w-3 h-3" />
                            Disabled
                          </>
                        )}
                      </button>
                      
                      <button
                        onClick={() => setConfigPlugin(plugin)}
                        className="px-3 py-1.5 bg-[#3e3e42] hover:bg-[#555] text-gray-300 rounded text-xs font-medium transition-colors flex items-center gap-1.5"
                        title="Configure plugin"
                      >
                        <Settings className="w-3 h-3" />
                        Config
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>

          {/* 占位：未来的插件商店 */}
          <div className="mt-6 bg-[#252526] border border-[#3e3e42] rounded-lg p-6 text-center">
            <Download className="w-12 h-12 text-gray-600 mx-auto mb-3" />
            <h3 className="text-lg font-semibold text-gray-100 mb-2">Plugin Store Coming Soon</h3>
            <p className="text-sm text-gray-500">
              Discover and install community plugins to extend iLauncher's functionality
            </p>
          </div>
        </div>
      </div>

      {/* 配置面板 */}
      {configPlugin && (
        <PluginConfigPanel
          plugin={configPlugin}
          onClose={() => setConfigPlugin(null)}
        />
      )}
    </div>
  );
};

// 插件配置面板组件
interface PluginConfigPanelProps {
  plugin: PluginMetadata;
  onClose: () => void;
}

const PluginConfigPanel: React.FC<PluginConfigPanelProps> = ({ plugin, onClose }) => {
  const [config, setConfig] = useState<PluginConfig>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 加载插件配置
  useEffect(() => {
    const loadPluginConfig = async () => {
      try {
        const pluginConfig = await invoke<PluginConfig>('get_plugin_config', {
          pluginId: plugin.id,
        });
        setConfig(pluginConfig);
      } catch (err) {
        console.error('Failed to load plugin config:', err);
        setError('Failed to load configuration');
      } finally {
        setLoading(false);
      }
    };

    loadPluginConfig();
  }, [plugin.id]);

  // 保存插件配置
  const handleSave = async () => {
    setSaving(true);
    setError(null);

    try {
      await invoke('save_plugin_config', {
        pluginId: plugin.id,
        config,
      });
      onClose();
    } catch (err) {
      console.error('Failed to save plugin config:', err);
      setError('Failed to save configuration');
    } finally {
      setSaving(false);
    }
  };

  const getIconEmoji = (icon: PluginMetadata['icon']): string => {
    if ('Emoji' in icon) return icon.Emoji;
    return '🔌';
  };

  // 渲染设置项
  const renderSettingField = (setting: PluginMetadata['settings'][0], index: number) => {
    const key = setting.key || `setting_${index}`;
    const value = config[key] || setting.value || '';

    const updateValue = (newValue: any) => {
      setConfig({ ...config, [key]: newValue });
    };

    switch (setting.type) {
      case 'head':
        return (
          <div key={index} className="pt-4 pb-2">
            <h3 className="text-base font-semibold text-gray-100">{setting.label}</h3>
          </div>
        );

      case 'newline':
        return <div key={index} className="h-3" />;

      case 'textbox':
        return (
          <div key={index} className="space-y-2">
            <label className="block text-sm font-medium text-gray-300">
              {setting.label}
            </label>
            <input
              type="text"
              value={value}
              onChange={(e) => updateValue(e.target.value)}
              className="w-full px-3 py-2 bg-[#3c3c3c] border border-[#555] rounded text-gray-200 text-sm focus:border-[#007acc] focus:outline-none transition-colors"
              placeholder={setting.label}
            />
          </div>
        );

      case 'checkbox':
        return (
          <div key={index} className="flex items-center justify-between py-2">
            <label className="text-sm font-medium text-gray-300">
              {setting.label}
            </label>
            <input
              type="checkbox"
              checked={Boolean(value)}
              onChange={(e) => updateValue(e.target.checked)}
              className="w-4 h-4 accent-[#007acc]"
            />
          </div>
        );

      case 'select':
        const options = setting.value?.options || [];
        return (
          <div key={index} className="space-y-2">
            <label className="block text-sm font-medium text-gray-300">
              {setting.label}
            </label>
            <select
              value={value}
              onChange={(e) => updateValue(e.target.value)}
              className="w-full px-3 py-2 bg-[#3c3c3c] border border-[#555] rounded text-gray-200 text-sm focus:border-[#007acc] focus:outline-none transition-colors"
            >
              {options.map((option: any, i: number) => (
                <option key={i} value={option.value || option}>
                  {option.label || option}
                </option>
              ))}
            </select>
          </div>
        );

      case 'number':
        return (
          <div key={index} className="space-y-2">
            <label className="block text-sm font-medium text-gray-300">
              {setting.label}
            </label>
            <input
              type="number"
              value={value}
              onChange={(e) => updateValue(parseFloat(e.target.value) || 0)}
              className="w-full px-3 py-2 bg-[#3c3c3c] border border-[#555] rounded text-gray-200 text-sm focus:border-[#007acc] focus:outline-none transition-colors"
            />
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[#1e1e1e] rounded-lg shadow-2xl w-full max-w-3xl max-h-[85vh] overflow-hidden flex flex-col">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#2d2d30] border-b border-[#3e3e42]">
          <div className="flex items-center gap-3">
            <span className="text-2xl">{getIconEmoji(plugin.icon)}</span>
            <div>
              <h2 className="text-lg font-semibold text-gray-100">{plugin.name} Settings</h2>
              <p className="text-xs text-gray-500">Configure plugin preferences</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-[#3e3e42] rounded transition-colors"
            title="Close"
          >
            <X className="w-5 h-5 text-gray-400" />
          </button>
        </div>

        {/* 内容 */}
        <div className="flex-1 overflow-y-auto p-6">
          {loading ? (
            <div className="flex items-center justify-center py-12">
              <div className="text-gray-400">Loading configuration...</div>
            </div>
          ) : error ? (
            <div className="flex items-center justify-center gap-2 py-12 text-red-400">
              <AlertCircle className="w-5 h-5" />
              <span>{error}</span>
            </div>
          ) : plugin.settings && plugin.settings.length > 0 ? (
            <div className="space-y-4 max-w-2xl">
              {plugin.settings.map((setting, index) => renderSettingField(setting, index))}
            </div>
          ) : (
            <div className="flex items-center justify-center py-12">
              <div className="text-center">
                <AlertCircle className="w-12 h-12 text-gray-600 mx-auto mb-3" />
                <p className="text-gray-400 mb-1">No settings available</p>
                <p className="text-gray-600 text-sm">This plugin doesn't have any configurable options</p>
              </div>
            </div>
          )}
        </div>

        {/* 底部操作栏 */}
        <div className="flex items-center justify-end gap-3 px-6 py-4 bg-[#2d2d30] border-t border-[#3e3e42]">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-gray-300 hover:bg-[#3e3e42] rounded transition-colors"
            disabled={saving}
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={saving || !plugin.settings || plugin.settings.length === 0}
            className="px-4 py-2 text-sm bg-[#007acc] text-white rounded hover:bg-[#005a9e] transition-colors flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Save className="w-4 h-4" />
            {saving ? 'Saving...' : 'Save Changes'}
          </button>
        </div>
      </div>
    </div>
  );
};
