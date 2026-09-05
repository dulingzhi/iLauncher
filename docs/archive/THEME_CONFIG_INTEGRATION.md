# Theme、Appearance 和 Font 配置整合方案

## 概述

本文档说明了如何将 theme 系统、外观设置和字体配置整合到统一的配置管理系统中。

## 配置结构

### 统一的 AppConfig 接口

```typescript
interface AppConfig {
  appearance: {
    // 主题配置
    theme: string;  // 预设主题名称: 'dark', 'light', 'blue', etc.
    custom_theme?: {  // 自定义主题（可选）
      name: string;
      colors: {
        primary: string;
        secondary: string;
        background: string;
        surface: string;
        text: { primary, secondary, muted };
        border: string;
        hover: string;
        accent: string;
        primaryAlpha: string;
      };
    };
    // 窗口设置
    show_preview: boolean;
    window_width: number;
    window_height: number;
    transparency: number;
  };
  
  font: {
    font_family: string;
    font_size: number;
    line_height: number;
    letter_spacing: number;
    font_weight: number;
    title_size: number;
    subtitle_size: number;
  };
}
```

## 配置管理方式

### 1. 使用统一的 useConfigStore

所有配置通过 `useConfigStore` 进行管理：

```typescript
const { config, saveConfig } = useConfigStore();

// 更新主题
await saveConfig({
  ...config,
  appearance: { ...config.appearance, theme: 'dark' }
});

// 更新字体
await saveConfig({
  ...config,
  font: { ...config.font, font_size: 16 }
});
```

### 2. 使用便捷的 useThemeConfig Hook

提供了 `useThemeConfig` Hook 来简化主题相关操作：

```typescript
const { setTheme, setCustomTheme, updateFont, updateAppearance } = useThemeConfig();

// 切换预设主题
await setTheme('dracula');

// 应用自定义主题
await setCustomTheme(customThemeObject);

// 更新字体
await updateFont({ font_size: 16, line_height: 1.6 });

// 更新外观
await updateAppearance({ transparency: 95, window_width: 900 });
```

## 配置应用流程

```
用户修改设置
    ↓
保存到 useConfigStore
    ↓
触发 useEffect (监听 config 变化)
    ↓
├─ 应用主题颜色 (applyTheme)
├─ 应用字体配置 (CSS variables)
└─ 应用窗口设置
    ↓
保存到后端配置文件
```

## 优势

1. **统一管理**：所有UI相关配置在一个地方管理
2. **自动同步**：配置改变自动应用到界面
3. **持久化**：自动保存到后端配置文件
4. **类型安全**：完整的 TypeScript 类型定义
5. **易于扩展**：添加新配置项只需更新接口

## 组件使用示例

### Settings组件

```typescript
const { config, saveConfig } = useConfigStore();

// 切换主题
setConfig({
  ...config,
  appearance: { ...config.appearance, theme: 'nord' }
});
await saveConfig(config);
```

### FontSettings组件

```typescript
const { config, saveConfig } = useConfigStore();

// 更新字体配置
await saveConfig({
  ...config,
  font: {
    ...config.font,
    font_size: 16,
    line_height: 1.6
  }
});
```

### App组件（初始化）

```typescript
const { config } = useConfigStore();
const { setTheme } = useThemeStore();

useEffect(() => {
  if (config) {
    // 应用主题
    if (config.appearance.custom_theme) {
      applyTheme(config.appearance.custom_theme);
    } else {
      setTheme(config.appearance.theme);
    }
  }
}, [config]);
```

## 迁移步骤

对于现有组件的迁移：

1. 移除独立的 localStorage 操作
2. 使用 `useConfigStore` 获取和保存配置
3. 监听 `config` 变化来应用设置
4. 删除冗余的配置状态管理

## 后端配置格式

配置文件（config.json）的格式：

```json
{
  "appearance": {
    "theme": "dark",
    "window_width": 800,
    "window_height": 600,
    "transparency": 95,
    "show_preview": true
  },
  "font": {
    "font_family": "system-ui",
    "font_size": 14,
    "line_height": 1.5,
    "letter_spacing": 0,
    "font_weight": 400,
    "title_size": 14,
    "subtitle_size": 12
  }
}
```

## 注意事项

1. **配置初始化**：App 启动时需要调用 `loadConfig()` 加载配置
2. **默认值**：后端应提供合理的默认配置值
3. **配置验证**：保存前应验证配置值的有效性
4. **错误处理**：配置加载失败时使用默认配置
