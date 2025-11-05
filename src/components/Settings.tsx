import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { useThemeStore } from '../stores/themeStore';
import { useConfigStore } from '../store/useConfigStore';
import { applyTheme, Theme, themes } from '../theme';
import { ThemeEditor } from './ThemeEditor';
import { HotkeyRecorder } from './HotkeyRecorder';

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
  const { config: globalConfig, saveConfig: saveGlobalConfig } = useConfigStore();
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [activeTab, setActiveTab] = useState<'general' | 'appearance' | 'plugins' | 'advanced'>('general');
  const [showThemeEditor, setShowThemeEditor] = useState(false);
  const [editingTheme, setEditingTheme] = useState<Theme | null>(null);
  const [hotkeyError, setHotkeyError] = useState<string>('');
  const { setTheme } = useThemeStore();

  // 从全局配置初始化本地编辑状态
  useEffect(() => {
    if (globalConfig) {
      setConfig(globalConfig as any);
      setTheme(globalConfig.appearance.theme);
      setLoading(false);
    }
  }, [globalConfig, setTheme]);

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

  const handleSave = async () => {
    if (!config) return;
    
    setSaving(true);
    try {
      // 保存到全局store（会同时调用后端保存）
      await saveGlobalConfig(config as any);
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

  const handleSaveTheme = (theme: Theme) => {
    if (!config) return;
    
    // 应用主题
    applyTheme(theme);
    // 更新配置
    setConfig({
      ...config,
      appearance: { ...config.appearance, theme: theme.name }
    });
    // 关闭编辑器
    setShowThemeEditor(false);
    setEditingTheme(null);
    alert(`Theme "${theme.name}" created successfully!`);
  };

  if (loading || !config) {
    return (
      <div className="flex items-center justify-center h-full bg-[#1e1e1e]">
        <div className="text-gray-300 text-sm">Loading settings...</div>
      </div>
    );
  }

  return (
    <>
      {/* 主题编辑器 */}
      {showThemeEditor && editingTheme && (
        <ThemeEditor
          initialTheme={editingTheme}
          onSave={handleSaveTheme}
          onClose={() => {
            setShowThemeEditor(false);
            setEditingTheme(null);
          }}
        />
      )}

      <div className="h-full bg-[#1e1e1e] text-gray-200 flex flex-col">
      {/* 顶部标题栏 - VS Code 风格 */}
      <div className="flex items-center justify-between px-6 py-2 bg-[#2d2d30] border-b border-[#3e3e42]">
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
        <div className="w-48 bg-[#252526] border-r border-[#3e3e42] overflow-y-auto flex-shrink-0">
          <nav className="py-1">
            {[
              { key: 'general' as const, label: t('settings.general') || 'General' },
              { key: 'appearance' as const, label: t('settings.appearance') || 'Appearance' },
              { key: 'plugins' as const, label: t('settings.plugins') || 'Plugins' },
              { key: 'advanced' as const, label: t('settings.advanced') || 'Advanced' },
            ].map(({ key, label }) => (
              <button
                key={key}
                onClick={() => setActiveTab(key)}
                className={`w-full px-4 py-2 text-left text-sm transition-colors ${
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
          <div className="flex-1 overflow-y-auto px-6 py-5">
            <div className="max-w-4xl">
              {/* General 设置 */}
              {activeTab === 'general' && (
                <div className="space-y-5">
                  <div>
                    <h2 className="text-base font-semibold mb-3 text-gray-100">General Settings</h2>
                    
                    <div className="space-y-3">
                      <div>
                        <label className="block text-sm font-medium mb-2 text-gray-300">
                          Global Hotkey
                        </label>
                        <HotkeyRecorder
                          value={config.general.hotkey}
                          onChange={(hotkey) => setConfig({
                            ...config,
                            general: { ...config.general, hotkey }
                          })}
                          onValidation={(isValid, message) => {
                            if (!isValid) {
                              setHotkeyError(message || 'Invalid hotkey');
                            } else {
                              setHotkeyError('');
                            }
                          }}
                        />
                        {hotkeyError && (
                          <p className="mt-1.5 text-xs text-red-400">{hotkeyError}</p>
                        )}
                        <p className="mt-1.5 text-xs text-gray-500">
                          Click the input and press your desired key combination. Requires application restart to take effect.
                        </p>
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
                <div className="space-y-5">
                  <div>
                    <h2 className="text-base font-semibold mb-3 text-gray-100">Appearance Settings</h2>
                    
                    <div className="space-y-3">
                      <div>
                        <label className="block text-sm font-medium mb-3 text-gray-300">
                          {t('settings.theme') || 'Theme'}
                        </label>
                        
                        {/* 基础主题 */}
                        <div className="mb-3">
                          <p className="text-xs text-gray-500 mb-2">Basic Themes</p>
                          <div className="flex flex-wrap gap-2">
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
                        </div>

                        {/* 流行主题 */}
                        <div>
                          <p className="text-xs text-gray-500 mb-2">Popular Themes</p>
                          <div className="flex flex-wrap gap-2">
                            {[
                              { name: 'dracula', label: '🧛 Dracula' },
                              { name: 'nord', label: '❄️ Nord' },
                              { name: 'solarized-dark', label: '☀️ Solarized' },
                              { name: 'monokai', label: '🎨 Monokai' },
                              { name: 'one-dark', label: '⚫ One Dark' },
                              { name: 'catppuccin', label: '🐱 Catppuccin' },
                              { name: 'tokyo-night', label: '🌃 Tokyo Night' },
                            ].map(({ name, label }) => (
                              <button
                                key={name}
                                onClick={() => {
                                  setTheme(name);
                                  setConfig({
                                    ...config,
                                    appearance: { ...config.appearance, theme: name }
                                  });
                                }}
                                className={`px-4 py-2 rounded border text-sm transition-colors ${
                                  config.appearance.theme === name
                                    ? 'border-[#007acc] bg-[#007acc]/20 text-white'
                                    : 'border-[#555] bg-[#3c3c3c] text-gray-400 hover:border-[#666] hover:text-gray-200'
                                }`}
                              >
                                {label}
                              </button>
                            ))}
                          </div>
                        </div>
                        
                        <p className="mt-3 text-xs text-gray-500">Choose your preferred color theme for the search interface</p>
                      </div>

                      {/* 自定义主题编辑器 */}
                      <div>
                        <label className="block text-sm font-medium mb-3 text-gray-300">
                          Custom Theme
                        </label>
                        <button
                          onClick={() => {
                            // 使用当前主题作为基础
                            const currentThemeName = config.appearance.theme;
                            const currentTheme = themes[currentThemeName];
                            setEditingTheme(currentTheme || themes.dark);
                            setShowThemeEditor(true);
                          }}
                          className="px-4 py-2 rounded border border-[#555] bg-[#3c3c3c] text-gray-200 text-sm hover:border-[#666] hover:text-white transition-colors"
                        >
                          🎨 Create Custom Theme
                        </button>
                        <p className="mt-2 text-xs text-gray-500">Create your own custom theme with a visual editor</p>
                      </div>

                      {/* 主题导入导出 */}
                      <div>
                        <label className="block text-sm font-medium mb-3 text-gray-300">
                          Import/Export
                        </label>
                        <div 
                          className="p-4 rounded-lg border transition-all duration-300"
                          style={{
                            backgroundColor: 'var(--color-background)',
                            borderColor: 'var(--color-border)'
                          }}
                        >
                          {/* 搜索框预览 */}
                          <div 
                            className="flex items-center gap-3 px-4 py-3 rounded-lg mb-3"
                            style={{ backgroundColor: 'var(--color-surface)' }}
                          >
                            <div className="w-5 h-5 rounded" style={{ backgroundColor: 'var(--color-primary)' }}></div>
                            <div className="flex-1">
                              <div className="h-2 rounded mb-2" style={{ backgroundColor: 'var(--color-text-primary)', width: '60%' }}></div>
                              <div className="h-2 rounded" style={{ backgroundColor: 'var(--color-text-secondary)', width: '40%' }}></div>
                            </div>
                          </div>

                          {/* 结果列表预览 */}
                          <div className="space-y-2">
                            <div 
                              className="flex items-center gap-3 px-4 py-3 rounded-lg"
                              style={{ backgroundColor: 'var(--color-primary-alpha)' }}
                            >
                              <div className="w-8 h-8 rounded" style={{ backgroundColor: 'var(--color-primary)' }}></div>
                              <div className="flex-1">
                                <div className="h-2 rounded mb-2" style={{ backgroundColor: 'var(--color-text-primary)', width: '50%' }}></div>
                                <div className="h-1.5 rounded" style={{ backgroundColor: 'var(--color-text-muted)', width: '35%' }}></div>
                              </div>
                            </div>

                            <div 
                              className="flex items-center gap-3 px-4 py-3 rounded-lg"
                              style={{ backgroundColor: 'var(--color-hover)' }}
                            >
                              <div className="w-8 h-8 rounded" style={{ backgroundColor: 'var(--color-secondary)' }}></div>
                              <div className="flex-1">
                                <div className="h-2 rounded mb-2" style={{ backgroundColor: 'var(--color-text-primary)', width: '45%' }}></div>
                                <div className="h-1.5 rounded" style={{ backgroundColor: 'var(--color-text-muted)', width: '30%' }}></div>
                              </div>
                            </div>

                            <div 
                              className="flex items-center gap-3 px-4 py-3 rounded-lg"
                              style={{ backgroundColor: 'transparent' }}
                            >
                              <div className="w-8 h-8 rounded" style={{ backgroundColor: 'var(--color-accent)' }}></div>
                              <div className="flex-1">
                                <div className="h-2 rounded mb-2" style={{ backgroundColor: 'var(--color-text-primary)', width: '55%' }}></div>
                                <div className="h-1.5 rounded" style={{ backgroundColor: 'var(--color-text-muted)', width: '40%' }}></div>
                              </div>
                            </div>
                          </div>

                          {/* 颜色图例 */}
                          <div className="mt-4 pt-4 grid grid-cols-4 gap-2 text-xs" style={{ borderTop: '1px solid var(--color-border)' }}>
                            <div className="flex items-center gap-2">
                              <div className="w-4 h-4 rounded" style={{ backgroundColor: 'var(--color-primary)' }}></div>
                              <span style={{ color: 'var(--color-text-muted)' }}>Primary</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <div className="w-4 h-4 rounded" style={{ backgroundColor: 'var(--color-secondary)' }}></div>
                              <span style={{ color: 'var(--color-text-muted)' }}>Secondary</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <div className="w-4 h-4 rounded" style={{ backgroundColor: 'var(--color-accent)' }}></div>
                              <span style={{ color: 'var(--color-text-muted)' }}>Accent</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <div className="w-4 h-4 rounded" style={{ backgroundColor: 'var(--color-hover)' }}></div>
                              <span style={{ color: 'var(--color-text-muted)' }}>Hover</span>
                            </div>
                          </div>
                        </div>
                        <p className="mt-2 text-xs text-gray-500">Preview updates instantly when you select a theme</p>
                      </div>

                      {/* 主题导入/导出 */}
                      <div>
                        <label className="block text-sm font-medium mb-3 text-gray-300">
                          Theme Import/Export
                        </label>
                        <div className="flex gap-3">
                          <button
                            onClick={() => {
                              // 导出当前主题
                              const theme = useThemeStore.getState().theme;
                              const themeJson = JSON.stringify(theme, null, 2);
                              const blob = new Blob([themeJson], { type: 'application/json' });
                              const url = URL.createObjectURL(blob);
                              const a = document.createElement('a');
                              a.href = url;
                              a.download = `${theme.name}-theme.json`;
                              document.body.appendChild(a);
                              a.click();
                              document.body.removeChild(a);
                              URL.revokeObjectURL(url);
                            }}
                            className="px-4 py-2 rounded border border-[#555] bg-[#3c3c3c] text-gray-200 text-sm hover:border-[#666] hover:text-white transition-colors"
                          >
                            📤 Export Current Theme
                          </button>
                          
                          <button
                            onClick={() => {
                              // 创建文件输入元素
                              const input = document.createElement('input');
                              input.type = 'file';
                              input.accept = '.json';
                              input.onchange = (e: any) => {
                                const file = e.target?.files?.[0];
                                if (file) {
                                  const reader = new FileReader();
                                  reader.onload = (event) => {
                                    try {
                                      const themeData = JSON.parse(event.target?.result as string);
                                      // 验证主题格式
                                      if (themeData.name && themeData.colors) {
                                        // 应用导入的主题
                                        applyTheme(themeData);
                                        // 更新配置
                                        setConfig({
                                          ...config,
                                          appearance: { ...config.appearance, theme: 'custom' }
                                        });
                                        alert(`Theme "${themeData.name}" imported successfully!`);
                                      } else {
                                        alert('Invalid theme file format');
                                      }
                                    } catch (error) {
                                      alert('Failed to parse theme file: ' + error);
                                    }
                                  };
                                  reader.readAsText(file);
                                }
                              };
                              input.click();
                            }}
                            className="px-4 py-2 rounded border border-[#555] bg-[#3c3c3c] text-gray-200 text-sm hover:border-[#666] hover:text-white transition-colors"
                          >
                            📥 Import Theme
                          </button>
                        </div>
                        <p className="mt-2 text-xs text-gray-500">Export themes to share or backup, import custom themes from JSON files</p>
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
                <div className="space-y-5">
                  <div>
                    <h2 className="text-base font-semibold mb-3 text-gray-100">Plugin Settings</h2>
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
                <div className="space-y-5">
                  <div>
                    <h2 className="text-base font-semibold mb-3 text-gray-100">Advanced Settings</h2>
                    
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
    </>
  );
};
