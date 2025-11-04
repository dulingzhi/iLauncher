import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Search } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store/useAppStore';
import { useQuery, useExecuteAction } from '../hooks/useQuery';
import { ActionPanel } from './ActionPanel';
import { cn } from '../utils/cn';

interface SearchBoxProps {
  onOpenSettings: () => void;
  onOpenPlugins: () => void;
}

export function SearchBox({ onOpenSettings, onOpenPlugins }: SearchBoxProps) {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
  const resultsContainerRef = useRef<HTMLDivElement>(null);
  const selectedItemRef = useRef<HTMLDivElement>(null);
  const [showActionPanel, setShowActionPanel] = useState(false);
  const [selectedActionIndex, setSelectedActionIndex] = useState(0);
  
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
  
  // 获取当前选中结果的操作列表
  const currentActions = results.length > 0 ? results[selectedIndex]?.actions || [] : [];
  
  useEffect(() => {
    setResults(results);
  }, [results, setResults]);
  
  useEffect(() => {
    debouncedQuery(query);
  }, [query, debouncedQuery]);
  
  // 自动滚动到选中项
  useEffect(() => {
    if (selectedItemRef.current && resultsContainerRef.current) {
      selectedItemRef.current.scrollIntoView({
        block: 'nearest',
        behavior: 'smooth',
      });
    }
  }, [selectedIndex]);
  
  // 监听窗口显示事件，自动聚焦输入框
  useEffect(() => {
    const appWindow = getCurrentWindow();
    
    const setupListener = async () => {
      const unlisten = await appWindow.listen('focus-input', () => {
        reset();
        setTimeout(() => {
          if (inputRef.current) {
            inputRef.current.focus();
          }
        }, 10);
      });
      return unlisten;
    };
    
    const unlistenPromise = setupListener();
    
    return () => {
      unlistenPromise.then(fn => fn());
    };
  }, [reset]);
  
  // 当选中的结果改变时，重置 Action Panel 状态
  useEffect(() => {
    setShowActionPanel(false);
    setSelectedActionIndex(0);
  }, [selectedIndex]);
  
  const handleKeyDown = async (e: React.KeyboardEvent) => {
    // 如果 Action Panel 显示中，处理左右键选择操作
    if (showActionPanel && currentActions.length > 0) {
      switch (e.key) {
        case 'ArrowLeft':
          e.preventDefault();
          setSelectedActionIndex(prev => 
            prev > 0 ? prev - 1 : currentActions.length - 1
          );
          return;
          
        case 'ArrowRight':
          e.preventDefault();
          setSelectedActionIndex(prev => 
            prev < currentActions.length - 1 ? prev + 1 : 0
          );
          return;
          
        case 'Enter':
          e.preventDefault();
          await handleExecuteAction(currentActions[selectedActionIndex].id);
          return;
          
        case 'Escape':
          e.preventDefault();
          setShowActionPanel(false);
          return;
      }
    }
    
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
        
      case 'Tab':
        e.preventDefault();
        // Tab 键切换显示 Action Panel
        if (results.length > 0 && currentActions.length > 0) {
          setShowActionPanel(!showActionPanel);
          setSelectedActionIndex(0);
        }
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
    
    // 检查是否是 Settings 或 Plugin Manager
    if (result.id === 'settings') {
      onOpenSettings();
      return;
    }
    
    if (result.id === 'plugin_manager') {
      onOpenPlugins();
      return;
    }
    
    const defaultAction = result.actions.find(a => a.is_default) || result.actions[0];
    
    if (defaultAction) {
      await handleExecuteAction(defaultAction.id);
    }
  };
  
  const handleExecuteAction = async (actionId: string) => {
    if (results.length === 0) return;
    
    const result = results[selectedIndex];
    const action = result.actions.find(a => a.id === actionId);
    
    if (!action) return;
    
    await executeAction(result.id, actionId, result.plugin_id, result.title);
    
    if (!action.prevent_hide) {
      await handleHide();
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
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('search.placeholder')}
          className="flex-1 text-lg bg-transparent outline-none"
          autoFocus
        />
        {loading && (
          <div className="w-4 h-4 border-2 border-gray-300 border-t-blue-500 rounded-full animate-spin" />
        )}
      </div>
      
      {/* 结果列表 */}
      {results.length > 0 && (
        <>
          <div 
            ref={resultsContainerRef}
            className="bg-white/95 backdrop-blur-sm max-h-[400px] overflow-y-auto"
          >
            {results.map((result, index) => (
              <ResultItem
                key={result.id}
                ref={index === selectedIndex ? selectedItemRef : null}
                result={result}
                isSelected={index === selectedIndex}
                onClick={() => useAppStore.setState({ selectedIndex: index })}
              />
            ))}
          </div>
          
          {/* Action Panel */}
          {showActionPanel && currentActions.length > 0 && (
            <ActionPanel
              actions={currentActions}
              selectedActionIndex={selectedActionIndex}
              onActionSelect={setSelectedActionIndex}
              onExecuteAction={handleExecuteAction}
            />
          )}
          
          {/* 提示信息 */}
          {!showActionPanel && currentActions.length > 0 && (
            <div className="px-4 py-2 bg-gray-50/95 backdrop-blur-sm border-t border-gray-200">
              <div className="text-xs text-gray-500 text-center">
                {t('search.showMoreActions')}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

interface ResultItemProps {
  result: any;
  isSelected: boolean;
  onClick: () => void;
}

const ResultItem = React.forwardRef<HTMLDivElement, ResultItemProps>(
  ({ result, isSelected, onClick }, ref) => {
    return (
      <div
        ref={ref}
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
);

ResultItem.displayName = 'ResultItem';
