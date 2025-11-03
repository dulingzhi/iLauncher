import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { getTheme, applyTheme, Theme } from '../theme';

interface ThemeStore {
  currentTheme: string;
  theme: Theme;
  setTheme: (themeName: string) => void;
}

export const useThemeStore = create<ThemeStore>()(
  persist(
    (set) => ({
      currentTheme: 'dark',
      theme: getTheme('dark'),
      setTheme: (themeName: string) => {
        const newTheme = getTheme(themeName);
        applyTheme(newTheme);
        set({ currentTheme: themeName, theme: newTheme });
      },
    }),
    {
      name: 'theme-storage',
    }
  )
);
