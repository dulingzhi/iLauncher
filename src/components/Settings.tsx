import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useThemeStore } from '../stores/themeStore';
import { applyTheme, getTheme } from '../theme';

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

interface SettingsProps {
  onClose: () => void;
}

export const Settings: React.FC<SettingsProps> = ({ onClose }) => {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [activeTab, setActiveTab] = useState<'general' | 'appearance' | 'plugins' | 'advanced'>('general');
  const { setTheme, currentTheme } = useThemeStore();

  useEffect(() => {
    loadConfig();
  }, []);

  // ESC 键监听
  useEffect(() => {
    const handleEsc = async (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        await invoke('hide_app');
        onClose();
      }
    };
    
    window.addEventListener('keydown', handleEsc);
    return () => window.removeEventListener('keydown', handleEsc);
  }, [onClose]);

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
      
      // 应用主题
      setTheme(config.appearance.theme);
      
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
      <div className="flex items-center justify-center h-full bg-gray-900">
        <div className="text-white text-xl">Loading settings...</div>
      </div>
    );
  }

  return (
    <div className="h-full bg-gray-900 text-white p-4 overflow-y-auto">
      <div className="max-w-4xl mx-auto">
        <div className="flex items-center justify-between mb-4">
          <h1 className="text-xl font-bold">Settings</h1>
          <button
            onClick={async () => {
              await invoke('hide_app');
              onClose();
            }}
            className="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 rounded text-sm transition-colors"
          >
            Close (Esc)
          </button>
        </div>
        
        {/* 标签页 */}
        <div className="flex gap-2 mb-3 border-b border-gray-700">
          {(['general', 'appearance', 'plugins', 'advanced'] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={`px-3 py-1.5 text-sm font-medium capitalize transition-colors ${
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
          <div className="space-y-3">
            <div className="bg-gray-800 rounded-lg p-3">
              <h2 className="text-base font-semibold mb-2">General Settings</h2>
              
              <div className="space-y-2">
                <div>
                  <label className="block text-xs font-medium mb-1">
                    Global Hotkey
                  </label>
                  <input
                    type="text"
                    value={config.general.hotkey}
                    onChange={(e) => setConfig({
                      ...config,
                      general: { ...config.general, hotkey: e.target.value }
                    })}
                    className="w-full px-3 py-1.5 text-sm bg-gray-700 rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
                    placeholder="Alt+Space"
                  />
                </div>

                <div>
                  <label className="block text-xs font-medium mb-1">
                    Search Delay (ms)
                  </label>
                  <input
                    type="number"
                    value={config.general.search_delay}
                    onChange={(e) => setConfig({
                      ...config,
                      general: { ...config.general, search_delay: parseInt(e.target.value) }
                    })}
                    className="w-full px-3 py-1.5 text-sm bg-gray-700 rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
                    min="0"
                    max="1000"
                  />
                </div>

                <div>
                  <label className="block text-xs font-medium mb-1">
                    Max Results
                  </label>
                  <input
                    type="number"
                    value={config.general.max_results}
                    onChange={(e) => setConfig({
                      ...config,
                      general: { ...config.general, max_results: parseInt(e.target.value) }
                    })}
                    className="w-full px-3 py-1.5 text-sm bg-gray-700 rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
                    min="5"
                    max="50"
                  />
                </div>
              </div>
            </div>
          </div>
        )}

        {/* 外观设置 */}
        {activeTab === 'appearance' && (
          <div className="space-y-3">
            <div className="bg-gray-800 rounded-lg p-3">
              <h2 className="text-base font-semibold mb-2">Appearance</h2>
              
              <div className="space-y-2">
                <div>
                  <label className="block text-xs font-medium mb-1">Theme</label>
                  <div className="grid grid-cols-5 gap-1.5">
                    {['dark', 'light', 'blue', 'purple', 'green'].map((themeName) => (
                      <button
                        key={themeName}
                        onClick={() => {
                          // 立即预览主题
                          setTheme(themeName);
                          setConfig({
                            ...config,
                            appearance: { ...config.appearance, theme: themeName }
                          });
                        }}
                        className={`py-1.5 px-1 rounded border-2 transition-colors text-xs ${
                          config.appearance.theme === themeName
                            ? 'border-blue-500 bg-blue-500/20'
                            : 'border-gray-600 bg-gray-800 hover:border-gray-500'
                        }`}
                      >
                        {themeName === 'dark' && '🌙'}
                        {themeName === 'light' && '☀️'}
                        {themeName === 'blue' && '💙'}
                        {themeName === 'purple' && '💜'}
                        {themeName === 'green' && '💚'}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <label className="block text-xs font-medium mb-1">
                      Width: {config.appearance.window_width}px
                    </label>
                    <input
                      type="range"
                      value={config.appearance.window_width}
                      onChange={(e) => setConfig({
                        ...config,
                        appearance: { ...config.appearance, window_width: parseInt(e.target.value) }
                      })}
                      className="w-full h-1"
                      min="600"
                      max="1200"
                      step="50"
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium mb-1">
                      Height: {config.appearance.window_height}px
                    </label>
                    <input
                      type="range"
                      value={config.appearance.window_height}
                      onChange={(e) => setConfig({
                        ...config,
                        appearance: { ...config.appearance, window_height: parseInt(e.target.value) }
                      })}
                      className="w-full h-1"
                      min="400"
                      max="800"
                      step="50"
                    />
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-medium mb-1">
                    Font Size: {config.appearance.font_size}px
                  </label>
                  <input
                    type="range"
                    value={config.appearance.font_size}
                    onChange={(e) => setConfig({
                      ...config,
                      appearance: { ...config.appearance, font_size: parseInt(e.target.value) }
                    })}
                    className="w-full h-1"
                    min="12"
                    max="20"
                  />
                </div>
              </div>
            </div>
          </div>
        )}

        {/* 插件设置 */}
        {activeTab === 'plugins' && (
          <div className="bg-gray-800 rounded-lg p-3">
            <h2 className="text-base font-semibold mb-2">Plugin Settings</h2>
            <p className="text-gray-400 text-xs">
              Plugin-specific settings will appear here. Go to Plugin Manager to enable/disable plugins.
            </p>
          </div>
        )}

        {/* 高级设置 */}
        {activeTab === 'advanced' && (
          <div className="bg-gray-800 rounded-lg p-3">
            <h2 className="text-base font-semibold mb-2">Advanced Settings</h2>
            <div className="space-y-2">
              <div className="flex items-center justify-between p-2 bg-gray-700 rounded">
                <div>
                  <div className="font-medium text-xs">Start on System Boot</div>
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

              <div className="flex items-center justify-between p-2 bg-gray-700 rounded">
                <div>
                  <div className="font-medium text-xs">Show Tray Icon</div>
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

              <div className="flex items-center justify-between p-2 bg-gray-700 rounded">
                <div>
                  <div className="font-medium text-xs">Enable Analytics</div>
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
        <div className="mt-4 flex justify-end gap-2 pb-2">
          <button 
            onClick={handleReset}
            className="px-4 py-1.5 bg-gray-700 hover:bg-gray-600 rounded text-xs font-medium transition-colors"
            disabled={saving}
          >
            Reset
          </button>
          <button
            onClick={handleSave}
            className="px-4 py-1.5 bg-blue-600 hover:bg-blue-700 rounded text-xs font-medium transition-colors disabled:opacity-50"
            disabled={saving}
          >
            {saving ? 'Saving...' : 'Save Settings'}
          </button>
        </div>
      </div>
    </div>
  );
};
