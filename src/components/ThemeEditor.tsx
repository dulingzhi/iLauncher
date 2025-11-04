import React, { useState } from 'react';
import { HexColorPicker } from 'react-colorful';
import { Theme } from '../theme';
import { X, Eye, Save, Palette } from 'lucide-react';

interface ThemeEditorProps {
  initialTheme?: Theme;
  onSave: (theme: Theme) => void;
  onClose: () => void;
}

export const ThemeEditor: React.FC<ThemeEditorProps> = ({ initialTheme, onSave, onClose }) => {
  const [themeName, setThemeName] = useState(initialTheme?.name || 'Custom Theme');
  const [colors, setColors] = useState(initialTheme?.colors || {
    primary: '#60a5fa',
    secondary: '#a78bfa',
    background: '#0f172a',
    surface: '#1e293b',
    text: {
      primary: '#f1f5f9',
      secondary: '#cbd5e1',
      muted: '#94a3b8',
    },
    border: '#334155',
    hover: '#2d3748',
    accent: '#60a5fa',
    primaryAlpha: 'rgba(96, 165, 250, 0.25)',
  });

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
    };
    onSave(theme);
  };

  const applyPreview = () => {
    // 应用预览到 CSS 变量
    document.documentElement.style.setProperty('--color-primary', colors.primary);
    document.documentElement.style.setProperty('--color-secondary', colors.secondary);
    document.documentElement.style.setProperty('--color-background', colors.background);
    document.documentElement.style.setProperty('--color-surface', colors.surface);
    document.documentElement.style.setProperty('--color-text-primary', colors.text.primary);
    document.documentElement.style.setProperty('--color-text-secondary', colors.text.secondary);
    document.documentElement.style.setProperty('--color-text-muted', colors.text.muted);
    document.documentElement.style.setProperty('--color-border', colors.border);
    document.documentElement.style.setProperty('--color-hover', colors.hover);
    document.documentElement.style.setProperty('--color-accent', colors.accent);
    document.documentElement.style.setProperty('--color-primary-alpha', colors.primaryAlpha);
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
      <div className="bg-[#1e1e1e] rounded-lg shadow-2xl w-full max-w-4xl max-h-[90vh] overflow-hidden flex flex-col">
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
        <div className="px-6 py-4 border-b border-[#3e3e42]">
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

        {/* 内容区域 */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="grid grid-cols-2 gap-6">
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
