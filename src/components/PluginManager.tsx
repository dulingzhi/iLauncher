import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Settings, Power, PowerOff, RefreshCw, Download } from 'lucide-react';
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
}

interface PluginManagerProps {
  onClose: () => void;
}

export const PluginManager: React.FC<PluginManagerProps> = ({ onClose }) => {
  const [plugins, setPlugins] = useState<PluginMetadata[]>([]);
  const [pluginStatuses, setPluginStatuses] = useState<Map<string, boolean>>(new Map());
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [configPlugin, setConfigPlugin] = useState<string | null>(null);
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
                        onClick={() => setConfigPlugin(plugin.id)}
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

      {/* 配置面板（占位） */}
      {configPlugin && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-[#2d2d30] rounded-lg shadow-2xl w-full max-w-2xl max-h-[80vh] overflow-hidden flex flex-col">
            <div className="flex items-center justify-between px-6 py-4 bg-[#1e1e1e] border-b border-[#3e3e42]">
              <h2 className="text-lg font-semibold text-gray-100">
                Plugin Configuration
              </h2>
              <button
                onClick={() => setConfigPlugin(null)}
                className="text-gray-400 hover:text-gray-100 transition-colors"
              >
                ✕
              </button>
            </div>
            <div className="flex-1 overflow-y-auto p-6">
              <p className="text-gray-400 text-center py-8">
                Configuration for plugin: <span className="text-white font-mono">{configPlugin}</span>
              </p>
              <p className="text-gray-500 text-sm text-center">
                Plugin-specific settings will be available here
              </p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
