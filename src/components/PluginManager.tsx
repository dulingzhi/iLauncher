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

export const PluginManager: React.FC = () => {
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
      <div className="flex items-center justify-center h-screen bg-gray-900">
        <div className="text-white text-xl">Loading plugins...</div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-900 text-white p-8">
      <div className="max-w-4xl mx-auto">
        <h1 className="text-3xl font-bold mb-8">Plugin Manager</h1>
        
        <div className="grid gap-4">
          {plugins.map((plugin) => (
            <div
              key={plugin.id}
              className="bg-gray-800 rounded-lg p-6 border border-gray-700 hover:border-gray-600 transition-colors"
            >
              <div className="flex items-start gap-4">
                <div className="text-4xl">{getIconEmoji(plugin.icon)}</div>
                
                <div className="flex-1">
                  <div className="flex items-center gap-3 mb-2">
                    <h3 className="text-xl font-semibold">{plugin.name}</h3>
                    <span className="text-xs bg-gray-700 px-2 py-1 rounded">
                      v{plugin.version}
                    </span>
                    <span className="text-xs bg-blue-600 px-2 py-1 rounded">
                      {plugin.plugin_type}
                    </span>
                  </div>
                  
                  <p className="text-gray-400 mb-3">{plugin.description}</p>
                  
                  <div className="flex items-center gap-4 text-sm text-gray-500">
                    <span>👤 {plugin.author}</span>
                    <span>🆔 {plugin.id}</span>
                  </div>
                  
                  {plugin.trigger_keywords.length > 0 && (
                    <div className="mt-3 flex gap-2 flex-wrap">
                      {plugin.trigger_keywords.map((keyword) => (
                        <span
                          key={keyword}
                          className="text-xs bg-gray-700 px-2 py-1 rounded"
                        >
                          {keyword}
                        </span>
                      ))}
                    </div>
                  )}
                  
                  <div className="mt-3 flex gap-2 text-xs">
                    {plugin.supported_os.map((os) => (
                      <span key={os} className="text-gray-500">
                        {os === 'windows' && '🪟'}
                        {os === 'macos' && '🍎'}
                        {os === 'linux' && '🐧'}
                        {os}
                      </span>
                    ))}
                  </div>
                </div>
                
                <div className="flex gap-2">
                  <button className="px-4 py-2 bg-green-600 hover:bg-green-700 rounded text-sm font-medium transition-colors">
                    ✓ Enabled
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
        
        <div className="mt-8 p-6 bg-gray-800 rounded-lg border border-gray-700">
          <h2 className="text-xl font-semibold mb-4">Statistics</h2>
          <div className="grid grid-cols-3 gap-4 text-center">
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
