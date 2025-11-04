import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Search } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store/useAppStore';
import { useQuery, useExecuteAction } from '../hooks/useQuery';
import { ContextMenu } from './ContextMenu';
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
  const [clearOnHide, setClearOnHide] = useState(true);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    actions: any[];
    resultId: string;
    resultTitle: string;
    pluginId: string;
  } | null>(null);
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
  
  // 加载配置
  useEffect(() => {
    const loadConfig = async () => {
      try {
        const config = await invoke<any>('load_config');
        setClearOnHide(config.general.clear_on_hide);
      } catch (error) {
        console.error('Failed to load config:', error);
      }
    };
    loadConfig();
  }, []);
  
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
  
  // 当选中的结果改变时，关闭右键菜单
  useEffect(() => {
    setContextMenu(null);
    setSelectedActionIndex(0);
  }, [selectedIndex]);
  
  // 点击其他地方关闭右键菜单
  useEffect(() => {
    const handleClickOutside = () => setContextMenu(null);
    document.addEventListener('click', handleClickOutside);
    return () => document.removeEventListener('click', handleClickOutside);
  }, []);
  
  const handleKeyDown = async (e: React.KeyboardEvent) => {
    // 如果右键菜单显示中，处理上下键选择操作
    if (contextMenu && contextMenu.actions.length > 0) {
      switch (e.key) {
        case 'ArrowUp':
          e.preventDefault();
          setSelectedActionIndex(prev => 
            prev > 0 ? prev - 1 : contextMenu.actions.length - 1
          );
          return;
          
        case 'ArrowDown':
          e.preventDefault();
          setSelectedActionIndex(prev => 
            prev < contextMenu.actions.length - 1 ? prev + 1 : 0
          );
          return;
          
        case 'Enter':
          e.preventDefault();
          await handleExecuteContextAction(contextMenu.actions[selectedActionIndex].id);
          return;
          
        case 'Escape':
          e.preventDefault();
          setContextMenu(null);
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
    
    // 关闭右键菜单
    setContextMenu(null);
  };
  
  const handleExecuteContextAction = async (actionId: string) => {
    if (!contextMenu) return;
    
    const action = contextMenu.actions.find(a => a.id === actionId);
    if (!action) return;
    
    console.log('[handleExecuteContextAction] Executing:', {
      actionId,
      resultId: contextMenu.resultId,
      pluginId: contextMenu.pluginId,
      title: contextMenu.resultTitle
    });
    
    await executeAction(contextMenu.resultId, actionId, contextMenu.pluginId, contextMenu.resultTitle);
    
    if (!action.prevent_hide) {
      await handleHide();
    }
    
    // 关闭右键菜单
    setContextMenu(null);
  };
  
  const handleContextMenu = (e: React.MouseEvent, result: any) => {
    e.preventDefault();
    e.stopPropagation();
    
    if (result.actions.length === 0) return;
    
    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      actions: result.actions,
      resultId: result.id,
      resultTitle: result.title,
      pluginId: result.plugin_id,
    });
    setSelectedActionIndex(0);
  };
  
  const handleHide = async () => {
    try {
      await invoke('hide_app');
      if (clearOnHide) {
        reset();
      }
    } catch (error) {
      console.error('Failed to hide app:', error);
    }
  };
  
  return (
    <div className="w-full">
      {/* 搜索输入框 */}
      <div className="flex items-center gap-3 px-4 py-3 border-b border-border" style={{ backgroundColor: 'var(--color-surface)' }}>
        <Search className="w-5 h-5 text-text-muted" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('search.placeholder')}
          className="flex-1 text-lg bg-transparent outline-none text-text-primary placeholder:text-text-muted"
          autoFocus
        />
        {loading && (
          <div className="w-4 h-4 border-2 border-text-muted border-t-primary rounded-full animate-spin" />
        )}
      </div>
      
      {/* 结果列表 */}
      {results.length > 0 && (
        <div 
          ref={resultsContainerRef}
          className="max-h-[450px] overflow-y-auto"
          style={{ backgroundColor: 'var(--color-surface)' }}
        >
          {results.map((result, index) => (
            <ResultItem
              key={result.id}
              ref={index === selectedIndex ? selectedItemRef : null}
              result={result}
              isSelected={index === selectedIndex}
              onClick={() => {
                useAppStore.setState({ selectedIndex: index });
                // 延迟执行,确保选中状态已更新
                setTimeout(async () => {
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
                    await executeAction(result.id, defaultAction.id, result.plugin_id, result.title);
                    if (!defaultAction.prevent_hide) {
                      await handleHide();
                    }
                  }
                }, 0);
              }}
              onContextMenu={(e) => handleContextMenu(e, result)}
            />
          ))}
        </div>
      )}
      
      {/* 右键上下文菜单 */}
      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          actions={contextMenu.actions}
          selectedIndex={selectedActionIndex}
          onSelect={setSelectedActionIndex}
          onExecute={handleExecuteContextAction}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}

interface ResultItemProps {
  result: any;
  isSelected: boolean;
  onClick: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
}

const ResultItem = React.forwardRef<HTMLDivElement, ResultItemProps>(
  ({ result, isSelected, onClick, onContextMenu }, ref) => {
    return (
      <div
        ref={ref}
        className={cn(
          "flex items-center gap-3 px-4 py-3 cursor-pointer transition-colors",
          isSelected ? "bg-primary/10" : "hover:bg-hover"
        )}
        onClick={onClick}
        onContextMenu={onContextMenu}
      >
        {/* 图标 */}
        <div className="flex-shrink-0 w-10 h-10 flex items-center justify-center text-2xl">
          {result.icon.type === 'emoji' ? result.icon.data : '📄'}
        </div>
        
        {/* 文本 */}
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-text-primary truncate">
            {result.title}
          </div>
          {result.subtitle && (
            <div className="text-xs text-text-secondary truncate">
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
