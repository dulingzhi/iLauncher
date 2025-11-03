import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AppConfig {
  general: {
    hotkey: string;
    search_delay: number;
    max_results: number;
    language: string;
  };
  appearance: {
    theme: string;
    window_width: number;
    window_height: number;
    font_size: number;
    transparency: number;
  };
  plugins: {
    enabled_plugins: string[];
    disabled_plugins: string[];
  };
  advanced: {
    start_on_boot: boolean;
    show_tray_icon: boolean;
    enable_analytics: boolean;
    cache_enabled: boolean;
  };
}

export const Settings: React.FC = () => {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [activeTab, setActiveTab] = useState<'general' | 'appearance' | 'plugins' | 'advanced'>('general');

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      const loadedConfig = await invoke<AppConfig>('load_config');
      setConfig(loadedConfig);
    } catch (error) {
      console.error('Failed to load config:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!config) return;
    
    setSaving(true);
    try {
      await invoke('save_config', { config });
      alert('Settings saved successfully!');
    } catch (error) {
      console.error('Failed to save config:', error);
      alert('Failed to save settings');
    } finally {
      setSaving(false);
    }
  };

  const handleReset = async () => {
    if (confirm('Reset all settings to default?')) {
      setLoading(true);
      try {
        // 重新加载会使用默认值
        await loadConfig();
      } finally {
        setLoading(false);
      }
    }
  };

  if (loading || !config) {
    return (
      <div className="flex items-center justify-center h-screen bg-gray-900">
        <div className="text-white text-xl">Loading settings...</div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-900 text-white p-8">
      <div className="max-w-5xl mx-auto">
        <h1 className="text-3xl font-bold mb-8">Settings</h1>
        
        {/* 标签页 */}
        <div className="flex gap-2 mb-6 border-b border-gray-700">
          {(['general', 'appearance', 'plugins', 'advanced'] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={`px-6 py-3 font-medium capitalize transition-colors ${
                activeTab === tab
                  ? 'text-blue-500 border-b-2 border-blue-500'
                  : 'text-gray-400 hover:text-gray-300'
              }`}
            >
              {tab}
            </button>
          ))}
        </div>

        {/* 通用设置 */}
        {activeTab === 'general' && (
          <div className="space-y-6">
            <div className="bg-gray-800 rounded-lg p-6">
              <h2 className="text-xl font-semibold mb-4">General Settings</h2>
              
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium mb-2">
                    Global Hotkey
                  </label>
                  <input
                    type="text"
                    value={config.general.hotkey}
                    onChange={(e) => setConfig({
                      ...config,
                      general: { ...config.general, hotkey: e.target.value }
                    })}
                    className="w-full px-4 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
                    placeholder="Alt+Space"
                  />
                  <p className="text-xs text-gray-400 mt-1">
                    Keyboard shortcut to show/hide iLauncher
                  </p>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Search Delay (ms)
                  </label>
                  <input
                    type="number"
                    value={config.general.search_delay}
                    onChange={(e) => setConfig({
                      ...config,
                      general: { ...config.general, search_delay: parseInt(e.target.value) }
                    })}
                    className="w-full px-4 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
                    min="0"
                    max="1000"
                  />
                  <p className="text-xs text-gray-400 mt-1">
                    Delay before starting search (debounce)
                  </p>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Max Results
                  </label>
                  <input
                    type="number"
                    value={config.general.max_results}
                    onChange={(e) => setConfig({
                      ...config,
                      general: { ...config.general, max_results: parseInt(e.target.value) }
                    })}
                    className="w-full px-4 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
                    min="5"
                    max="50"
                  />
                  <p className="text-xs text-gray-400 mt-1">
                    Maximum number of results to display
                  </p>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* 外观设置 */}
        {activeTab === 'appearance' && (
          <div className="space-y-6">
            <div className="bg-gray-800 rounded-lg p-6">
              <h2 className="text-xl font-semibold mb-4">Appearance</h2>
              
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium mb-3">Theme</label>
                  <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
                    {['dark', 'light', 'blue', 'purple', 'green'].map((themeName) => (
                      <button
                        key={themeName}
                        onClick={() => setConfig({
                          ...config,
                          appearance: { ...config.appearance, theme: themeName }
                        })}
                        className={`py-3 px-4 rounded-lg border-2 transition-colors capitalize ${
                          config.appearance.theme === themeName
                            ? 'border-blue-500 bg-gray-700'
                            : 'border-gray-600 bg-gray-800 hover:border-gray-500'
                        }`}
                      >
                        {themeName === 'dark' && '🌙 Dark'}
                        {themeName === 'light' && '☀️ Light'}
                        {themeName === 'blue' && '💙 Blue'}
                        {themeName === 'purple' && '💜 Purple'}
                        {themeName === 'green' && '💚 Green'}
                      </button>
                    ))}
                  </div>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Window Width
                  </label>
                  <input
                    type="range"
                    value={config.appearance.window_width}
                    onChange={(e) => setConfig({
                      ...config,
                      appearance: { ...config.appearance, window_width: parseInt(e.target.value) }
                    })}
                    className="w-full"
                    min="600"
                    max="1200"
                    step="50"
                  />
                  <div className="text-sm text-gray-400 mt-1">{config.appearance.window_width}px</div>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Window Height
                  </label>
                  <input
                    type="range"
                    value={config.appearance.window_height}
                    onChange={(e) => setConfig({
                      ...config,
                      appearance: { ...config.appearance, window_height: parseInt(e.target.value) }
                    })}
                    className="w-full"
                    min="400"
                    max="800"
                    step="50"
                  />
                  <div className="text-sm text-gray-400 mt-1">{config.appearance.window_height}px</div>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Font Size
                  </label>
                  <input
                    type="range"
                    value={config.appearance.font_size}
                    onChange={(e) => setConfig({
                      ...config,
                      appearance: { ...config.appearance, font_size: parseInt(e.target.value) }
                    })}
                    className="w-full"
                    min="12"
                    max="20"
                  />
                  <div className="text-sm text-gray-400 mt-1">{config.appearance.font_size}px</div>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* 插件设置 */}
        {activeTab === 'plugins' && (
          <div className="bg-gray-800 rounded-lg p-6">
            <h2 className="text-xl font-semibold mb-4">Plugin Settings</h2>
            <p className="text-gray-400">
              Plugin-specific settings will appear here. Go to Plugin Manager to enable/disable plugins.
            </p>
          </div>
        )}

        {/* 高级设置 */}
        {activeTab === 'advanced' && (
          <div className="bg-gray-800 rounded-lg p-6">
            <h2 className="text-xl font-semibold mb-4">Advanced Settings</h2>
            <div className="space-y-4">
              <div className="flex items-center justify-between p-4 bg-gray-700 rounded">
                <div>
                  <div className="font-medium">Start on System Boot</div>
                  <div className="text-sm text-gray-400">Launch iLauncher when Windows starts</div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input 
                    type="checkbox" 
                    className="sr-only peer"
                    checked={config.advanced.start_on_boot}
                    onChange={(e) => setConfig({
                      ...config,
                      advanced: { ...config.advanced, start_on_boot: e.target.checked }
                    })}
                  />
                  <div className="w-11 h-6 bg-gray-600 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
                </label>
              </div>

              <div className="flex items-center justify-between p-4 bg-gray-700 rounded">
                <div>
                  <div className="font-medium">Show Tray Icon</div>
                  <div className="text-sm text-gray-400">Display icon in system tray</div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input 
                    type="checkbox" 
                    className="sr-only peer"
                    checked={config.advanced.show_tray_icon}
                    onChange={(e) => setConfig({
                      ...config,
                      advanced: { ...config.advanced, show_tray_icon: e.target.checked }
                    })}
                  />
                  <div className="w-11 h-6 bg-gray-600 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
                </label>
              </div>

              <div className="flex items-center justify-between p-4 bg-gray-700 rounded">
                <div>
                  <div className="font-medium">Enable Analytics</div>
                  <div className="text-sm text-gray-400">Help improve iLauncher by sharing usage data</div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input 
                    type="checkbox" 
                    className="sr-only peer"
                    checked={config.advanced.enable_analytics}
                    onChange={(e) => setConfig({
                      ...config,
                      advanced: { ...config.advanced, enable_analytics: e.target.checked }
                    })}
                  />
                  <div className="w-11 h-6 bg-gray-600 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
                </label>
              </div>
            </div>
          </div>
        )}

        {/* 保存按钮 */}
        <div className="mt-8 flex justify-end gap-4">
          <button 
            onClick={handleReset}
            className="px-6 py-2 bg-gray-700 hover:bg-gray-600 rounded font-medium transition-colors"
            disabled={saving}
          >
            Reset
          </button>
          <button
            onClick={handleSave}
            className="px-6 py-2 bg-blue-600 hover:bg-blue-700 rounded font-medium transition-colors disabled:opacity-50"
            disabled={saving}
          >
            {saving ? 'Saving...' : 'Save Settings'}
          </button>
        </div>
      </div>
    </div>
  );
};
