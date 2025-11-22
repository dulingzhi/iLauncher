import { useEffect } from 'react';
import { useConfigStore } from '../store/useConfigStore';
import { applyTheme, getTheme, Theme } from '../theme';

/**
 * 统一的主题配置Hook
 * 将theme、外观、字体配置整合到一起
 */
export const useThemeConfig = () => {
  const { config, saveConfig } = useConfigStore();

  // 应用主题配置
  useEffect(() => {
    if (!config) return;

    // 应用主题颜色
    let theme: Theme;
    if (config.appearance.custom_theme) {
      // 使用自定义主题
      theme = config.appearance.custom_theme;
    } else {
      // 使用预设主题
      theme = getTheme(config.appearance.theme);
    }
    applyTheme(theme);

    // 应用字体配置
    if (config.font) {
      const root = document.documentElement;
      root.style.setProperty('--font-family', config.font.font_family);
      root.style.setProperty('--font-size', `${config.font.font_size}px`);
      root.style.setProperty('--line-height', config.font.line_height.toString());
      root.style.setProperty('--letter-spacing', `${config.font.letter_spacing}em`);
      root.style.setProperty('--font-weight', config.font.font_weight.toString());
      root.style.setProperty('--title-size', `${config.font.title_size}px`);
      root.style.setProperty('--subtitle-size', `${config.font.subtitle_size}px`);
    }

    // 应用窗口透明度
    if (config.appearance.transparency !== undefined) {
      const root = document.documentElement;
      root.style.setProperty('--window-opacity', (config.appearance.transparency / 100).toString());
    }
  }, [config]);

  // 设置预设主题
  const setTheme = async (themeName: string) => {
    if (!config) return;

    const newConfig = {
      ...config,
      appearance: {
        ...config.appearance,
        theme: themeName,
        custom_theme: undefined, // 清除自定义主题
      },
    };

    await saveConfig(newConfig);
  };

  // 设置自定义主题
  const setCustomTheme = async (theme: Theme) => {
    if (!config) return;

    const newConfig = {
      ...config,
      appearance: {
        ...config.appearance,
        theme: 'custom',
        custom_theme: theme,
      },
    };

    await saveConfig(newConfig);
  };

  // 更新字体配置
  const updateFont = async (fontConfig: Partial<NonNullable<typeof config>['font']>) => {
    if (!config) return;

    const newConfig = {
      ...config,
      font: {
        ...config.font,
        ...fontConfig,
      },
    };

    await saveConfig(newConfig);
  };

  // 更新外观配置
  const updateAppearance = async (appearanceConfig: Partial<NonNullable<typeof config>['appearance']>) => {
    if (!config) return;

    const newConfig = {
      ...config,
      appearance: {
        ...config.appearance,
        ...appearanceConfig,
      },
    };

    await saveConfig(newConfig);
  };

  return {
    config,
    setTheme,
    setCustomTheme,
    updateFont,
    updateAppearance,
  };
};
