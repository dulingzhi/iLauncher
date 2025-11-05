import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface AppConfig {
  general: {
    language: string;
    clear_on_hide: boolean;
    max_results: number;
    startup_on_boot: boolean;
    hotkey: string;
    search_delay: number;
  };
  appearance: {
    theme: string;
    show_preview: boolean;
    opacity: number;
    font_size: number;
    language: string;
    window_width: number;
    window_height: number;
    transparency: number;
  };
  hotkey: {
    toggle_window: string;
  };
  plugins: {
    enabled_plugins: string[];
    disabled_plugins: string[];
  };
  advanced: {
    start_on_boot: boolean;
    show_tray_icon: boolean;
    enable_analytics: boolean;
    cache_enabled: boolean;
  };
}

interface ConfigState {
  config: AppConfig | null;
  loading: boolean;
  error: string | null;
  loadConfig: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  updateConfig: (updates: Partial<AppConfig>) => void;
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: null,
  loading: false,
  error: null,

  loadConfig: async () => {
    // 如果已经在加载或已加载，则跳过
    const state = get();
    if (state.loading || state.config !== null) {
      return;
    }

    set({ loading: true, error: null });
    try {
      const config = await invoke<AppConfig>('load_config');
      set({ config, loading: false });
    } catch (error) {
      console.error('Failed to load config:', error);
      set({ error: String(error), loading: false });
    }
  },

  saveConfig: async (config: AppConfig) => {
    try {
      await invoke('save_config', { config });
      set({ config });
    } catch (error) {
      console.error('Failed to save config:', error);
      set({ error: String(error) });
      throw error;
    }
  },

  updateConfig: (updates: Partial<AppConfig>) => {
    const state = get();
    if (state.config) {
      const newConfig = { ...state.config, ...updates };
      set({ config: newConfig });
    }
  },
}));
