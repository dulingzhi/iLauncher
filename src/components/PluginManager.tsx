import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

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
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadPlugins();
  }, []);

  const loadPlugins = async () => {
    try {
      const result = await invoke<PluginMetadata[]>('get_plugins');
      setPlugins(result);
    } catch (error) {
      console.error('Failed to load plugins:', error);
    } finally {
      setLoading(false);
    }
  };

  const getIconEmoji = (icon: PluginMetadata['icon']): string => {
    if ('Emoji' in icon) return icon.Emoji;
    return '🔌';
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full bg-gray-900">
        <div className="text-white text-xl">Loading plugins...</div>
      </div>
    );
  }

  return (
    <div className="h-full bg-gray-900 text-white p-6 overflow-y-auto">
      <div className="max-w-4xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <h1 className="text-2xl font-bold">Plugin Manager</h1>
          <button
            onClick={onClose}
            className="px-4 py-2 bg-gray-800 hover:bg-gray-700 rounded-lg transition-colors"
          >
            Close (Esc)
          </button>
        </div>
        
        <div className="grid gap-3">
          {plugins.map((plugin) => (
            <div
              key={plugin.id}
              className="bg-gray-800 rounded-lg p-4 border border-gray-700 hover:border-gray-600 transition-colors"
            >
              <div className="flex items-start gap-3">
                <div className="text-3xl">{getIconEmoji(plugin.icon)}</div>
                
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-2">
                    <h3 className="text-lg font-semibold">{plugin.name}</h3>
                    <span className="text-xs bg-gray-700 px-2 py-0.5 rounded">
                      v{plugin.version}
                    </span>
                    <span className="text-xs bg-blue-600 px-2 py-0.5 rounded">
                      {plugin.plugin_type}
                    </span>
                  </div>
                  
                  <p className="text-gray-400 text-sm mb-2">{plugin.description}</p>
                  
                  <div className="flex items-center gap-3 text-xs text-gray-500">
                    <span>👤 {plugin.author}</span>
                    <span className="truncate">🆔 {plugin.id}</span>
                  </div>
                  
                  {plugin.trigger_keywords.length > 0 && (
                    <div className="mt-2 flex gap-1 flex-wrap">
                      {plugin.trigger_keywords.slice(0, 5).map((keyword) => (
                        <span
                          key={keyword}
                          className="text-xs bg-gray-700 px-2 py-0.5 rounded"
                        >
                          {keyword}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
                
                <div className="flex gap-2">
                  <button className="px-3 py-1.5 bg-green-600 hover:bg-green-700 rounded text-xs font-medium transition-colors">
                    ✓ Enabled
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
        
        <div className="mt-6 p-4 bg-gray-800 rounded-lg border border-gray-700">
          <h2 className="text-lg font-semibold mb-3">Statistics</h2>
          <div className="grid grid-cols-3 gap-3 text-center">
            <div>
              <div className="text-3xl font-bold text-blue-500">{plugins.length}</div>
              <div className="text-sm text-gray-400">Total Plugins</div>
            </div>
            <div>
              <div className="text-3xl font-bold text-green-500">{plugins.length}</div>
              <div className="text-sm text-gray-400">Enabled</div>
            </div>
            <div>
              <div className="text-3xl font-bold text-gray-500">0</div>
              <div className="text-sm text-gray-400">Disabled</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
