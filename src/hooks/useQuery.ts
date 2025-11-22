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
    const queryStartTime = performance.now();
    
    console.log(`[Query] Starting query #${currentQueryId}: "${input}"`);
    
    setLoading(true);
    try {
      const data = await invoke<QueryResult[]>('query', { input });
      const queryElapsed = performance.now() - queryStartTime;
      
      // 只有当这是最新的查询时才更新结果
      if (currentQueryId === queryIdRef.current) {
        setResults(data);
        console.log(`[Query] ✅ Completed #${currentQueryId}: ${data.length} results in ${queryElapsed.toFixed(2)}ms`);
      } else {
        console.log('[useQuery] Discarding stale query result:', { 
          currentQueryId, 
          latestQueryId: queryIdRef.current,
          elapsed: `${queryElapsed.toFixed(2)}ms`
        });
      }
    } catch (error) {
      const queryElapsed = performance.now() - queryStartTime;
      console.error(`[Query] ❌ Failed #${currentQueryId} after ${queryElapsed.toFixed(2)}ms:`, error);
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
    // 🔥 优化：增加 debounce 延迟到 100ms，减少连续输入时的查询次数
    // 虽然 MFT 查询很快，但频繁查询仍会造成卡顿（评分、渲染等）
    debounceTimer = setTimeout(() => {
      performQuery(input);
    }, 100);
  }, [performQuery]);
  
  return { results, loading, debouncedQuery };
}

export function useExecuteAction() {
  return useCallback(async (
    resultId: string, 
    actionId: string, 
    pluginId: string, 
    title: string,
    subtitle: string,
    icon: any // WoxImage type
  ) => {
    console.log('[useExecuteAction] Called with:', { resultId, actionId, pluginId, title });
    try {
      await invoke('execute_action', { resultId, actionId, pluginId, title, subtitle, icon });
    } catch (error) {
      console.error('Execute action failed:', error);
    }
  }, []);
}
