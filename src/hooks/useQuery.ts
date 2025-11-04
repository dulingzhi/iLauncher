import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { QueryResult } from '../types';

let debounceTimer: ReturnType<typeof setTimeout>;

export function useQuery() {
  const [results, setResults] = useState<QueryResult[]>([]);
  const [loading, setLoading] = useState(false);
  
  const performQuery = useCallback(async (input: string) => {
    if (!input.trim()) {
      setResults([]);
      return;
    }
    
    setLoading(true);
    try {
      const data = await invoke<QueryResult[]>('query', { input });
      setResults(data);
    } catch (error) {
      console.error('Query failed:', error);
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);
  
  const debouncedQuery = useCallback((input: string) => {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      performQuery(input);
    }, 100);
  }, [performQuery]);
  
  return { results, loading, debouncedQuery };
}

export function useExecuteAction() {
  return useCallback(async (resultId: string, actionId: string, pluginId: string, title: string) => {
    console.log('[useExecuteAction] Called with:', { resultId, actionId, pluginId, title });
    try {
      await invoke('execute_action', { resultId, actionId, pluginId, title });
    } catch (error) {
      console.error('Execute action failed:', error);
    }
  }, []);
}
