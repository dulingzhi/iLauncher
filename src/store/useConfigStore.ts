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
    // Theme settings
    theme: string;
    custom_theme?: {
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
        primaryAlpha: string;
      };
    };
    // Window settings
    show_preview: boolean;
    opacity: number;
    window_width: number;
    window_height: number;
    transparency: number;
    language: string;
    // Font settings (deprecated, moved to font)
    font_size: number;
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
  _loadAttempted: boolean; // 跟踪是否已尝试加载
  loadConfig: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  updateConfig: (updates: Partial<AppConfig>) => void;
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: null,
  loading: false,
  error: null,
  _loadAttempted: false,

  loadConfig: async () => {
    // 如果已经在加载、已加载或已尝试加载，则跳过
    const state = get();
    if (state.loading || state.config !== null || state._loadAttempted) {
      console.log('Config load skipped:', { 
        loading: state.loading, 
        hasConfig: state.config !== null,
        attempted: state._loadAttempted 
      });
      return;
    }

    set({ loading: true, error: null, _loadAttempted: true });
    try {
      const config = await invoke<AppConfig>('load_config');
      console.log('✓ Config loaded from backend');
      set({ config, loading: false });
    } catch (error) {
      console.error('Failed to load config:', error);
      set({ error: String(error), loading: false });
    }
  },

  saveConfig: async (config: AppConfig) => {
    try {
      await invoke('save_config', { config });
      console.log('✓ Config saved to backend');
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
