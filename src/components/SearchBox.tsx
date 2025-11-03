import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Search } from 'lucide-react';
import { useAppStore } from '../store/useAppStore';
import { useQuery, useExecuteAction } from '../hooks/useQuery';
import { cn } from '../utils/cn';

export function SearchBox() {
  const {
    query,
    selectedIndex,
    setQuery,
    setResults,
    selectNext,
    selectPrev,
    reset,
  } = useAppStore();
  
  const { results, loading, debouncedQuery } = useQuery();
  const executeAction = useExecuteAction();
  
  useEffect(() => {
    setResults(results);
  }, [results, setResults]);
  
  useEffect(() => {
    debouncedQuery(query);
  }, [query, debouncedQuery]);
  
  const handleKeyDown = async (e: React.KeyboardEvent) => {
    switch (e.key) {
      case 'Enter':
        e.preventDefault();
        await handleExecute();
        break;
        
      case 'ArrowDown':
        e.preventDefault();
        selectNext();
        break;
        
      case 'ArrowUp':
        e.preventDefault();
        selectPrev();
        break;
        
      case 'Escape':
        e.preventDefault();
        await handleHide();
        break;
    }
  };
  
  const handleExecute = async () => {
    if (results.length === 0) return;
    
    const result = results[selectedIndex];
    const defaultAction = result.actions.find(a => a.is_default) || result.actions[0];
    
    if (defaultAction) {
      await executeAction(result.id, defaultAction.id);
      
      if (!defaultAction.prevent_hide) {
        await handleHide();
      }
    }
  };
  
  const handleHide = async () => {
    try {
      await invoke('hide_app');
      reset();
    } catch (error) {
      console.error('Failed to hide app:', error);
    }
  };
  
  return (
    <div className="w-full">
      {/* 搜索输入框 */}
      <div className="flex items-center gap-3 px-4 py-3 bg-white/95 backdrop-blur-sm border-b border-gray-200">
        <Search className="w-5 h-5 text-gray-400" />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type to search..."
          className="flex-1 text-lg bg-transparent outline-none"
          autoFocus
        />
        {loading && (
          <div className="w-4 h-4 border-2 border-gray-300 border-t-blue-500 rounded-full animate-spin" />
        )}
      </div>
      
      {/* 结果列表 */}
      {results.length > 0 && (
        <div className="bg-white/95 backdrop-blur-sm max-h-[500px] overflow-y-auto">
          {results.map((result, index) => (
            <ResultItem
              key={result.id}
              result={result}
              isSelected={index === selectedIndex}
              onClick={() => useAppStore.setState({ selectedIndex: index })}
            />
          ))}
        </div>
      )}
    </div>
  );
}

interface ResultItemProps {
  result: any;
  isSelected: boolean;
  onClick: () => void;
}

function ResultItem({ result, isSelected, onClick }: ResultItemProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-3 px-4 py-3 cursor-pointer transition-colors",
        isSelected ? "bg-blue-50" : "hover:bg-gray-50"
      )}
      onClick={onClick}
    >
      {/* 图标 */}
      <div className="flex-shrink-0 w-10 h-10 flex items-center justify-center text-2xl">
        {result.icon.type === 'emoji' ? result.icon.data : '📄'}
      </div>
      
      {/* 文本 */}
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-gray-900 truncate">
          {result.title}
        </div>
        {result.subtitle && (
          <div className="text-xs text-gray-500 truncate">
            {result.subtitle}
          </div>
        )}
      </div>
      
      {/* 分数（调试用） */}
      {/* <div className="text-xs text-gray-400">{result.score}</div> */}
    </div>
  );
}
