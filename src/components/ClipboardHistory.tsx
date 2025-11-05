import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Clipboard, Search, Trash2, Copy, Image, File, Type } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface ClipboardItem {
  id: string;
  type: 'text' | 'image' | 'file';
  content: string;
  preview?: string;
  timestamp: number;
  favorite?: boolean;
}

const ClipboardHistory: React.FC = () => {
  const { t } = useTranslation();
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadHistory();
  }, []);

  const loadHistory = async () => {
    try {
      setLoading(true);
      const history = await invoke<ClipboardItem[]>('get_clipboard_history');
      setItems(history);
    } catch (error) {
      console.error('Failed to load clipboard history:', error);
    } finally {
      setLoading(false);
    }
  };

  const copyToClipboard = async (item: ClipboardItem) => {
    try {
      await invoke('copy_to_clipboard', { content: item.content });
      // 更新时间戳到最新
      await invoke('update_clipboard_timestamp', { id: item.id });
      await loadHistory();
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
    }
  };

  const deleteItem = async (id: string) => {
    try {
      await invoke('delete_clipboard_item', { id });
      setItems(items.filter(item => item.id !== id));
    } catch (error) {
      console.error('Failed to delete item:', error);
    }
  };

  const toggleFavorite = async (id: string) => {
    try {
      await invoke('toggle_clipboard_favorite', { id });
      await loadHistory();
    } catch (error) {
      console.error('Failed to toggle favorite:', error);
    }
  };

  const clearHistory = async () => {
    if (!window.confirm(t('clipboard.confirmClear'))) return;
    try {
      await invoke('clear_clipboard_history');
      setItems([]);
    } catch (error) {
      console.error('Failed to clear history:', error);
    }
  };

  const filteredItems = items.filter(item => {
    if (!searchQuery) return true;
    const query = searchQuery.toLowerCase();
    return item.content.toLowerCase().includes(query) ||
           item.preview?.toLowerCase().includes(query);
  });

  const getTypeIcon = (type: string) => {
    switch (type) {
      case 'image': return <Image className="w-4 h-4" />;
      case 'file': return <File className="w-4 h-4" />;
      default: return <Type className="w-4 h-4" />;
    }
  };

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return t('clipboard.justNow');
    if (diffMins < 60) return t('clipboard.minutesAgo', { count: diffMins });
    if (diffHours < 24) return t('clipboard.hoursAgo', { count: diffHours });
    if (diffDays < 7) return t('clipboard.daysAgo', { count: diffDays });
    return date.toLocaleDateString();
  };

  return (
    <div className="flex flex-col h-full" style={{ backgroundColor: 'var(--color-background)' }}>
      {/* 头部 */}
      <div className="p-4" style={{ 
        backgroundColor: 'var(--color-surface)', 
        borderBottom: '1px solid var(--color-border)' 
      }}>
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Clipboard className="w-5 h-5" style={{ color: 'var(--color-accent)' }} />
            <h2 className="text-lg font-semibold" style={{ color: 'var(--color-text)' }}>{t('clipboard.title')}</h2>
          </div>
          <button
            onClick={clearHistory}
            className="px-3 py-1.5 text-sm rounded-md transition-colors"
            style={{ 
              color: 'var(--color-danger, #e53e3e)',
              backgroundColor: 'transparent'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = 'var(--color-hover)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = 'transparent';
            }}
          >
            {t('clipboard.clearAll')}
          </button>
        </div>

        {/* 搜索框 */}
        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4" style={{ color: 'var(--color-text-secondary)' }} />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t('clipboard.search')}
            className="w-full pl-10 pr-4 py-2 border-0 rounded-md outline-none clipboard-search-input"
            style={{
              backgroundColor: 'var(--color-input-background)',
              color: 'var(--color-text)',
            }}
            onFocus={(e) => {
              e.currentTarget.style.boxShadow = '0 0 0 2px var(--color-accent)';
            }}
            onBlur={(e) => {
              e.currentTarget.style.boxShadow = 'none';
            }}
          />
        </div>
      </div>

      {/* 列表 */}
      <div className="flex-1 overflow-y-auto p-4">
        {loading ? (
          <div className="flex items-center justify-center h-32">
            <div style={{ color: 'var(--color-text-secondary)' }}>{t('common.loading')}</div>
          </div>
        ) : filteredItems.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-32" style={{ color: 'var(--color-text-secondary)' }}>
            <Clipboard className="w-12 h-12 mb-2 opacity-50" />
            <p>{searchQuery ? t('clipboard.noResults') : t('clipboard.empty')}</p>
          </div>
        ) : (
          <div className="space-y-2">
            {filteredItems.map(item => (
              <div
                key={item.id}
                className="group rounded-lg transition-all cursor-pointer"
                style={{
                  backgroundColor: 'var(--color-surface)',
                  border: '1px solid var(--color-border)',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.borderColor = 'var(--color-accent)';
                  e.currentTarget.style.backgroundColor = 'var(--color-hover)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.borderColor = 'var(--color-border)';
                  e.currentTarget.style.backgroundColor = 'var(--color-surface)';
                }}
                onClick={() => copyToClipboard(item)}
              >
                <div className="p-3">
                  <div className="flex items-start gap-3">
                    {/* 类型图标 */}
                    <div className="flex-shrink-0 mt-1" style={{ color: 'var(--color-text-secondary)' }}>
                      {getTypeIcon(item.type)}
                    </div>

                    {/* 内容 */}
                    <div className="flex-1 min-w-0">
                      {item.type === 'text' ? (
                        <p className="text-sm line-clamp-2" style={{ color: 'var(--color-text)' }}>
                          {item.content}
                        </p>
                      ) : item.type === 'image' ? (
                        <div className="flex items-center gap-2">
                          {item.preview && (
                            <img 
                              src={item.preview} 
                              alt="Preview" 
                              className="w-16 h-16 object-cover rounded"
                            />
                          )}
                          <span className="text-sm" style={{ color: 'var(--color-text-secondary)' }}>{t('clipboard.image')}</span>
                        </div>
                      ) : (
                        <div className="text-sm" style={{ color: 'var(--color-text)' }}>
                          <span className="font-mono">{item.content}</span>
                        </div>
                      )}
                      <p className="text-xs mt-1" style={{ color: 'var(--color-text-secondary)' }}>
                        {formatTimestamp(item.timestamp)}
                      </p>
                    </div>

                    {/* 操作按钮 */}
                    <div className="flex-shrink-0 flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          toggleFavorite(item.id);
                        }}
                        className="p-1.5 rounded transition-colors"
                        style={{ 
                          color: item.favorite ? '#ecc94b' : 'var(--color-text-secondary)',
                          backgroundColor: 'transparent'
                        }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.backgroundColor = 'var(--color-hover)';
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.backgroundColor = 'transparent';
                        }}
                        title={t('clipboard.favorite')}
                      >
                        <svg className="w-4 h-4" fill={item.favorite ? 'currentColor' : 'none'} stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
                        </svg>
                      </button>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          copyToClipboard(item);
                        }}
                        className="p-1.5 rounded transition-colors"
                        style={{ 
                          color: 'var(--color-text-secondary)',
                          backgroundColor: 'transparent'
                        }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.backgroundColor = 'var(--color-hover)';
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.backgroundColor = 'transparent';
                        }}
                        title={t('clipboard.copy')}
                      >
                        <Copy className="w-4 h-4" />
                      </button>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          deleteItem(item.id);
                        }}
                        className="p-1.5 rounded transition-colors"
                        style={{ 
                          color: 'var(--color-text-secondary)',
                          backgroundColor: 'transparent'
                        }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.backgroundColor = 'var(--color-hover)';
                          e.currentTarget.style.color = 'var(--color-danger, #e53e3e)';
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.backgroundColor = 'transparent';
                          e.currentTarget.style.color = 'var(--color-text-secondary)';
                        }}
                        title={t('common.delete')}
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* 底部统计 */}
      <div className="p-3" style={{ 
        backgroundColor: 'var(--color-surface)', 
        borderTop: '1px solid var(--color-border)' 
      }}>
        <p className="text-xs text-center" style={{ color: 'var(--color-text-secondary)' }}>
          {t('clipboard.total', { count: items.length })}
        </p>
      </div>
    </div>
  );
};

export default ClipboardHistory;
