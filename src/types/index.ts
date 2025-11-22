// TypeScript 类型定义

export interface QueryResult {
  id: string;
  title: string;
  subtitle: string;
  icon: WoxImage;
  score: number;
  plugin_id: string;
  context_data: any;
  actions: Action[];
  preview?: Preview;
  refreshable: boolean;
  group?: string;
}

export interface Action {
  id: string;
  name: string;
  icon?: WoxImage;
  is_default: boolean;
  hotkey?: string;
  prevent_hide: boolean;
}

export type WoxImage =
  | { type: 'svg'; data: string }
  | { type: 'file'; data: string }
  | { type: 'url'; data: string }
  | { type: 'base64'; data: string }
  | { type: 'emoji'; data: string }
  | { type: 'system_icon'; data: string };

export type Preview =
  | { type: 'text'; data: string }
  | { type: 'markdown'; data: string }
  | { type: 'image'; data: string }
  | { type: 'html'; data: string }
  | { type: 'file'; data: string };

export interface PluginMetadata {
  id: string;
  name: string;
  author: string;
  version: string;
  description: string;
  icon: WoxImage;
  trigger_keywords: string[];
  commands: Command[];
  settings: SettingDefinition[];
  supported_os: string[];
  plugin_type: PluginType;
}

export interface Command {
  command: string;
  description: string;
}

export interface SettingDefinition {
  type: 'textbox' | 'checkbox' | 'select' | 'head' | 'newline';
  key?: string;
  label?: string;
  value?: any;
  options?: { label: string; value: string }[];
}

export type PluginType = 'Native' | 'Python' | 'NodeJS' | 'Script';

export interface Settings {
  main_hotkey: string;
  theme_id: string;
  language: string;
  show_tray: boolean;
  hide_on_blur: boolean;
  max_results: number;
}

export interface Theme {
  id: string;
  name: string;
  author: string;
  version: string;
  colors: {
    background: string;
    foreground: string;
    primary: string;
    secondary: string;
    accent: string;
    muted: string;
  };
  fonts: {
    query: string;
    result: string;
  };
  border_radius: number;
}

export interface SearchHistoryItem {
  query: string;
  timestamp: string;
  result_count: number;
}
