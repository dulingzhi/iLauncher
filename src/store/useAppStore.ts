import { create } from 'zustand';
import type { QueryResult } from '../types';

interface AppState {
  query: string;
  results: QueryResult[];
  selectedIndex: number;
  loading: boolean;
  visible: boolean;
  
  setQuery: (query: string) => void;
  setResults: (results: QueryResult[]) => void;
  setSelectedIndex: (index: number) => void;
  setLoading: (loading: boolean) => void;
  setVisible: (visible: boolean) => void;
  
  selectNext: () => void;
  selectPrev: () => void;
  reset: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  query: '',
  results: [],
  selectedIndex: 0,
  loading: false,
  visible: false,
  
  setQuery: (query) => set({ query }),
  setResults: (results) => set({ results, selectedIndex: 0 }),
  setSelectedIndex: (index) => set({ selectedIndex: index }),
  setLoading: (loading) => set({ loading }),
  setVisible: (visible) => set({ visible }),
  
  selectNext: () => {
    const { selectedIndex, results } = get();
    if (selectedIndex < results.length - 1) {
      set({ selectedIndex: selectedIndex + 1 });
    }
  },
  
  selectPrev: () => {
    const { selectedIndex } = get();
    if (selectedIndex > 0) {
      set({ selectedIndex: selectedIndex - 1 });
    }
  },
  
  reset: () => set({
    query: '',
    results: [],
    selectedIndex: 0,
    loading: false,
  }),
}));
