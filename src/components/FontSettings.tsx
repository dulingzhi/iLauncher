import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Type, AlignLeft, Minus, Plus } from 'lucide-react';

interface FontConfig {
  fontFamily: string;
  fontSize: number;        // 12-24px
  lineHeight: number;      // 1.0-2.0
  letterSpacing: number;   // -0.05 - 0.2em
  fontWeight: number;      // 300-700
  titleSize: number;       // 12-20px
  subtitleSize: number;    // 10-16px
}

const DEFAULT_FONT_CONFIG: FontConfig = {
  fontFamily: 'system-ui',
  fontSize: 14,
  lineHeight: 1.5,
  letterSpacing: 0,
  fontWeight: 400,
  titleSize: 14,
  subtitleSize: 12,
};

const FONT_FAMILIES = [
  { name: 'System Default', value: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif' },
  { name: 'Segoe UI', value: '"Segoe UI", Tahoma, Geneva, Verdana, sans-serif' },
  { name: 'Arial', value: 'Arial, Helvetica, sans-serif' },
  { name: 'Microsoft YaHei', value: '"Microsoft YaHei", 微软雅黑, sans-serif' },
  { name: 'SimSun', value: 'SimSun, 宋体, serif' },
  { name: 'Consolas', value: 'Consolas, "Courier New", monospace' },
  { name: 'Fira Code', value: '"Fira Code", "Cascadia Code", Consolas, monospace' },
  { name: 'JetBrains Mono', value: '"JetBrains Mono", Consolas, monospace' },
  { name: 'Roboto', value: 'Roboto, sans-serif' },
  { name: 'Inter', value: 'Inter, sans-serif' },
];

export const FontSettings: React.FC = () => {
  const [config, setConfig] = useState<FontConfig>(DEFAULT_FONT_CONFIG);

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      // 从后端配置加载
      const backendConfig = await invoke<any>('get_config');
      if (backendConfig?.font) {
        const fontConfig: FontConfig = {
          fontFamily: backendConfig.font.font_family || DEFAULT_FONT_CONFIG.fontFamily,
          fontSize: backendConfig.font.font_size || 14,
          lineHeight: backendConfig.font.line_height || 1.5,
          letterSpacing: backendConfig.font.letter_spacing || 0,
          fontWeight: backendConfig.font.font_weight || 400,
          titleSize: backendConfig.font.title_size || 14,
          subtitleSize: backendConfig.font.subtitle_size || 12,
        };
        setConfig(fontConfig);
        applyConfig(fontConfig);
      } else {
        // 回退到localStorage
        const saved = localStorage.getItem('font_config');
        if (saved) {
          const loaded = JSON.parse(saved);
          setConfig(loaded);
          applyConfig(loaded);
        } else {
          applyConfig(DEFAULT_FONT_CONFIG);
        }
      }
    } catch (error) {
      console.error('Failed to load font config:', error);
      const saved = localStorage.getItem('font_config');
      if (saved) {
        setConfig(JSON.parse(saved));
      }
    }
  };

  const applyConfig = (cfg: FontConfig) => {
    const root = document.documentElement;
    
    root.style.setProperty('--font-family', cfg.fontFamily);
    root.style.setProperty('--font-size', `${cfg.fontSize}px`);
    root.style.setProperty('--line-height', cfg.lineHeight.toString());
    root.style.setProperty('--letter-spacing', `${cfg.letterSpacing}em`);
    root.style.setProperty('--font-weight', cfg.fontWeight.toString());
    root.style.setProperty('--title-size', `${cfg.titleSize}px`);
    root.style.setProperty('--subtitle-size', `${cfg.subtitleSize}px`);
  };

  const saveConfig = async (newConfig: FontConfig) => {
    try {
      // 同时保存到localStorage和后端配置
      localStorage.setItem('font_config', JSON.stringify(newConfig));
      setConfig(newConfig);
      applyConfig(newConfig);
      
      // 保存到后端配置文件
      try {
        const backendConfig = await invoke<any>('get_config');
        backendConfig.font = {
          font_family: newConfig.fontFamily,
          font_size: newConfig.fontSize,
          line_height: newConfig.lineHeight,
          letter_spacing: newConfig.letterSpacing,
          font_weight: newConfig.fontWeight,
          title_size: newConfig.titleSize,
          subtitle_size: newConfig.subtitleSize,
        };
        await invoke('save_config', { config: backendConfig });
      } catch (err) {
        console.warn('Failed to save to backend config:', err);
      }
    } catch (error) {
      console.error('Failed to save font config:', error);
    }
  };

  const handleChange = (key: keyof FontConfig, value: string | number) => {
    const newConfig = { ...config, [key]: value };
    saveConfig(newConfig);
  };

  const resetToDefault = () => {
    saveConfig(DEFAULT_FONT_CONFIG);
  };

  return (
    <div className="space-y-6">
      {/* 字体族选择 */}
      <div>
        <label className="block text-sm font-medium mb-3 flex items-center gap-2" style={{ color: 'var(--color-text-primary)' }}>
          <Type className="w-4 h-4" />
          字体族
        </label>
        <select
          value={config.fontFamily}
          onChange={(e) => handleChange('fontFamily', e.target.value)}
          className="w-full px-3 py-2 rounded border transition-all focus:outline-none"
          style={{
            backgroundColor: 'var(--color-surface)',
            borderColor: 'var(--color-border)',
            color: 'var(--color-text-primary)'
          }}
        >
          {FONT_FAMILIES.map((font) => (
            <option key={font.value} value={font.value} style={{ fontFamily: font.value }}>
              {font.name}
            </option>
          ))}
        </select>
        <p className="mt-2 text-xs" style={{ color: 'var(--color-text-muted)' }}>
          选择应用程序使用的字体
        </p>
      </div>

      {/* 字体大小 */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <label className="text-sm font-medium flex items-center gap-2" style={{ color: 'var(--color-text-primary)' }}>
            <AlignLeft className="w-4 h-4" />
            基础字号
          </label>
          <div className="flex items-center gap-2">
            <button
              onClick={() => handleChange('fontSize', Math.max(12, config.fontSize - 1))}
              className="p-1 rounded transition-colors"
              style={{ color: 'var(--color-text-secondary)' }}
              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--color-hover)'}
              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
            >
              <Minus className="w-4 h-4" />
            </button>
            <span className="text-sm font-mono w-12 text-center" style={{ color: 'var(--color-text-secondary)' }}>
              {config.fontSize}px
            </span>
            <button
              onClick={() => handleChange('fontSize', Math.min(24, config.fontSize + 1))}
              className="p-1 rounded transition-colors"
              style={{ color: 'var(--color-text-secondary)' }}
              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--color-hover)'}
              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>
        </div>
        <input
          type="range"
          min={12}
          max={24}
          value={config.fontSize}
          onChange={(e) => handleChange('fontSize', Number(e.target.value))}
          className="w-full h-2 rounded-lg appearance-none cursor-pointer"
          style={{ accentColor: 'var(--color-primary)' }}
        />
      </div>

      {/* 标题字号 */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <label className="text-sm font-medium" style={{ color: 'var(--color-text-primary)' }}>
            标题字号
          </label>
          <span className="text-sm font-mono" style={{ color: 'var(--color-text-secondary)' }}>
            {config.titleSize}px
          </span>
        </div>
        <input
          type="range"
          min={12}
          max={20}
          value={config.titleSize}
          onChange={(e) => handleChange('titleSize', Number(e.target.value))}
          className="w-full h-2 rounded-lg appearance-none cursor-pointer"
          style={{ accentColor: 'var(--color-primary)' }}
        />
      </div>

      {/* 副标题字号 */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <label className="text-sm font-medium" style={{ color: 'var(--color-text-primary)' }}>
            副标题字号
          </label>
          <span className="text-sm font-mono" style={{ color: 'var(--color-text-secondary)' }}>
            {config.subtitleSize}px
          </span>
        </div>
        <input
          type="range"
          min={10}
          max={16}
          value={config.subtitleSize}
          onChange={(e) => handleChange('subtitleSize', Number(e.target.value))}
          className="w-full h-2 rounded-lg appearance-none cursor-pointer"
          style={{ accentColor: 'var(--color-primary)' }}
        />
      </div>

      {/* 行高 */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <label className="text-sm font-medium" style={{ color: 'var(--color-text-primary)' }}>
            行高
          </label>
          <span className="text-sm font-mono" style={{ color: 'var(--color-text-secondary)' }}>
            {config.lineHeight.toFixed(2)}
          </span>
        </div>
        <input
          type="range"
          min={1.0}
          max={2.0}
          step={0.1}
          value={config.lineHeight}
          onChange={(e) => handleChange('lineHeight', Number(e.target.value))}
          className="w-full h-2 rounded-lg appearance-none cursor-pointer"
          style={{ accentColor: 'var(--color-primary)' }}
        />
      </div>

      {/* 字间距 */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <label className="text-sm font-medium" style={{ color: 'var(--color-text-primary)' }}>
            字间距
          </label>
          <span className="text-sm font-mono" style={{ color: 'var(--color-text-secondary)' }}>
            {config.letterSpacing.toFixed(2)}em
          </span>
        </div>
        <input
          type="range"
          min={-0.05}
          max={0.2}
          step={0.01}
          value={config.letterSpacing}
          onChange={(e) => handleChange('letterSpacing', Number(e.target.value))}
          className="w-full h-2 rounded-lg appearance-none cursor-pointer"
          style={{ accentColor: 'var(--color-primary)' }}
        />
      </div>

      {/* 字重 */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <label className="text-sm font-medium" style={{ color: 'var(--color-text-primary)' }}>
            字重
          </label>
          <span className="text-sm font-mono" style={{ color: 'var(--color-text-secondary)' }}>
            {config.fontWeight}
          </span>
        </div>
        <div className="flex gap-2">
          {[300, 400, 500, 600, 700].map((weight) => (
            <button
              key={weight}
              onClick={() => handleChange('fontWeight', weight)}
              className="flex-1 px-3 py-2 rounded border transition-colors"
              style={{
                fontWeight: weight,
                borderColor: config.fontWeight === weight ? 'var(--color-primary)' : 'var(--color-border)',
                backgroundColor: config.fontWeight === weight ? 'var(--color-primary-alpha)' : 'var(--color-surface)',
                color: config.fontWeight === weight ? 'var(--color-text-primary)' : 'var(--color-text-secondary)'
              }}
            >
              {weight}
            </button>
          ))}
        </div>
      </div>

      {/* 预览 */}
      <div className="p-4 rounded-lg space-y-3" style={{ backgroundColor: 'var(--color-background)' }}>
        <h3 className="text-sm font-semibold mb-3" style={{ color: 'var(--color-text-primary)' }}>预览</h3>
        <div
          className="space-y-2"
          style={{
            fontFamily: config.fontFamily,
            fontSize: `${config.fontSize}px`,
            lineHeight: config.lineHeight,
            letterSpacing: `${config.letterSpacing}em`,
            fontWeight: config.fontWeight,
            color: 'var(--color-text-primary)'
          }}
        >
          <div style={{ fontSize: `${config.titleSize}px`, fontWeight: 600 }}>
            这是标题文本 This is Title Text
          </div>
          <div style={{ fontSize: `${config.subtitleSize}px`, color: 'var(--color-text-muted)' }}>
            这是副标题文本 This is subtitle text
          </div>
          <div>
            The quick brown fox jumps over the lazy dog. 
            敏捷的棕色狐狸跳过了懒狗。
          </div>
        </div>
      </div>

      {/* 重置按钮 */}
      <button
        onClick={resetToDefault}
        className="w-full px-4 py-2 rounded transition-colors"
        style={{
          backgroundColor: 'var(--color-surface)',
          color: 'var(--color-text-secondary)'
        }}
        onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--color-hover)'}
        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--color-surface)'}
      >
        恢复默认字体设置
      </button>
    </div>
  );
};
