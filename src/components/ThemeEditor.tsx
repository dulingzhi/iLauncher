import React, { useState } from 'react';
import { HexColorPicker } from 'react-colorful';
import { Theme, themes, getTheme } from '../theme';
import { X, Eye, Save, Palette, Copy, Wand2, RefreshCw, Type, Layout, Sliders } from 'lucide-react';

interface ThemeEditorProps {
  initialTheme?: Theme;
  onSave: (theme: Theme) => void;
  onClose: () => void;
}

// 预设调色板
const COLOR_PRESETS = {
  blues: ['#1e3a8a', '#3b82f6', '#60a5fa', '#93c5fd', '#dbeafe'],
  purples: ['#581c87', '#a855f7', '#c084fc', '#e9d5ff', '#f3e8ff'],
  greens: ['#14532d', '#22c55e', '#4ade80', '#86efac', '#dcfce7'],
  reds: ['#7f1d1d', '#ef4444', '#f87171', '#fca5a5', '#fee2e2'],
  grays: ['#0f172a', '#1e293b', '#475569', '#94a3b8', '#e2e8f0'],
  warm: ['#7c2d12', '#ea580c', '#fb923c', '#fdba74', '#fed7aa'],
  cool: ['#164e63', '#0891b2', '#06b6d4', '#67e8f9', '#cffafe'],
};

const FONT_FAMILIES = [
  'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  '"Segoe UI", Tahoma, Geneva, Verdana, sans-serif',
  'Arial, Helvetica, sans-serif',
  '"Microsoft YaHei", 微软雅黑, sans-serif',
  'Consolas, "Courier New", monospace',
  '"Fira Code", monospace',
  'Inter, sans-serif',
];

export const ThemeEditor: React.FC<ThemeEditorProps> = ({ initialTheme, onSave, onClose }) => {
  const [themeName, setThemeName] = useState(initialTheme?.name || 'Custom Theme');
  const [colors, setColors] = useState(initialTheme?.colors || getTheme('dark').colors);
  const [appearance, setAppearance] = useState(initialTheme?.appearance || {
    window_width: 800,
    window_height: 600,
    transparency: 95,
    border_radius: 12,
    blur_strength: 10,
  });
  const [font, setFont] = useState(initialTheme?.font || {
    font_family: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
    font_size: 14,
    line_height: 1.5,
    letter_spacing: 0,
    font_weight: 400,
    title_size: 14,
    subtitle_size: 12,
  });
  const [activeTab, setActiveTab] = useState<'colors' | 'appearance' | 'font'>('colors');
  const [activeColorPicker, setActiveColorPicker] = useState<string | null>(null);
  const [previewMode, setPreviewMode] = useState(false);

  const colorFields = [
    { key: 'primary', label: 'Primary Color', path: ['primary'] },
    { key: 'secondary', label: 'Secondary Color', path: ['secondary'] },
    { key: 'background', label: 'Background', path: ['background'] },
    { key: 'surface', label: 'Surface', path: ['surface'] },
    { key: 'text.primary', label: 'Text Primary', path: ['text', 'primary'] },
    { key: 'text.secondary', label: 'Text Secondary', path: ['text', 'secondary'] },
    { key: 'text.muted', label: 'Text Muted', path: ['text', 'muted'] },
    { key: 'border', label: 'Border', path: ['border'] },
    { key: 'hover', label: 'Hover', path: ['hover'] },
    { key: 'accent', label: 'Accent', path: ['accent'] },
  ];

  const getColorValue = (path: string[]): string => {
    let value: any = colors;
    for (const key of path) {
      value = value[key];
    }
    return value;
  };

  const setColorValue = (path: string[], color: string) => {
    const newColors = { ...colors };
    let current: any = newColors;
    
    for (let i = 0; i < path.length - 1; i++) {
      current[path[i]] = { ...current[path[i]] };
      current = current[path[i]];
    }
    
    current[path[path.length - 1]] = color;
    
    // 自动更新 primaryAlpha
    if (path[0] === 'primary') {
      const rgb = hexToRgb(color);
      if (rgb) {
        newColors.primaryAlpha = `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.25)`;
      }
    }
    
    setColors(newColors);
  };

  const hexToRgb = (hex: string): { r: number; g: number; b: number } | null => {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result ? {
      r: parseInt(result[1], 16),
      g: parseInt(result[2], 16),
      b: parseInt(result[3], 16)
    } : null;
  };

  const handleSave = () => {
    const theme: Theme = {
      name: themeName.toLowerCase().replace(/\s+/g, '_'),
      colors,
      appearance,
      font,
    };
    onSave(theme);
  };

  const applyPreview = () => {
    const root = document.documentElement;
    
    // 应用颜色
    root.style.setProperty('--color-primary', colors.primary);
    root.style.setProperty('--color-secondary', colors.secondary);
    root.style.setProperty('--color-background', colors.background);
    root.style.setProperty('--color-surface', colors.surface);
    root.style.setProperty('--color-text-primary', colors.text.primary);
    root.style.setProperty('--color-text-secondary', colors.text.secondary);
    root.style.setProperty('--color-text-muted', colors.text.muted);
    root.style.setProperty('--color-border', colors.border);
    root.style.setProperty('--color-hover', colors.hover);
    root.style.setProperty('--color-accent', colors.accent);
    root.style.setProperty('--color-primary-alpha', colors.primaryAlpha);
    
    // 应用外观
    root.style.setProperty('--window-opacity', (appearance.transparency / 100).toString());
    root.style.setProperty('--border-radius', `${appearance.border_radius}px`);
    root.style.setProperty('--blur-strength', `${appearance.blur_strength}px`);
    
    // 应用字体
    root.style.setProperty('--font-family', font.font_family);
    root.style.setProperty('--font-size', `${font.font_size}px`);
    root.style.setProperty('--line-height', font.line_height.toString());
    root.style.setProperty('--letter-spacing', `${font.letter_spacing}em`);
    root.style.setProperty('--font-weight', font.font_weight.toString());
    root.style.setProperty('--title-size', `${font.title_size}px`);
    root.style.setProperty('--subtitle-size', `${font.subtitle_size}px`);
  };
  
  // 生成随机主题
  const generateRandomTheme = () => {
    const randomColor = () => {
      const hue = Math.floor(Math.random() * 360);
      const sat = 60 + Math.floor(Math.random() * 30);
      const light = 50 + Math.floor(Math.random() * 20);
      return `hsl(${hue}, ${sat}%, ${light}%)`;
    };
    
    const primary = randomColor();
    const secondary = randomColor();
    
    setColors({
      primary,
      secondary,
      background: '#0f172a',
      surface: '#1e293b',
      text: {
        primary: '#f1f5f9',
        secondary: '#cbd5e1',
        muted: '#94a3b8',
      },
      border: '#334155',
      hover: '#2d3748',
      accent: primary,
      primaryAlpha: `${primary.replace(')', ', 0.25)')}`,
    });
    
    setThemeName('Random ' + Math.random().toString(36).substring(7));
  };
  
  // 重置为默认
  const resetToDefault = () => {
    setColors(getTheme('dark').colors);
    setThemeName('Custom Theme');
  };
  
  // 复制主题JSON
  const copyThemeJson = () => {
    const theme: Theme = {
      name: themeName.toLowerCase().replace(/\s+/g, '_'),
      colors,
    };
    
    const json = JSON.stringify(theme, null, 2);
    navigator.clipboard.writeText(json);
    alert('Theme JSON copied to clipboard!');
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-6">
      <div className="bg-[#1e1e1e] rounded-lg shadow-2xl w-full max-w-6xl max-h-[95vh] overflow-hidden flex flex-col">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#2d2d30] border-b border-[#3e3e42]">
          <div className="flex items-center gap-3">
            <Palette className="w-5 h-5" style={{ color: colors.primary }} />
            <h2 className="text-lg font-semibold text-gray-100">Theme Editor</h2>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-[#3e3e42] rounded transition-colors"
          >
            <X className="w-5 h-5 text-gray-400" />
          </button>
        </div>

        {/* 主题名称 */}
        <div className="px-8 py-5 border-b border-[#3e3e42] space-y-5">
          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              Theme Name
            </label>
            <input
              type="text"
              value={themeName}
              onChange={(e) => setThemeName(e.target.value)}
              className="w-full px-3 py-2 bg-[#2d2d30] border border-[#3e3e42] rounded text-gray-100 focus:outline-none focus:border-[#007acc]"
              placeholder="Enter theme name"
            />
          </div>
          
          {/* 主题预设选择器 */}
          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              Load from Preset
            </label>
            <div className="flex gap-2 flex-wrap">
              {Object.keys(themes).slice(0, 8).map((presetName) => {
                const preset = themes[presetName];
                return (
                  <button
                    key={presetName}
                    onClick={() => {
                      setColors(preset.colors);
                      setThemeName(`${preset.name} Custom`);
                    }}
                    className="px-3 py-1.5 text-xs rounded border border-[#3e3e42] hover:border-[#007acc] transition-colors flex items-center gap-2"
                    style={{
                      background: `linear-gradient(135deg, ${preset.colors.primary}, ${preset.colors.secondary})`,
                      color: '#fff',
                    }}
                  >
                    {preset.name}
                  </button>
                );
              })}
            </div>
          </div>
          
          {/* 快速调色板 */}
          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2 flex items-center gap-2">
              <Palette className="w-4 h-4" />
              Quick Color Palette
            </label>
            <div className="grid grid-cols-7 gap-2">
              {Object.entries(COLOR_PRESETS).map(([name, palette]) => (
                <div key={name} className="flex flex-col items-center gap-1">
                  <div className="flex gap-0.5">
                    {palette.slice(0, 3).map((color, i) => (
                      <button
                        key={i}
                        onClick={() => setColorValue(['primary'], color)}
                        className="w-5 h-5 rounded border border-gray-600 hover:scale-110 transition-transform"
                        style={{ backgroundColor: color }}
                        title={`Apply ${name} - ${color}`}
                      />
                    ))}
                  </div>
                  <span className="text-xs text-gray-500 capitalize">{name}</span>
                </div>
              ))}
            </div>
          </div>
          
          {/* 随机生成 */}
          <div className="flex gap-2">
            <button
              onClick={generateRandomTheme}
              className="flex-1 px-3 py-2 bg-[#2d2d30] border border-[#3e3e42] rounded text-sm text-gray-300 hover:bg-[#3e3e42] transition-colors flex items-center justify-center gap-2"
            >
              <Wand2 className="w-4 h-4" />
              Generate Random
            </button>
            <button
              onClick={resetToDefault}
              className="flex-1 px-3 py-2 bg-[#2d2d30] border border-[#3e3e42] rounded text-sm text-gray-300 hover:bg-[#3e3e42] transition-colors flex items-center justify-center gap-2"
            >
              <RefreshCw className="w-4 h-4" />
              Reset
            </button>
            <button
              onClick={copyThemeJson}
              className="flex-1 px-3 py-2 bg-[#2d2d30] border border-[#3e3e42] rounded text-sm text-gray-300 hover:bg-[#3e3e42] transition-colors flex items-center justify-center gap-2"
            >
              <Copy className="w-4 h-4" />
              Copy JSON
            </button>
          </div>
        </div>

        {/* 内容区域 */}
        <div className="flex-1 overflow-hidden flex flex-col">
          {/* 标签页导航 */}
          <div className="flex border-b border-[#3e3e42] px-8">
            <button
              onClick={() => setActiveTab('colors')}
              className={`px-5 py-3.5 text-sm font-medium border-b-2 transition-colors flex items-center gap-2 ${
                activeTab === 'colors'
                  ? 'border-[#007acc] text-[#007acc]'
                  : 'border-transparent text-gray-400 hover:text-gray-300'
              }`}
            >
              <Palette className="w-4 h-4" />
              Colors
            </button>
            <button
              onClick={() => setActiveTab('appearance')}
              className={`px-4 py-3 text-sm font-medium border-b-2 transition-colors flex items-center gap-2 ${
                activeTab === 'appearance'
                  ? 'border-[#007acc] text-[#007acc]'
                  : 'border-transparent text-gray-400 hover:text-gray-300'
              }`}
            >
              <Layout className="w-4 h-4" />
              Appearance
            </button>
            <button
              onClick={() => setActiveTab('font')}
              className={`px-4 py-3 text-sm font-medium border-b-2 transition-colors flex items-center gap-2 ${
                activeTab === 'font'
                  ? 'border-[#007acc] text-[#007acc]'
                  : 'border-transparent text-gray-400 hover:text-gray-300'
              }`}
            >
              <Type className="w-4 h-4" />
              Font
            </button>
          </div>

          {/* 标签页内容 */}
          <div className="flex-1 overflow-y-auto p-8">
            {/* 颜色配置 */}
            {activeTab === 'colors' && (
              <div className="grid grid-cols-2 gap-8">
                {/* 左侧：颜色编辑器 */}
                <div className="space-y-4">
                  <h3 className="text-sm font-semibold text-gray-100 mb-3">Colors</h3>
                  {colorFields.map((field) => {
                    const colorValue = getColorValue(field.path);
                    const isActive = activeColorPicker === field.key;
                    
                    return (
                      <div key={field.key} className="relative">
                        <label className="block text-xs font-medium text-gray-400 mb-1.5">
                          {field.label}
                        </label>
                        <div className="flex gap-2">
                          <button
                            onClick={() => setActiveColorPicker(isActive ? null : field.key)}
                            className="w-12 h-10 rounded border-2 border-[#3e3e42] hover:border-[#007acc] transition-colors flex-shrink-0"
                        style={{ backgroundColor: colorValue }}
                      />
                      <input
                        type="text"
                        value={colorValue}
                        onChange={(e) => setColorValue(field.path, e.target.value)}
                        className="flex-1 px-3 py-2 bg-[#2d2d30] border border-[#3e3e42] rounded text-gray-100 text-sm focus:outline-none focus:border-[#007acc]"
                      />
                    </div>
                    
                    {/* 颜色选择器弹出层 */}
                    {isActive && (
                      <div className="absolute top-full left-0 mt-2 z-10 p-3 bg-[#252526] border border-[#3e3e42] rounded-lg shadow-xl">
                        <HexColorPicker
                          color={colorValue}
                          onChange={(color) => setColorValue(field.path, color)}
                        />
                        <button
                          onClick={() => setActiveColorPicker(null)}
                          className="mt-2 w-full px-3 py-1.5 bg-[#007acc] text-white text-xs rounded hover:bg-[#005a9e] transition-colors"
                        >
                          Done
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>

            {/* 右侧：实时预览 */}
            <div className="space-y-4">
              <div className="flex items-center justify-between mb-3">
                <h3 className="text-sm font-semibold text-gray-100">Preview</h3>
                <button
                  onClick={() => {
                    setPreviewMode(!previewMode);
                    if (!previewMode) {
                      applyPreview();
                    }
                  }}
                  className={`px-3 py-1.5 text-xs rounded transition-colors flex items-center gap-1.5 ${
                    previewMode ? 'bg-[#007acc] text-white' : 'bg-[#2d2d30] text-gray-300 hover:bg-[#3e3e42]'
                  }`}
                >
                  <Eye className="w-3.5 h-3.5" />
                  {previewMode ? 'Previewing' : 'Preview'}
                </button>
              </div>

              {/* 预览面板 */}
              <div className="space-y-3">
                {/* 搜索框预览 */}
                <div
                  className="rounded-lg p-4"
                  style={{ backgroundColor: colors.surface }}
                >
                  <div className="flex items-center gap-2 mb-3">
                    <div className="w-4 h-4 rounded" style={{ backgroundColor: colors.primary }} />
                    <input
                      type="text"
                      placeholder="Search..."
                      readOnly
                      className="flex-1 px-3 py-2 rounded border"
                      style={{
                        backgroundColor: colors.background,
                        borderColor: colors.border,
                        color: colors.text.primary,
                      }}
                    />
                  </div>

                  {/* 搜索结果预览 */}
                  <div className="space-y-1.5">
                    {[1, 2, 3].map((i) => (
                      <div
                        key={i}
                        className="p-2.5 rounded"
                        style={{
                          backgroundColor: i === 1 ? colors.primaryAlpha : 'transparent',
                        }}
                      >
                        <div
                          className="text-sm font-medium mb-0.5"
                          style={{ color: colors.text.primary }}
                        >
                          Result Item {i}
                        </div>
                        <div
                          className="text-xs"
                          style={{ color: colors.text.secondary }}
                        >
                          Subtitle information
                        </div>
                      </div>
                    ))}
                  </div>
                </div>

                {/* 颜色图例 */}
                <div
                  className="rounded-lg p-4 space-y-2"
                  style={{ backgroundColor: colors.surface }}
                >
                  <div className="text-xs font-semibold mb-2" style={{ color: colors.text.primary }}>
                    Color Legend
                  </div>
                  <div className="grid grid-cols-2 gap-2 text-xs">
                    <div className="flex items-center gap-2">
                      <div className="w-4 h-4 rounded" style={{ backgroundColor: colors.primary }} />
                      <span style={{ color: colors.text.secondary }}>Primary</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <div className="w-4 h-4 rounded" style={{ backgroundColor: colors.secondary }} />
                      <span style={{ color: colors.text.secondary }}>Secondary</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <div className="w-4 h-4 rounded" style={{ backgroundColor: colors.accent }} />
                      <span style={{ color: colors.text.secondary }}>Accent</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <div className="w-4 h-4 rounded" style={{ backgroundColor: colors.hover }} />
                      <span style={{ color: colors.text.secondary }}>Hover</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
            )}

            {/* 外观配置 */}
            {activeTab === 'appearance' && (
              <div className="max-w-3xl mx-auto space-y-6">
                <h3 className="text-base font-semibold text-gray-100 mb-4">Window Appearance</h3>
                
                {/* 窗口宽度 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Window Width</label>
                    <span className="text-sm text-gray-400">{appearance.window_width}px</span>
                  </div>
                  <input
                    type="range"
                    min="600"
                    max="1200"
                    value={appearance.window_width}
                    onChange={(e) => setAppearance({ ...appearance, window_width: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 窗口高度 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Window Height</label>
                    <span className="text-sm text-gray-400">{appearance.window_height}px</span>
                  </div>
                  <input
                    type="range"
                    min="400"
                    max="800"
                    value={appearance.window_height}
                    onChange={(e) => setAppearance({ ...appearance, window_height: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 透明度 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Transparency</label>
                    <span className="text-sm text-gray-400">{appearance.transparency}%</span>
                  </div>
                  <input
                    type="range"
                    min="50"
                    max="100"
                    value={appearance.transparency}
                    onChange={(e) => setAppearance({ ...appearance, transparency: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 圆角大小 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Border Radius</label>
                    <span className="text-sm text-gray-400">{appearance.border_radius}px</span>
                  </div>
                  <input
                    type="range"
                    min="0"
                    max="20"
                    value={appearance.border_radius}
                    onChange={(e) => setAppearance({ ...appearance, border_radius: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 背景模糊 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Background Blur</label>
                    <span className="text-sm text-gray-400">{appearance.blur_strength}px</span>
                  </div>
                  <input
                    type="range"
                    min="0"
                    max="20"
                    value={appearance.blur_strength}
                    onChange={(e) => setAppearance({ ...appearance, blur_strength: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 预览 */}
                <div className="mt-6 p-4 bg-[#252526] rounded-lg border border-[#3e3e42]">
                  <div className="text-xs font-semibold text-gray-300 mb-3">Preview</div>
                  <div 
                    className="p-4 bg-[#1e1e1e] border border-[#3e3e42] transition-all"
                    style={{
                      borderRadius: `${appearance.border_radius}px`,
                      opacity: appearance.transparency / 100,
                      backdropFilter: `blur(${appearance.blur_strength}px)`,
                    }}
                  >
                    <div className="text-sm text-gray-300">Window Preview</div>
                    <div className="text-xs text-gray-500 mt-1">
                      {appearance.window_width} × {appearance.window_height}px
                    </div>
                  </div>
                </div>
              </div>
            )}

            {/* 字体配置 */}
            {activeTab === 'font' && (
              <div className="max-w-3xl mx-auto space-y-6">
                <h3 className="text-base font-semibold text-gray-100 mb-4">Font Settings</h3>
                
                {/* 字体族 */}
                <div>
                  <label className="block text-sm font-medium text-gray-300 mb-2">Font Family</label>
                  <select
                    value={font.font_family}
                    onChange={(e) => setFont({ ...font, font_family: e.target.value })}
                    className="w-full px-3 py-2 bg-[#2d2d30] border border-[#3e3e42] rounded text-gray-100 focus:outline-none focus:border-[#007acc]"
                  >
                    {FONT_FAMILIES.map((family) => (
                      <option key={family} value={family} style={{ fontFamily: family }}>
                        {family.split(',')[0].replace(/"/g, '')}
                      </option>
                    ))}
                  </select>
                </div>

                {/* 基础字号 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Base Font Size</label>
                    <span className="text-sm text-gray-400">{font.font_size}px</span>
                  </div>
                  <input
                    type="range"
                    min="12"
                    max="24"
                    value={font.font_size}
                    onChange={(e) => setFont({ ...font, font_size: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 标题字号 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Title Size</label>
                    <span className="text-sm text-gray-400">{font.title_size}px</span>
                  </div>
                  <input
                    type="range"
                    min="12"
                    max="20"
                    value={font.title_size}
                    onChange={(e) => setFont({ ...font, title_size: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 副标题字号 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Subtitle Size</label>
                    <span className="text-sm text-gray-400">{font.subtitle_size}px</span>
                  </div>
                  <input
                    type="range"
                    min="10"
                    max="16"
                    value={font.subtitle_size}
                    onChange={(e) => setFont({ ...font, subtitle_size: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 行高 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Line Height</label>
                    <span className="text-sm text-gray-400">{font.line_height.toFixed(2)}</span>
                  </div>
                  <input
                    type="range"
                    min="1.0"
                    max="2.0"
                    step="0.1"
                    value={font.line_height}
                    onChange={(e) => setFont({ ...font, line_height: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 字间距 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm font-medium text-gray-300">Letter Spacing</label>
                    <span className="text-sm text-gray-400">{font.letter_spacing.toFixed(2)}em</span>
                  </div>
                  <input
                    type="range"
                    min="-0.05"
                    max="0.2"
                    step="0.01"
                    value={font.letter_spacing}
                    onChange={(e) => setFont({ ...font, letter_spacing: Number(e.target.value) })}
                    className="w-full h-2 bg-[#3e3e42] rounded-lg appearance-none cursor-pointer accent-[#007acc]"
                  />
                </div>

                {/* 字重 */}
                <div>
                  <label className="block text-sm font-medium text-gray-300 mb-2">Font Weight</label>
                  <div className="flex gap-2">
                    {[300, 400, 500, 600, 700].map((weight) => (
                      <button
                        key={weight}
                        onClick={() => setFont({ ...font, font_weight: weight })}
                        className={`flex-1 px-3 py-2 rounded border transition-colors ${
                          font.font_weight === weight
                            ? 'border-[#007acc] bg-[#007acc] bg-opacity-20 text-white'
                            : 'border-[#3e3e42] bg-[#2d2d30] text-gray-400 hover:border-[#007acc]'
                        }`}
                        style={{ fontWeight: weight }}
                      >
                        {weight}
                      </button>
                    ))}
                  </div>
                </div>

                {/* 字体预览 */}
                <div className="mt-6 p-4 bg-[#252526] rounded-lg border border-[#3e3e42]">
                  <div className="text-xs font-semibold text-gray-300 mb-3">Preview</div>
                  <div
                    className="space-y-2"
                    style={{
                      fontFamily: font.font_family,
                      fontSize: `${font.font_size}px`,
                      lineHeight: font.line_height,
                      letterSpacing: `${font.letter_spacing}em`,
                      fontWeight: font.font_weight,
                    }}
                  >
                    <div style={{ fontSize: `${font.title_size}px`, fontWeight: 600 }} className="text-gray-200">
                      This is Title Text 这是标题文本
                    </div>
                    <div style={{ fontSize: `${font.subtitle_size}px` }} className="text-gray-400">
                      This is subtitle text 这是副标题文本
                    </div>
                    <div className="text-gray-300">
                      The quick brown fox jumps over the lazy dog. 敏捷的棕色狐狸跳过了懒狗。
                    </div>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* 底部操作栏 */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#2d2d30] border-t border-[#3e3e42]">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-gray-300 hover:bg-[#3e3e42] rounded transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="px-4 py-2 text-sm bg-[#007acc] text-white rounded hover:bg-[#005a9e] transition-colors flex items-center gap-2"
          >
            <Save className="w-4 h-4" />
            Save Theme
          </button>
        </div>
      </div>
    </div>
  );
};
