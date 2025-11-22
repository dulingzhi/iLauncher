// 主题系统

export interface Theme {
  name: string;
  colors: {
    primary: string;
    secondary: string;
    background: string;
    surface: string;
    text: {
      primary: string;
      secondary: string;
      muted: string;
    };
    border: string;
    hover: string;
    accent: string;
    primaryAlpha: string; // 选中状态背景色
  };
  // 窗口外观设置（可选）
  appearance?: {
    window_width: number;      // 窗口宽度 600-1200
    window_height: number;     // 窗口高度 400-800
    transparency: number;      // 透明度 50-100
    border_radius: number;     // 圆角大小 0-20
    blur_strength: number;     // 背景模糊强度 0-20
  };
  // 字体设置（可选）
  font?: {
    font_family: string;
    font_size: number;         // 12-24px
    line_height: number;       // 1.0-2.0
    letter_spacing: number;    // -0.05 - 0.2em
    font_weight: number;       // 300-700
    title_size: number;        // 12-20px
    subtitle_size: number;     // 10-16px
  };
}

export const darkTheme: Theme = {
  name: 'dark',
  colors: {
    primary: '#60a5fa',     // 更亮的蓝色
    secondary: '#a78bfa',   // 更亮的紫色
    background: '#0f172a',  // 深蓝灰
    surface: '#1e293b',     // 较浅的表面
    text: {
      primary: '#f1f5f9',   // 非常亮的白色
      secondary: '#cbd5e1', // 明亮的灰色
      muted: '#94a3b8',     // 中等灰色
    },
    border: '#334155',      // 可见的边框
    hover: '#2d3748',       // 悬停背景
    accent: '#60a5fa',
    primaryAlpha: 'rgba(96, 165, 250, 0.25)', // 选中状态
  },
};

export const lightTheme: Theme = {
  name: 'light',
  colors: {
    primary: '#2563eb',
    secondary: '#7c3aed',
    background: '#ffffff',
    surface: '#f8fafc',     // 更浅的背景
    text: {
      primary: '#0f172a',   // 深色文字
      secondary: '#475569', // 中灰文字
      muted: '#64748b',     // 浅灰文字
    },
    border: '#e2e8f0',
    hover: '#f1f5f9',
    accent: '#3b82f6',
    primaryAlpha: 'rgba(37, 99, 235, 0.15)', // 选中状态
  },
};

export const blueTheme: Theme = {
  name: 'blue',
  colors: {
    primary: '#38bdf8',     // 亮青色
    secondary: '#22d3ee',
    background: '#082f49',  // 深蓝
    surface: '#0c4a6e',     // 中蓝
    text: {
      primary: '#f0f9ff',   // 极亮白
      secondary: '#bae6fd', // 亮青色
      muted: '#7dd3fc',     // 中青色
    },
    border: '#075985',
    hover: '#164e63',
    accent: '#38bdf8',
    primaryAlpha: 'rgba(56, 189, 248, 0.25)', // 选中状态
  },
};

export const purpleTheme: Theme = {
  name: 'purple',
  colors: {
    primary: '#c084fc',     // 亮紫色
    secondary: '#e879f9',
    background: '#1e1b4b',  // 深紫蓝
    surface: '#312e81',     // 中紫
    text: {
      primary: '#faf5ff',   // 极亮白
      secondary: '#e9d5ff', // 亮紫
      muted: '#d8b4fe',     // 中紫
    },
    border: '#4c1d95',
    hover: '#5b21b6',
    accent: '#c084fc',
    primaryAlpha: 'rgba(192, 132, 252, 0.25)', // 选中状态
  },
};

export const greenTheme: Theme = {
  name: 'green',
  colors: {
    primary: '#34d399',     // 亮绿色
    secondary: '#6ee7b7',
    background: '#022c22',  // 深绿
    surface: '#064e3b',     // 中绿
    text: {
      primary: '#ecfdf5',   // 极亮白
      secondary: '#a7f3d0', // 亮绿
      muted: '#6ee7b7',     // 中绿
    },
    border: '#065f46',
    hover: '#047857',
    accent: '#34d399',
    primaryAlpha: 'rgba(52, 211, 153, 0.25)', // 选中状态
  },
};

// Dracula Theme - 流行的吸血鬼配色
export const draculaTheme: Theme = {
  name: 'dracula',
  colors: {
    primary: '#ff79c6',     // 粉色
    secondary: '#bd93f9',   // 紫色
    background: '#282a36',  // 深灰蓝
    surface: '#44475a',     // 中灰
    text: {
      primary: '#f8f8f2',   // 白色
      secondary: '#f8f8f2', // 白色
      muted: '#6272a4',     // 蓝灰
    },
    border: '#6272a4',
    hover: '#44475a',
    accent: '#ff79c6',
    primaryAlpha: 'rgba(255, 121, 198, 0.25)',
  },
};

// Nord Theme - 北欧风格配色
export const nordTheme: Theme = {
  name: 'nord',
  colors: {
    primary: '#88c0d0',     // 青色
    secondary: '#81a1c1',   // 蓝色
    background: '#2e3440',  // 深灰蓝
    surface: '#3b4252',     // 中灰
    text: {
      primary: '#eceff4',   // 雪白
      secondary: '#d8dee9', // 浅灰
      muted: '#8fbcbb',     // 青灰
    },
    border: '#4c566a',
    hover: '#434c5e',
    accent: '#88c0d0',
    primaryAlpha: 'rgba(136, 192, 208, 0.25)',
  },
};

// Solarized Dark - 经典护眼配色
export const solarizedDarkTheme: Theme = {
  name: 'solarized-dark',
  colors: {
    primary: '#268bd2',     // 蓝色
    secondary: '#2aa198',   // 青色
    background: '#002b36',  // 深蓝灰
    surface: '#073642',     // 中蓝灰
    text: {
      primary: '#fdf6e3',   // 米白
      secondary: '#93a1a1', // 灰色
      muted: '#657b83',     // 深灰
    },
    border: '#586e75',
    hover: '#073642',
    accent: '#268bd2',
    primaryAlpha: 'rgba(38, 139, 210, 0.25)',
  },
};

// Monokai Theme - Sublime Text 经典配色
export const monokaiTheme: Theme = {
  name: 'monokai',
  colors: {
    primary: '#66d9ef',     // 青色
    secondary: '#a6e22e',   // 绿色
    background: '#272822',  // 深灰绿
    surface: '#3e3d32',     // 中灰
    text: {
      primary: '#f8f8f2',   // 白色
      secondary: '#f8f8f2', // 白色
      muted: '#75715e',     // 棕灰
    },
    border: '#75715e',
    hover: '#3e3d32',
    accent: '#66d9ef',
    primaryAlpha: 'rgba(102, 217, 239, 0.25)',
  },
};

// One Dark - VS Code 默认暗色主题
export const oneDarkTheme: Theme = {
  name: 'one-dark',
  colors: {
    primary: '#61afef',     // 蓝色
    secondary: '#c678dd',   // 紫色
    background: '#282c34',  // 深灰
    surface: '#21252b',     // 更深灰
    text: {
      primary: '#abb2bf',   // 浅灰
      secondary: '#abb2bf', // 浅灰
      muted: '#5c6370',     // 中灰
    },
    border: '#3e4451',
    hover: '#2c313c',
    accent: '#61afef',
    primaryAlpha: 'rgba(97, 175, 239, 0.25)',
  },
};

// Catppuccin Mocha - 现代柔和配色
export const catppuccinTheme: Theme = {
  name: 'catppuccin',
  colors: {
    primary: '#89b4fa',     // 蓝色
    secondary: '#cba6f7',   // 紫色
    background: '#1e1e2e',  // 深灰
    surface: '#313244',     // 中灰
    text: {
      primary: '#cdd6f4',   // 浅蓝白
      secondary: '#bac2de', // 灰蓝
      muted: '#7f849c',     // 中灰
    },
    border: '#45475a',
    hover: '#313244',
    accent: '#89b4fa',
    primaryAlpha: 'rgba(137, 180, 250, 0.25)',
  },
};

// Tokyo Night - 夜间东京风格
export const tokyoNightTheme: Theme = {
  name: 'tokyo-night',
  colors: {
    primary: '#7aa2f7',     // 蓝色
    secondary: '#bb9af7',   // 紫色
    background: '#1a1b26',  // 深蓝黑
    surface: '#24283b',     // 深蓝灰
    text: {
      primary: '#c0caf5',   // 浅蓝白
      secondary: '#a9b1d6', // 灰蓝
      muted: '#565f89',     // 深蓝灰
    },
    border: '#414868',
    hover: '#292e42',
    accent: '#7aa2f7',
    primaryAlpha: 'rgba(122, 162, 247, 0.25)',
  },
};

export const themes: Record<string, Theme> = {
  dark: darkTheme,
  light: lightTheme,
  blue: blueTheme,
  purple: purpleTheme,
  green: greenTheme,
  dracula: draculaTheme,
  nord: nordTheme,
  'solarized-dark': solarizedDarkTheme,
  monokai: monokaiTheme,
  'one-dark': oneDarkTheme,
  catppuccin: catppuccinTheme,
  'tokyo-night': tokyoNightTheme,
};

export const getTheme = (name: string): Theme => {
  return themes[name] || darkTheme;
};

export const applyTheme = (theme: Theme) => {
  const root = document.documentElement;
  
  // 应用颜色配置
  root.style.setProperty('--color-primary', theme.colors.primary);
  root.style.setProperty('--color-secondary', theme.colors.secondary);
  root.style.setProperty('--color-background', theme.colors.background);
  root.style.setProperty('--color-surface', theme.colors.surface);
  root.style.setProperty('--color-text-primary', theme.colors.text.primary);
  root.style.setProperty('--color-text-secondary', theme.colors.text.secondary);
  root.style.setProperty('--color-text-muted', theme.colors.text.muted);
  root.style.setProperty('--color-border', theme.colors.border);
  root.style.setProperty('--color-hover', theme.colors.hover);
  root.style.setProperty('--color-accent', theme.colors.accent);
  root.style.setProperty('--color-primary-alpha', theme.colors.primaryAlpha);

  // 应用窗口外观配置（如果有）
  if (theme.appearance) {
    root.style.setProperty('--window-width', `${theme.appearance.window_width}px`);
    root.style.setProperty('--window-height', `${theme.appearance.window_height}px`);
    root.style.setProperty('--window-opacity', (theme.appearance.transparency / 100).toString());
    root.style.setProperty('--border-radius', `${theme.appearance.border_radius}px`);
    root.style.setProperty('--blur-strength', `${theme.appearance.blur_strength}px`);
  }

  // 应用字体配置（如果有）
  if (theme.font) {
    root.style.setProperty('--font-family', theme.font.font_family);
    root.style.setProperty('--font-size', `${theme.font.font_size}px`);
    root.style.setProperty('--line-height', theme.font.line_height.toString());
    root.style.setProperty('--letter-spacing', `${theme.font.letter_spacing}em`);
    root.style.setProperty('--font-weight', theme.font.font_weight.toString());
    root.style.setProperty('--title-size', `${theme.font.title_size}px`);
    root.style.setProperty('--subtitle-size', `${theme.font.subtitle_size}px`);
  }
};
