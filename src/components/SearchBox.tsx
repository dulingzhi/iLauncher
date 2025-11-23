import React, { useEffect, useRef, useState, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Search } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useAppStore } from '../store/useAppStore';
import { useConfigStore } from '../store/useConfigStore';
import { useQuery, useExecuteAction } from '../hooks/useQuery';
import { ContextMenu } from './ContextMenu';
import { highlightMatch } from '../utils/pinyinSearch';
import '../animations.css';

// 格式化时间戳为相对时间
function formatTimestamp(timestamp: string): string {
  const date = new Date(timestamp);
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  
  if (days > 0) {
    return `${days} 天前`;
  } else if (hours > 0) {
    return `${hours} 小时前`;
  } else if (minutes > 0) {
    return `${minutes} 分钟前`;
  } else {
    return '刚刚';
  }
}

interface SearchBoxProps {
  onOpenSettings: () => void;
  onOpenPlugins: () => void;
  onOpenClipboard: () => void;
  onOpenAIChat: () => void;
  onShowHotkeyGuide: () => void;
}

export function SearchBox({ onOpenSettings, onOpenPlugins, onOpenClipboard, onOpenAIChat, onShowHotkeyGuide }: SearchBoxProps) {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
  const resultsContainerRef = useRef<HTMLDivElement>(null);
  const selectedItemRef = useRef<HTMLDivElement>(null);
  const lastNavigationTime = useRef<number>(0);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    actions: any[];
    resultId: string;
    resultTitle: string;
    pluginId: string;
  } | null>(null);
  const [selectedActionIndex, setSelectedActionIndex] = useState(0);
  const [suggestions, setSuggestions] = useState<any[]>([]);
  
  const {
    query,
    setQuery,
    setResults,
    reset,
  } = useAppStore();
  
  const { config } = useConfigStore();
  const clearOnHide = config?.general.clear_on_hide ?? true;
  
  const { results, loading, debouncedQuery } = useQuery();
  const executeAction = useExecuteAction();
  
  // 获取搜索建议
  useEffect(() => {
    const fetchSuggestions = async () => {
      if (!query.trim() || loading) {
        setSuggestions([]);
        return;
      }
      
      try {
        const items = await invoke<any[]>('get_search_suggestions', {
          prefix: query,
          limit: 5,
        });
        setSuggestions(items);
      } catch (error) {
        console.error('Failed to fetch suggestions:', error);
        setSuggestions([]);
      }
    };
    
    const timer = setTimeout(fetchSuggestions, 150);
    return () => clearTimeout(timer);
  }, [query, loading]);
  
  // 判断是否有有效的搜索结果
  const hasValidResults = useMemo(() => {
    return results.length > 0 && !results.every(r => 
      r.title.toLowerCase().includes('no ') || 
      r.title.toLowerCase().includes('无') ||
      r.title === 'No files found' ||
      r.title === 'No clipboard history'
    );
  }, [results]);
  
  // 合并搜索结果和建议
  const displayResults = useMemo(() => {
    if (hasValidResults) {
      return results;
    } else if (suggestions.length > 0) {
      const suggestionResults = suggestions.map((item) => ({
        id: `suggestion_${item.query}`,
        title: item.query,
        subtitle: `搜索历史 · 使用 ${item.frequency} 次 · ${formatTimestamp(item.timestamp)}`,
        icon: { type: 'emoji', data: '🕒' } as any,
        score: 1000,
        plugin_id: 'search_history',
        context_data: null,
        actions: [{
          id: 'search',
          name: '搜索',
          is_default: true,
          prevent_hide: true,
          hotkey: undefined,
          icon: undefined,
        }],
        preview: undefined,
        refreshable: false,
        group: '搜索历史建议',
      }));
      
      if (results.length > 0) {
        return [...suggestionResults, ...results];
      }
      return suggestionResults;
    }
    return results;
  }, [results, suggestions, hasValidResults]);
  
  // 本地导航函数（定义在displayResults之后）
  const selectNext = () => {
    setSelectedIndex(prev => Math.min(prev + 1, displayResults.length - 1));
  };
  
  const selectPrev = () => {
    setSelectedIndex(prev => Math.max(prev - 1, 0));
  };
  
  // 当显示结果改变时重置选中索引
  useEffect(() => {
    setSelectedIndex(0);
  }, [displayResults.length]);
  
  useEffect(() => {
    debouncedQuery(query);
  }, [query, debouncedQuery]);
  
  // 自动滚动到选中项
  useEffect(() => {
    if (selectedItemRef.current && resultsContainerRef.current) {
      selectedItemRef.current.scrollIntoView({
        block: 'nearest',  // 🔥 使用 nearest，避免不必要的滚动
        behavior: 'smooth',
      });
    }
  }, [selectedIndex]);
  
  // 监听窗口显示事件，自动聚焦输入框
  useEffect(() => {
    const appWindow = getCurrentWindow();
    
    const setupListeners = async () => {
      // 监听 focus-input 事件
      const unlistenFocusInput = await appWindow.listen('focus-input', () => {
        reset();
        if (inputRef.current) {
          inputRef.current.focus();
          inputRef.current.select();
        }
      });
      
      // 监听 app-hiding 事件，根据配置清空搜索结果
      const unlistenAppHiding = await appWindow.listen('app-hiding', () => {
        if (clearOnHide) {
          console.log('Clearing search results on hide (clear_on_hide enabled)');
          reset();
        } else {
          console.log('Keeping search results on hide (clear_on_hide disabled)');
        }
      });
      
      return () => {
        unlistenFocusInput();
        unlistenAppHiding();
      };
    };
    
    const cleanup = setupListeners();
    
    return () => {
      cleanup.then(fn => fn());
    };
  }, [reset, clearOnHide]);
  
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
    // 快捷键指南
    if ((e.key === '?' && !e.shiftKey) || e.key === 'F1') {
      e.preventDefault();
      onShowHotkeyGuide();
      return;
    }
    
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
        // 节流处理：限制导航频率为每 50ms 一次
        const now = Date.now();
        if (now - lastNavigationTime.current >= 50) {
          selectNext();
          lastNavigationTime.current = now;
        }
        break;
        
      case 'ArrowUp':
        e.preventDefault();
        // 节流处理：限制导航频率为每 50ms 一次
        const upNow = Date.now();
        if (upNow - lastNavigationTime.current >= 50) {
          selectPrev();
          lastNavigationTime.current = upNow;
        }
        break;
        
      case 'Escape':
        e.preventDefault();
        await handleHide();
        break;
    }
  };
  
  const handleExecute = async () => {
    if (displayResults.length === 0) return;
    
    const result = displayResults[selectedIndex];
    
    // 处理搜索建议
    if (result.plugin_id === 'search_history') {
      setQuery(result.title);
      if (inputRef.current) {
        inputRef.current.focus();
      }
      return;
    }
    
    // 记录搜索执行
    if (query.trim()) {
      try {
        await invoke('record_search_execution', { query: query.trim() });
      } catch (error) {
        console.error('Failed to record search execution:', error);
      }
    }
    
    // 检查是否是 Settings 或 Plugin Manager 或 Clipboard 或 AI Chat
    if (result.id === 'settings') {
      onOpenSettings();
      return;
    }
    
    if (result.id === 'plugin_manager') {
      onOpenPlugins();
      return;
    }
    
    if (result.id === 'clipboard_history') {
      onOpenClipboard();
      return;
    }
    
    // AI助手的配置
    if (result.id === 'config' && result.plugin_id === 'ai_assistant') {
      onOpenAIChat();
      return;
    }
    
    const defaultAction = result.actions.find(a => a.is_default) || result.actions[0];
    
    if (defaultAction) {
      await handleExecuteAction(defaultAction.id);
    }
  };
  
  const handleExecuteAction = async (actionId: string) => {
    if (displayResults.length === 0) return;
    
    const result = displayResults[selectedIndex];
    const action = result.actions.find(a => a.id === actionId);
    
    if (!action) return;
    
    // 处理AI助手的open_settings动作
    if (actionId === 'open_settings' && result.plugin_id === 'ai_assistant') {
      onOpenAIChat();
      return;
    }
    
    // 处理执行历史的 execute 动作 - 使用原始的 plugin_id 和 result_id
    if (actionId === 'execute' && result.plugin_id === 'execution-history') {
      const contextData = result.context_data as any;
      if (contextData && contextData.original_id && contextData.plugin_id && contextData.action_id) {
        await executeAction(
          contextData.original_id,
          contextData.action_id,
          contextData.plugin_id,
          result.title,
          result.subtitle,
          result.icon
        );
        
        if (!action.prevent_hide) {
          await handleHide();
        }
        
        setContextMenu(null);
        return;
      }
    }
    
    await executeAction(result.id, actionId, result.plugin_id, result.title, result.subtitle, result.icon);
    
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
    
    // 从results中找到对应的结果获取subtitle和icon
    const result = displayResults.find(r => r.id === contextMenu.resultId);
    
    // 处理执行历史的 execute 动作 - 使用原始的 plugin_id 和 result_id
    if (actionId === 'execute' && contextMenu.pluginId === 'execution-history' && result) {
      const contextData = result.context_data as any;
      if (contextData && contextData.original_id && contextData.plugin_id && contextData.action_id) {
        await executeAction(
          contextData.original_id,
          contextData.action_id,
          contextData.plugin_id,
          contextMenu.resultTitle,
          result?.subtitle || '',
          result?.icon || { type: 'emoji', data: '📋' }
        );
        
        if (!action.prevent_hide) {
          await handleHide();
        }
        
        setContextMenu(null);
        return;
      }
    }
    
    await executeAction(
      contextMenu.resultId, 
      actionId, 
      contextMenu.pluginId, 
      contextMenu.resultTitle,
      result?.subtitle || '',
      result?.icon || { type: 'emoji', data: '📋' }
    );
    
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
      {/* 搜索输入框 - 添加动画效果 */}
      <div className="flex items-center gap-3 px-6 py-4 border-b" style={{ 
        backgroundColor: 'var(--color-surface)',
        borderBottomColor: 'rgba(255, 255, 255, 0.08)'
      }}>
        <Search className="w-5 h-5" style={{ color: 'var(--color-text-muted)' }} />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('search.placeholder') || 'Type to search...'}
          autoFocus
          className="search-input flex-1 text-base bg-transparent border-0 focus:outline-none focus:ring-0 focus:border-0 placeholder:text-gray-500"
          style={{ 
            color: 'var(--color-text-primary)',
            outline: 'none',
            boxShadow: 'none',
          }}
          tabIndex={0}
        />
        {loading && (
          <div 
            className="loading-spinner w-4 h-4 border-2 rounded-full" 
            style={{ 
              borderColor: 'var(--color-text-muted)',
              borderTopColor: 'var(--color-primary)'
            }}
          />
        )}
      </div>
      
      {/* 结果列表标题 - Windows 11 风格 */}
      {displayResults.length > 0 && (
        <>
          <div className="px-6 py-2 text-xs font-medium" style={{ 
            color: 'var(--color-text-muted)',
            backgroundColor: 'var(--color-surface)'
          }}>
            {t('search.searchResults')}
          </div>
          
          {/* 结果列表 */}
          <div 
            ref={resultsContainerRef}
            className="max-h-[450px] overflow-y-auto pb-2 scrollbar-thin scrollbar-thumb-gray-600 scrollbar-track-transparent"
            style={{ backgroundColor: 'var(--color-surface)' }}
          >
            {displayResults.map((result, index) => (
              <ResultItem
                key={result.id}
                ref={index === selectedIndex ? selectedItemRef : null}
                result={result}
                isSelected={index === selectedIndex}
                query={query}
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
                      await executeAction(result.id, defaultAction.id, result.plugin_id, result.title, result.subtitle, result.icon);
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
        </>
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
  query: string;
}

const ResultItem = React.forwardRef<HTMLDivElement, ResultItemProps>(
  ({ result, isSelected, onClick, onContextMenu, query }, ref) => {
    return (
      <div
        ref={ref}
        className={`result-item flex items-center gap-4 px-6 cursor-pointer ${
          isSelected ? 'result-item-selected' : ''
        }`}
        style={{
          minHeight: 'var(--result-height, 60px)',
          backgroundColor: isSelected 
            ? 'var(--color-primary-alpha)' 
            : 'transparent',
          borderLeft: isSelected ? '3px solid var(--color-primary)' : '3px solid transparent',
        }}
        onClick={onClick}
        onContextMenu={onContextMenu}
      >
        {/* 图标 - 使用动画类 */}
        <div 
          className="result-icon flex-shrink-0 flex items-center justify-center rounded-lg text-2xl" 
          style={{
            width: 'var(--icon-size, 32px)',
            height: 'var(--icon-size, 32px)',
            backgroundColor: isSelected ? 'rgba(255, 255, 255, 0.06)' : 'rgba(255, 255, 255, 0.03)'
          }}
        >
          {(() => {
            if (result.icon.type === 'emoji') {
              return result.icon.data;
            } else if (result.icon.type === 'base64') {
              return (
                <img 
                  src={result.icon.data}
                  alt="icon" 
                  className="object-contain"
                  style={{
                    width: 'calc(var(--icon-size, 32px) * 0.75)',
                    height: 'calc(var(--icon-size, 32px) * 0.75)',
                  }}
                  onError={(e) => {
                    console.error('❌ Base64 icon load failed');
                    e.currentTarget.style.display = 'none';
                    e.currentTarget.parentElement!.textContent = '📄';
                  }}
                  onLoad={() => {
                    console.log('✅ Base64 icon loaded successfully');
                  }}
                />
              );
            } else if (result.icon.type === 'file') {
              // 文件路径需要转换
              const iconSrc = convertFileSrc(result.icon.data);
              return (
                <img 
                  src={iconSrc}
                  alt="icon" 
                  className="w-8 h-8 object-contain"
                  onError={(e) => {
                    console.error('❌ File icon load failed:', {
                      originalPath: result.icon.data,
                      convertedSrc: iconSrc
                    });
                    e.currentTarget.style.display = 'none';
                    e.currentTarget.parentElement!.textContent = '📄';
                  }}
                  onLoad={() => {
                    console.log('✅ File icon loaded successfully');
                  }}
                />
              );
            } else {
              return '📄';
            }
          })()}
        </div>
        
        {/* 文本内容 - 使用动画类 */}
        <div className="flex-1 min-w-0">
          <div className="result-title text-sm font-medium truncate mb-0.5" style={{ color: 'var(--color-text-primary)' }}>
            {highlightMatch(result.title, query)}
          </div>
          {result.subtitle && (
            <div className="result-subtitle text-xs truncate" style={{ color: 'var(--color-text-secondary)' }}>
              {highlightMatch(result.subtitle, query)}
            </div>
          )}
        </div>
      </div>
    );
  }
);

ResultItem.displayName = 'ResultItem';
