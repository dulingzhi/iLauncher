import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Sliders, Eye, Box, Droplets, Square, Sparkles } from 'lucide-react';

interface AppearanceConfig {
  opacity: number;          // 窗口透明度 0-100
  blur: number;            // 背景模糊 0-50
  borderRadius: number;    // 圆角大小 0-30
  shadowSize: number;      // 阴影大小 0-50
  resultHeight: number;    // 结果项高度 40-80
  maxResults: number;      // 最大结果数 5-20
  animationSpeed: number;  // 动画速度 0-500ms
  iconSize: number;        // 图标大小 16-48
}

const DEFAULT_CONFIG: AppearanceConfig = {
  opacity: 95,
  blur: 10,
  borderRadius: 12,
  shadowSize: 20,
  resultHeight: 60,
  maxResults: 8,
  animationSpeed: 200,
  iconSize: 32,
};

export const AppearanceSettings: React.FC = () => {
  const [config, setConfig] = useState<AppearanceConfig>(DEFAULT_CONFIG);
  const [isApplying, setIsApplying] = useState(false);

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      // 从后端配置加载
      const backendConfig = await invoke<any>('get_config');
      if (backendConfig?.ui) {
        const uiConfig: AppearanceConfig = {
          opacity: backendConfig.ui.opacity || 95,
          blur: backendConfig.ui.blur || 10,
          borderRadius: backendConfig.ui.border_radius || 12,
          shadowSize: backendConfig.ui.shadow_size || 20,
          resultHeight: backendConfig.ui.result_height || 60,
          maxResults: backendConfig.ui.max_results || 8,
          animationSpeed: backendConfig.ui.animation_speed || 200,
          iconSize: backendConfig.ui.icon_size || 32,
        };
        setConfig(uiConfig);
        applyConfig(uiConfig);
      } else {
        // 回退到localStorage
        const saved = localStorage.getItem('appearance_config');
        if (saved) {
          const loaded = JSON.parse(saved);
          setConfig(loaded);
          applyConfig(loaded);
        }
      }
    } catch (error) {
      console.error('Failed to load appearance config:', error);
      // 回退到localStorage
      const saved = localStorage.getItem('appearance_config');
      if (saved) {
        setConfig(JSON.parse(saved));
      }
    }
  };

  const applyConfig = (cfg: AppearanceConfig) => {
    const root = document.documentElement;
    
    // 应用窗口透明度
    root.style.setProperty('--window-opacity', (cfg.opacity / 100).toString());
    
    // 应用背景模糊
    root.style.setProperty('--backdrop-blur', `${cfg.blur}px`);
    
    // 应用圆角
    root.style.setProperty('--border-radius-lg', `${cfg.borderRadius}px`);
    root.style.setProperty('--border-radius-md', `${cfg.borderRadius * 0.75}px`);
    root.style.setProperty('--border-radius-sm', `${cfg.borderRadius * 0.5}px`);
    
    // 应用阴影
    const shadowBlur = cfg.shadowSize;
    const shadowSpread = cfg.shadowSize * 0.3;
    root.style.setProperty('--shadow-lg', `0 ${shadowBlur}px ${shadowBlur * 2}px -${shadowSpread}px rgba(0, 0, 0, 0.5)`);
    root.style.setProperty('--shadow-md', `0 ${shadowBlur * 0.5}px ${shadowBlur}px -${shadowSpread * 0.5}px rgba(0, 0, 0, 0.3)`);
    
    // 应用结果项高度
    root.style.setProperty('--result-height', `${cfg.resultHeight}px`);
    
    // 应用动画速度
    root.style.setProperty('--animation-duration', `${cfg.animationSpeed}ms`);
    
    // 应用图标大小
    root.style.setProperty('--icon-size', `${cfg.iconSize}px`);
    
    // 通知 Tauri 后端更新窗口效果
    setIsApplying(true);
    invoke('set_window_effects', {
      opacity: cfg.opacity,
      blur: cfg.blur > 0,
    }).catch(err => {
      console.warn('Failed to set window effects:', err);
    }).finally(() => {
      setTimeout(() => setIsApplying(false), 300);
    });
  };

  const saveConfig = async (newConfig: AppearanceConfig) => {
    try {
      // 同时保存到localStorage和后端配置
      localStorage.setItem('appearance_config', JSON.stringify(newConfig));
      setConfig(newConfig);
      applyConfig(newConfig);
      
      // 保存到后端配置文件
      try {
        const backendConfig = await invoke<any>('get_config');
        backendConfig.ui = {
          opacity: newConfig.opacity,
          blur: newConfig.blur,
          border_radius: newConfig.borderRadius,
          shadow_size: newConfig.shadowSize,
          result_height: newConfig.resultHeight,
          max_results: newConfig.maxResults,
          animation_speed: newConfig.animationSpeed,
          icon_size: newConfig.iconSize,
        };
        await invoke('save_config', { config: backendConfig });
      } catch (err) {
        console.warn('Failed to save to backend config:', err);
      }
    } catch (error) {
      console.error('Failed to save appearance config:', error);
    }
  };

  const handleChange = (key: keyof AppearanceConfig, value: number) => {
    const newConfig = { ...config, [key]: value };
    saveConfig(newConfig);
  };

  const resetToDefault = () => {
    saveConfig(DEFAULT_CONFIG);
  };

  const SliderControl: React.FC<{
    label: string;
    icon: React.ReactNode;
    value: number;
    min: number;
    max: number;
    step?: number;
    unit?: string;
    onChange: (value: number) => void;
  }> = ({ label, icon, value, min, max, step = 1, unit = '', onChange }) => (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm font-medium text-gray-300">
          {icon}
          <span>{label}</span>
        </div>
        <span className="text-sm text-gray-400 font-mono">
          {value}{unit}
        </span>
      </div>
      <div className="relative">
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => onChange(Number(e.target.value))}
          className="w-full h-2 bg-gray-700 rounded-lg appearance-none cursor-pointer slider"
          style={{
            background: `linear-gradient(to right, 
              var(--color-primary) 0%, 
              var(--color-primary) ${((value - min) / (max - min)) * 100}%, 
              rgb(55, 65, 81) ${((value - min) / (max - min)) * 100}%, 
              rgb(55, 65, 81) 100%)`
          }}
        />
      </div>
    </div>
  );

  return (
    <div className="p-6 space-y-6">
      {/* 标题 */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <Sliders className="w-5 h-5 text-blue-400" />
          <h2 className="text-xl font-semibold text-gray-100">外观设置</h2>
        </div>
        <button
          onClick={resetToDefault}
          className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 rounded transition-colors text-gray-300"
        >
          恢复默认
        </button>
      </div>

      {/* 应用中提示 */}
      {isApplying && (
        <div className="flex items-center gap-2 px-4 py-2 bg-blue-500 bg-opacity-20 border border-blue-500 rounded-lg">
          <Sparkles className="w-4 h-4 text-blue-400 animate-spin" />
          <span className="text-sm text-blue-300">应用设置中...</span>
        </div>
      )}

      {/* 窗口效果 */}
      <div className="space-y-4 p-4 bg-gray-800 bg-opacity-50 rounded-lg">
        <h3 className="text-sm font-semibold text-gray-200 flex items-center gap-2">
          <Eye className="w-4 h-4" />
          窗口效果
        </h3>
        
        <SliderControl
          label="窗口透明度"
          icon={<Droplets className="w-4 h-4" />}
          value={config.opacity}
          min={60}
          max={100}
          unit="%"
          onChange={(value) => handleChange('opacity', value)}
        />
        
        <SliderControl
          label="背景模糊"
          icon={<Droplets className="w-4 h-4" />}
          value={config.blur}
          min={0}
          max={50}
          unit="px"
          onChange={(value) => handleChange('blur', value)}
        />
      </div>

      {/* 形状和大小 */}
      <div className="space-y-4 p-4 bg-gray-800 bg-opacity-50 rounded-lg">
        <h3 className="text-sm font-semibold text-gray-200 flex items-center gap-2">
          <Box className="w-4 h-4" />
          形状和大小
        </h3>
        
        <SliderControl
          label="圆角大小"
          icon={<Square className="w-4 h-4" />}
          value={config.borderRadius}
          min={0}
          max={30}
          unit="px"
          onChange={(value) => handleChange('borderRadius', value)}
        />
        
        <SliderControl
          label="阴影大小"
          icon={<Box className="w-4 h-4" />}
          value={config.shadowSize}
          min={0}
          max={50}
          unit="px"
          onChange={(value) => handleChange('shadowSize', value)}
        />
        
        <SliderControl
          label="结果项高度"
          icon={<Box className="w-4 h-4" />}
          value={config.resultHeight}
          min={40}
          max={80}
          unit="px"
          onChange={(value) => handleChange('resultHeight', value)}
        />
        
        <SliderControl
          label="图标大小"
          icon={<Box className="w-4 h-4" />}
          value={config.iconSize}
          min={16}
          max={48}
          unit="px"
          onChange={(value) => handleChange('iconSize', value)}
        />
      </div>

      {/* 性能和行为 */}
      <div className="space-y-4 p-4 bg-gray-800 bg-opacity-50 rounded-lg">
        <h3 className="text-sm font-semibold text-gray-200 flex items-center gap-2">
          <Sparkles className="w-4 h-4" />
          性能和行为
        </h3>
        
        <SliderControl
          label="动画速度"
          icon={<Sparkles className="w-4 h-4" />}
          value={config.animationSpeed}
          min={0}
          max={500}
          step={50}
          unit="ms"
          onChange={(value) => handleChange('animationSpeed', value)}
        />
        
        <SliderControl
          label="最大结果数"
          icon={<Box className="w-4 h-4" />}
          value={config.maxResults}
          min={5}
          max={20}
          onChange={(value) => handleChange('maxResults', value)}
        />
      </div>

      {/* 实时预览提示 */}
      <div className="p-4 bg-blue-500 bg-opacity-10 border border-blue-500 border-opacity-30 rounded-lg">
        <p className="text-sm text-blue-300">
          💡 所有设置都会实时应用到界面上，您可以立即看到效果
        </p>
      </div>
    </div>
  );
};
