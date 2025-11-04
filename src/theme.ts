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
  },
};

export const themes: Record<string, Theme> = {
  dark: darkTheme,
  light: lightTheme,
  blue: blueTheme,
  purple: purpleTheme,
  green: greenTheme,
};

export const getTheme = (name: string): Theme => {
  return themes[name] || darkTheme;
};

export const applyTheme = (theme: Theme) => {
  const root = document.documentElement;
  
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
};
