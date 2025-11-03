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
    primary: '#3b82f6',
    secondary: '#8b5cf6',
    background: '#111827',
    surface: '#1f2937',
    text: {
      primary: '#f9fafb',
      secondary: '#d1d5db',
      muted: '#9ca3af',
    },
    border: '#374151',
    hover: '#374151',
    accent: '#60a5fa',
  },
};

export const lightTheme: Theme = {
  name: 'light',
  colors: {
    primary: '#2563eb',
    secondary: '#7c3aed',
    background: '#ffffff',
    surface: '#f3f4f6',
    text: {
      primary: '#111827',
      secondary: '#4b5563',
      muted: '#6b7280',
    },
    border: '#e5e7eb',
    hover: '#e5e7eb',
    accent: '#3b82f6',
  },
};

export const blueTheme: Theme = {
  name: 'blue',
  colors: {
    primary: '#0ea5e9',
    secondary: '#06b6d4',
    background: '#0c4a6e',
    surface: '#075985',
    text: {
      primary: '#f0f9ff',
      secondary: '#bae6fd',
      muted: '#7dd3fc',
    },
    border: '#0369a1',
    hover: '#0369a1',
    accent: '#38bdf8',
  },
};

export const purpleTheme: Theme = {
  name: 'purple',
  colors: {
    primary: '#a855f7',
    secondary: '#c084fc',
    background: '#3b0764',
    surface: '#581c87',
    text: {
      primary: '#faf5ff',
      secondary: '#e9d5ff',
      muted: '#d8b4fe',
    },
    border: '#6b21a8',
    hover: '#6b21a8',
    accent: '#c084fc',
  },
};

export const greenTheme: Theme = {
  name: 'green',
  colors: {
    primary: '#10b981',
    secondary: '#34d399',
    background: '#064e3b',
    surface: '#065f46',
    text: {
      primary: '#ecfdf5',
      secondary: '#a7f3d0',
      muted: '#6ee7b7',
    },
    border: '#047857',
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
