import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { useThemeStore } from '../stores/themeStore';

interface AppConfig {
  general: {
    hotkey: string;
    search_delay: number;
    max_results: number;
    language: string;
    clear_on_hide: boolean;
  };
  appearance: {
    theme: string;
    language: string;
    window_width: number;
    window_height: number;
    font_size: number;
    transparency: number;
    show_preview: boolean;
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
  const { t, i18n } = useTranslation();
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [activeTab, setActiveTab] = useState<'general' | 'appearance' | 'plugins' | 'advanced'>('general');
  const { setTheme } = useThemeStore();

  useEffect(() => {
    loadConfig();
  }, []);

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
      setTheme(loadedConfig.appearance.theme);
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
      setTheme(config.appearance.theme);
      i18n.changeLanguage(config.appearance.language);
      alert(t('settings.saveSuccess') || 'Settings saved!');
    } catch (error) {
      console.error('Failed to save config:', error);
      alert('Failed to save settings');
    } finally {
      setSaving(false);
    }
  };

  if (loading || !config) {
    return (
      <div className="flex items-center justify-center h-full bg-[#1e1e1e]">
        <div className="text-gray-300 text-sm">Loading settings...</div>
      </div>
    );
  }

  return (
    <div className="h-full bg-[#1e1e1e] text-gray-200 flex flex-col">
      {/* 顶部标题栏 - VS Code 风格 */}
      <div className="flex items-center justify-between px-5 py-2.5 bg-[#2d2d30] border-b border-[#3e3e42]">
        <h1 className="text-sm font-medium text-gray-100">Settings</h1>
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

      {/* 主体：侧边栏 + 内容区 */}
      <div className="flex-1 flex overflow-hidden">
        {/* 左侧导航 - VS Code 侧边栏风格 */}
        <div className="w-52 bg-[#252526] border-r border-[#3e3e42] overflow-y-auto">
          <nav className="py-2">
            {[
              { key: 'general' as const, label: t('settings.general') || 'General' },
              { key: 'appearance' as const, label: t('settings.appearance') || 'Appearance' },
              { key: 'plugins' as const, label: t('settings.plugins') || 'Plugins' },
              { key: 'advanced' as const, label: t('settings.advanced') || 'Advanced' },
            ].map(({ key, label }) => (
              <button
                key={key}
                onClick={() => setActiveTab(key)}
                className={`w-full px-5 py-2 text-left text-[13px] transition-colors ${
                  activeTab === key
                    ? 'bg-[#37373d] text-white border-l-2 border-[#007acc]'
                    : 'text-gray-400 hover:bg-[#2a2d2e] hover:text-gray-200'
                }`}
              >
                {label}
              </button>
            ))}
          </nav>
        </div>

        {/* 右侧内容区 */}
        <div className="flex-1 flex flex-col overflow-hidden">
          <div className="flex-1 overflow-y-auto px-8 py-6">
            <div className="max-w-2xl">
              {/* General 设置 */}
              {activeTab === 'general' && (
                <div className="space-y-6">
                  <div>
                    <h2 className="text-lg font-semibold mb-4 text-gray-100">General Settings</h2>
                    
                    <div className="space-y-4">
                      <div>
                        <label className="block text-sm font-medium mb-2 text-gray-300">
                          Global Hotkey
                        </label>
                        <input
                          type="text"
                          value={config.general.hotkey}
                          onChange={(e) => setConfig({
                            ...config,
                            general: { ...config.general, hotkey: e.target.value }
                          })}
                          className="w-full max-w-md px-3 py-2 text-sm bg-[#3c3c3c] text-gray-200 rounded border border-[#555] focus:border-[#007acc] focus:outline-none transition-colors"
                          placeholder="Alt+Space"
                        />
                        <p className="mt-1 text-xs text-gray-500">Press the key combination you want to use</p>
                      </div>

                      <div>
                        <label className="block text-sm font-medium mb-2 text-gray-300">
                          Search Delay (ms)
                        </label>
                        <input
                          type="number"
                          value={config.general.search_delay}
                          onChange={(e) => setConfig({
                            ...config,
                            general: { ...config.general, search_delay: parseInt(e.target.value) || 0 }
                          })}
                          className="w-full max-w-md px-3 py-2 text-sm bg-[#3c3c3c] text-gray-200 rounded border border-[#555] focus:border-[#007acc] focus:outline-none transition-colors"
                          min="0"
                          max="1000"
                        />
                        <p className="mt-1 text-xs text-gray-500">Debounce delay for search input</p>
                      </div>

                      <div>
                        <label className="block text-sm font-medium mb-2 text-gray-300">
                          Max Results
                        </label>
                        <input
                          type="number"
                          value={config.general.max_results}
                          onChange={(e) => setConfig({
                            ...config,
                            general: { ...config.general, max_results: parseInt(e.target.value) || 10 }
                          })}
                          className="w-full max-w-md px-3 py-2 text-sm bg-[#3c3c3c] text-gray-200 rounded border border-[#555] focus:border-[#007acc] focus:outline-none transition-colors"
                          min="1"
                          max="50"
                        />
                        <p className="mt-1 text-xs text-gray-500">Maximum number of results to display</p>
                      </div>

                      <div>
                        <label className="flex items-center justify-between px-4 py-3 bg-[#2d2d30] rounded border border-[#3e3e42] cursor-pointer hover:bg-[#323234] transition-colors">
                          <div>
                            <span className="text-sm font-medium text-gray-300">{t('settings.clearOnHide')}</span>
                            <p className="text-xs text-gray-500 mt-0.5">{t('settings.clearOnHideDesc')}</p>
                          </div>
                          <input
                            type="checkbox"
                            checked={config.general.clear_on_hide}
                            onChange={(e) => setConfig({
                              ...config,
                              general: { ...config.general, clear_on_hide: e.target.checked }
                            })}
                            className="w-4 h-4 accent-[#007acc]"
                          />
                        </label>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {/* Appearance 设置 */}
              {activeTab === 'appearance' && (
                <div className="space-y-6">
                  <div>
                    <h2 className="text-lg font-semibold mb-4 text-gray-100">Appearance Settings</h2>
                    
                    <div className="space-y-4">
                      <div>
                        <label className="block text-sm font-medium mb-2 text-gray-300">
                          {t('settings.theme') || 'Theme'}
                        </label>
                        <div className="flex gap-2">
                          {['dark', 'light', 'blue', 'purple', 'green'].map((themeName) => (
                            <button
                              key={themeName}
                              onClick={() => {
                                setTheme(themeName);
                                setConfig({
                                  ...config,
                                  appearance: { ...config.appearance, theme: themeName }
                                });
                              }}
                              className={`px-4 py-2 rounded border text-sm transition-colors ${
                                config.appearance.theme === themeName
                                  ? 'border-[#007acc] bg-[#007acc]/20 text-white'
                                  : 'border-[#555] bg-[#3c3c3c] text-gray-400 hover:border-[#666] hover:text-gray-200'
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
                        <p className="mt-1 text-xs text-gray-500">Choose your preferred color theme for the search interface</p>
                      </div>

                      <div>
                        <label className="block text-sm font-medium mb-2 text-gray-300">
                          {t('settings.language') || 'Language'}
                        </label>
                        <select
                          value={config.appearance.language}
                          onChange={(e) => setConfig({
                            ...config,
                            appearance: { ...config.appearance, language: e.target.value }
                          })}
                          className="w-full max-w-md px-3 py-2 text-sm bg-[#3c3c3c] text-gray-200 rounded border border-[#555] focus:border-[#007acc] focus:outline-none transition-colors"
                        >
                          <option value="zh-CN">中文</option>
                          <option value="en">English</option>
                        </select>
                      </div>

                      <div>
                        <label className="block text-sm font-medium mb-2 text-gray-300">
                          Window Width: {config.appearance.window_width}px
                        </label>
                        <input
                          type="range"
                          min="600"
                          max="1200"
                          value={config.appearance.window_width}
                          onChange={(e) => setConfig({
                            ...config,
                            appearance: { ...config.appearance, window_width: parseInt(e.target.value) }
                          })}
                          className="w-full max-w-md accent-[#007acc]"
                        />
                      </div>

                      <div>
                        <label className="block text-sm font-medium mb-2 text-gray-300">
                          Transparency: {config.appearance.transparency}%
                        </label>
                        <input
                          type="range"
                          min="50"
                          max="100"
                          value={config.appearance.transparency}
                          onChange={(e) => setConfig({
                            ...config,
                            appearance: { ...config.appearance, transparency: parseInt(e.target.value) }
                          })}
                          className="w-full max-w-md accent-[#007acc]"
                        />
                      </div>

                      <div className="flex items-center justify-between py-2">
                        <div>
                          <label className="block text-sm font-medium text-gray-300">
                            Enable File Preview
                          </label>
                          <p className="mt-1 text-xs text-gray-500">
                            Show file preview panel when selecting files in search results
                          </p>
                        </div>
                        <label className="relative inline-flex items-center cursor-pointer">
                          <input
                            type="checkbox"
                            checked={config.appearance.show_preview}
                            onChange={(e) => setConfig({
                              ...config,
                              appearance: { ...config.appearance, show_preview: e.target.checked }
                            })}
                            className="sr-only peer"
                          />
                          <div className="w-9 h-5 bg-[#3c3c3c] peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-[#007acc]"></div>
                        </label>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {/* Plugins 设置 */}
              {activeTab === 'plugins' && (
                <div className="space-y-6">
                  <div>
                    <h2 className="text-lg font-semibold mb-4 text-gray-100">Plugin Settings</h2>
                    <div className="space-y-2">
                      {config.plugins.enabled_plugins.map((plugin) => (
                        <div
                          key={plugin}
                          className="flex items-center justify-between px-4 py-3 bg-[#2d2d30] rounded border border-[#3e3e42]"
                        >
                          <span className="text-sm text-gray-300 capitalize">{plugin.replace('_', ' ')}</span>
                          <label className="relative inline-flex items-center cursor-pointer">
                            <input
                              type="checkbox"
                              checked={true}
                              className="sr-only peer"
                            />
                            <div className="w-9 h-5 bg-[#3c3c3c] peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-[#007acc]"></div>
                          </label>
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
              )}

              {/* Advanced 设置 */}
              {activeTab === 'advanced' && (
                <div className="space-y-6">
                  <div>
                    <h2 className="text-lg font-semibold mb-4 text-gray-100">Advanced Settings</h2>
                    
                    <div className="space-y-3">
                      <label className="flex items-center justify-between px-4 py-3 bg-[#2d2d30] rounded border border-[#3e3e42] cursor-pointer hover:bg-[#323234] transition-colors">
                        <span className="text-sm text-gray-300">Start on Boot</span>
                        <input
                          type="checkbox"
                          checked={config.advanced.start_on_boot}
                          onChange={(e) => setConfig({
                            ...config,
                            advanced: { ...config.advanced, start_on_boot: e.target.checked }
                          })}
                          className="w-4 h-4 accent-[#007acc]"
                        />
                      </label>

                      <label className="flex items-center justify-between px-4 py-3 bg-[#2d2d30] rounded border border-[#3e3e42] cursor-pointer hover:bg-[#323234] transition-colors">
                        <span className="text-sm text-gray-300">Show Tray Icon</span>
                        <input
                          type="checkbox"
                          checked={config.advanced.show_tray_icon}
                          onChange={(e) => setConfig({
                            ...config,
                            advanced: { ...config.advanced, show_tray_icon: e.target.checked }
                          })}
                          className="w-4 h-4 accent-[#007acc]"
                        />
                      </label>

                      <label className="flex items-center justify-between px-4 py-3 bg-[#2d2d30] rounded border border-[#3e3e42] cursor-pointer hover:bg-[#323234] transition-colors">
                        <span className="text-sm text-gray-300">Cache Enabled</span>
                        <input
                          type="checkbox"
                          checked={config.advanced.cache_enabled}
                          onChange={(e) => setConfig({
                            ...config,
                            advanced: { ...config.advanced, cache_enabled: e.target.checked }
                          })}
                          className="w-4 h-4 accent-[#007acc]"
                        />
                      </label>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* 底部操作栏 */}
          <div className="flex items-center justify-end gap-3 px-8 py-4 bg-[#2d2d30] border-t border-[#3e3e42]">
            <button
              onClick={handleSave}
              disabled={saving}
              className="px-5 py-2 text-sm bg-[#0e639c] hover:bg-[#1177bb] text-white rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {saving ? 'Saving...' : t('settings.save') || 'Save Settings'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
