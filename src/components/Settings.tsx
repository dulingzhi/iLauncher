import React, { useState } from 'react';

interface Settings {
  hotkey: string;
  theme: 'dark' | 'light';
  searchDelay: number;
  maxResults: number;
  windowWidth: number;
  windowHeight: number;
  fontSize: number;
}

export const Settings: React.FC = () => {
  const [settings, setSettings] = useState<Settings>({
    hotkey: 'Alt+Space',
    theme: 'dark',
    searchDelay: 100,
    maxResults: 10,
    windowWidth: 800,
    windowHeight: 500,
    fontSize: 14,
  });

  const [activeTab, setActiveTab] = useState<'general' | 'appearance' | 'plugins' | 'advanced'>('general');

  const handleSave = () => {
    // TODO: 保存设置到后端
    console.log('Saving settings:', settings);
  };

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
                    value={settings.hotkey}
                    onChange={(e) => setSettings({ ...settings, hotkey: e.target.value })}
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
                    value={settings.searchDelay}
                    onChange={(e) => setSettings({ ...settings, searchDelay: parseInt(e.target.value) })}
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
                    value={settings.maxResults}
                    onChange={(e) => setSettings({ ...settings, maxResults: parseInt(e.target.value) })}
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
                  <label className="block text-sm font-medium mb-2">Theme</label>
                  <div className="flex gap-4">
                    <button
                      onClick={() => setSettings({ ...settings, theme: 'dark' })}
                      className={`flex-1 py-3 rounded border-2 transition-colors ${
                        settings.theme === 'dark'
                          ? 'border-blue-500 bg-gray-700'
                          : 'border-gray-600 bg-gray-800 hover:border-gray-500'
                      }`}
                    >
                      🌙 Dark
                    </button>
                    <button
                      onClick={() => setSettings({ ...settings, theme: 'light' })}
                      className={`flex-1 py-3 rounded border-2 transition-colors ${
                        settings.theme === 'light'
                          ? 'border-blue-500 bg-gray-700'
                          : 'border-gray-600 bg-gray-800 hover:border-gray-500'
                      }`}
                    >
                      ☀️ Light
                    </button>
                  </div>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Window Width
                  </label>
                  <input
                    type="range"
                    value={settings.windowWidth}
                    onChange={(e) => setSettings({ ...settings, windowWidth: parseInt(e.target.value) })}
                    className="w-full"
                    min="600"
                    max="1200"
                    step="50"
                  />
                  <div className="text-sm text-gray-400 mt-1">{settings.windowWidth}px</div>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Window Height
                  </label>
                  <input
                    type="range"
                    value={settings.windowHeight}
                    onChange={(e) => setSettings({ ...settings, windowHeight: parseInt(e.target.value) })}
                    className="w-full"
                    min="400"
                    max="800"
                    step="50"
                  />
                  <div className="text-sm text-gray-400 mt-1">{settings.windowHeight}px</div>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">
                    Font Size
                  </label>
                  <input
                    type="range"
                    value={settings.fontSize}
                    onChange={(e) => setSettings({ ...settings, fontSize: parseInt(e.target.value) })}
                    className="w-full"
                    min="12"
                    max="20"
                  />
                  <div className="text-sm text-gray-400 mt-1">{settings.fontSize}px</div>
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
                  <input type="checkbox" className="sr-only peer" />
                  <div className="w-11 h-6 bg-gray-600 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
                </label>
              </div>

              <div className="flex items-center justify-between p-4 bg-gray-700 rounded">
                <div>
                  <div className="font-medium">Show Tray Icon</div>
                  <div className="text-sm text-gray-400">Display icon in system tray</div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input type="checkbox" className="sr-only peer" defaultChecked />
                  <div className="w-11 h-6 bg-gray-600 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
                </label>
              </div>

              <div className="flex items-center justify-between p-4 bg-gray-700 rounded">
                <div>
                  <div className="font-medium">Enable Analytics</div>
                  <div className="text-sm text-gray-400">Help improve iLauncher by sharing usage data</div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input type="checkbox" className="sr-only peer" />
                  <div className="w-11 h-6 bg-gray-600 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
                </label>
              </div>
            </div>
          </div>
        )}

        {/* 保存按钮 */}
        <div className="mt-8 flex justify-end gap-4">
          <button className="px-6 py-2 bg-gray-700 hover:bg-gray-600 rounded font-medium transition-colors">
            Reset
          </button>
          <button
            onClick={handleSave}
            className="px-6 py-2 bg-blue-600 hover:bg-blue-700 rounded font-medium transition-colors"
          >
            Save Settings
          </button>
        </div>
      </div>
    </div>
  );
};
