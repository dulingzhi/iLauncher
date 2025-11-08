import { useState, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { QueryResult } from '../types';

let debounceTimer: ReturnType<typeof setTimeout>;

export function useQuery() {
  const [results, setResults] = useState<QueryResult[]>([]);
  const [loading, setLoading] = useState(false);
  
  // 使用 ref 跟踪最新的查询序列号
  const queryIdRef = useRef(0);
  
  const performQuery = useCallback(async (input: string) => {
    if (!input.trim()) {
      setResults([]);
      return;
    }
    
    // 生成新的查询ID
    const currentQueryId = ++queryIdRef.current;
    
    setLoading(true);
    try {
      const data = await invoke<QueryResult[]>('query', { input });
      
      // 只有当这是最新的查询时才更新结果
      if (currentQueryId === queryIdRef.current) {
        setResults(data);
      } else {
        console.log('[useQuery] Discarding stale query result:', { 
          currentQueryId, 
          latestQueryId: queryIdRef.current 
        });
      }
    } catch (error) {
      console.error('Query failed:', error);
      // 只有当这是最新的查询时才清空结果
      if (currentQueryId === queryIdRef.current) {
        setResults([]);
      }
    } finally {
      // 只有当这是最新的查询时才关闭 loading
      if (currentQueryId === queryIdRef.current) {
        setLoading(false);
      }
    }
  }, []);
  
  const debouncedQuery = useCallback((input: string) => {
    clearTimeout(debounceTimer);
    // 🔥 优化：减少 debounce 延迟到 50ms（MFT 查询很快）
    debounceTimer = setTimeout(() => {
      performQuery(input);
    }, 50);
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
