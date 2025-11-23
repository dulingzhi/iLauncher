import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Clipboard, Search, Trash2, Copy, Image, File, Type, Star } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface ClipboardItem {
  id: string;
  type: 'text' | 'image' | 'rich_text';
  content: string;
  preview?: string;
  timestamp: number;
  favorite?: boolean;
  file_path?: string;  // 图片文件路径
  category?: string;
  tags?: string[];
}

interface ClipboardHistoryProps {
  onClose?: () => void;
}

const ClipboardHistory: React.FC<ClipboardHistoryProps> = ({ onClose }) => {
  const { t } = useTranslation();
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState<'all' | 'text' | 'image' | 'favorites'>('all');
  const [stats, setStats] = useState({ total: 0, favorites: 0, text: 0, image: 0 });

  useEffect(() => {
    loadHistory();
    loadStats();
    
    // 监听剪贴板更新事件
    const unlisten = listen('clipboard:updated', () => {
      loadHistory();
      loadStats();
    });
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, [filter]);

  // 监听 ESC 键关闭
  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && onClose) {
        onClose();
      }
    };
    
    window.addEventListener('keydown', handleEsc);
    return () => window.removeEventListener('keydown', handleEsc);
  }, [onClose]);

  const loadHistory = async () => {
    try {
      setLoading(true);
      if (filter === 'favorites') {
        const favorites = await invoke<ClipboardItem[]>('get_clipboard_favorites');
        setItems(favorites);
      } else {
        const history = await invoke<ClipboardItem[]>('get_clipboard_history', {
          limit: 100,
          offset: 0
        });
        setItems(history.filter(item => 
          filter === 'all' || item.type === filter
        ));
      }
    } catch (error) {
      console.error('Failed to load clipboard history:', error);
    } finally {
      setLoading(false);
    }
  };

  const loadStats = async () => {
    try {
      const [total, favorites, text, image] = await invoke<[number, number, number, number]>('get_clipboard_stats');
      setStats({ total, favorites, text, image });
    } catch (error) {
      console.error('Failed to load stats:', error);
    }
  };

  const copyToClipboard = async (item: ClipboardItem) => {
    try {
      await invoke('copy_to_clipboard', { 
        content: item.content,
        contentType: item.type
      });
      if (onClose) onClose();  // 复制后关闭窗口
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
    }
  };

  const deleteItem = async (id: string) => {
    try {
      await invoke('delete_clipboard_item', { id });
      setItems(items.filter(item => item.id !== id));
      loadStats();
    } catch (error) {
      console.error('Failed to delete item:', error);
    }
  };

  const toggleFavorite = async (id: string) => {
    try {
      await invoke('toggle_clipboard_favorite', { id });
      await loadHistory();
      await loadStats();
    } catch (error) {
      console.error('Failed to toggle favorite:', error);
    }
  };

  const clearHistory = async () => {
    if (!window.confirm(t('clipboard.confirmClear'))) return;
    try {
      await invoke('clear_clipboard_history');
      setItems([]);
      await loadStats();
    } catch (error) {
      console.error('Failed to clear history:', error);
    }
  };

  const handleSearch = async (query: string) => {
    setSearchQuery(query);
    if (!query.trim()) {
      await loadHistory();
      return;
    }
    
    try {
      const results = await invoke<ClipboardItem[]>('search_clipboard', {
        query,
        limit: 50
      });
      setItems(results);
    } catch (error) {
      console.error('Failed to search:', error);
    }
  };

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp * 1000); // 从秒转换为毫秒
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

  const getTypeIcon = (type: string) => {
    switch (type) {
      case 'image': return <Image className="w-4 h-4" />;
      case 'rich_text': return <File className="w-4 h-4" />;
      default: return <Type className="w-4 h-4" />;
    }
  };

  return (
    <div className="flex flex-col h-full" style={{ backgroundColor: 'var(--color-background)' }}>
      {/* 头部 */}
      <div 
        data-tauri-drag-region
        className="p-4" 
        style={{ 
          backgroundColor: 'var(--color-surface)', 
          borderBottom: '1px solid var(--color-border)' 
        }}
      >
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

        {/* 过滤器 */}
        <div className="flex gap-2 mb-3">
          {[
            { key: 'all', label: t('clipboard.all'), icon: Clipboard },
            { key: 'text', label: t('clipboard.textOnly'), icon: Type },
            { key: 'image', label: t('clipboard.imageOnly'), icon: Image },
            { key: 'favorites', label: t('clipboard.favoritesOnly'), icon: Star },
          ].map(({ key, label, icon: Icon }) => (
            <button
              key={key}
              onClick={() => setFilter(key as typeof filter)}
              className="flex items-center gap-1 px-3 py-1.5 text-sm rounded-md transition-colors"
              style={{
                backgroundColor: filter === key ? 'var(--color-accent)' : 'transparent',
                color: filter === key ? 'white' : 'var(--color-text-secondary)',
                border: `1px solid ${filter === key ? 'var(--color-accent)' : 'var(--color-border)'}`,
              }}
            >
              <Icon className="w-3.5 h-3.5" />
              <span>{label}</span>
            </button>
          ))}
        </div>

        {/* 搜索框 */}
        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4" style={{ color: 'var(--color-text-secondary)' }} />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => handleSearch(e.target.value)}
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
        ) : items.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-32" style={{ color: 'var(--color-text-secondary)' }}>
            <Clipboard className="w-12 h-12 mb-2 opacity-50" />
            <p>{searchQuery ? t('clipboard.noResults') : t('clipboard.empty')}</p>
          </div>
        ) : (
          <div className="space-y-2">
            {items.map(item => (
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
                          {item.content && (
                            <img 
                              src={item.content} 
                              alt="Clipboard image" 
                              className="w-20 h-20 object-cover rounded border"
                              style={{ borderColor: 'var(--color-border)' }}
                            />
                          )}
                          <div className="text-sm" style={{ color: 'var(--color-text-secondary)' }}>
                            <div>{t('clipboard.image')}</div>
                            {item.preview && <div className="text-xs">{item.preview}</div>}
                          </div>
                        </div>
                      ) : item.type === 'rich_text' ? (
                        <div className="text-sm" style={{ color: 'var(--color-text)' }}>
                          <p className="line-clamp-2">{item.preview || item.content.substring(0, 100)}</p>
                          <span className="text-xs" style={{ color: 'var(--color-accent)' }}>Rich Text</span>
                        </div>
                      ) : (
                        <div className="text-sm" style={{ color: 'var(--color-text)' }}>
                          <span className="font-mono">{item.content}</span>
                        </div>
                      )}
                      <div className="flex items-center gap-2 mt-1">
                        <p className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                          {formatTimestamp(item.timestamp)}
                        </p>
                        {item.category && (
                          <span className="text-xs px-2 py-0.5 rounded" style={{
                            backgroundColor: 'var(--color-accent)',
                            color: 'white'
                          }}>
                            {item.category}
                          </span>
                        )}
                        {item.tags && item.tags.length > 0 && (
                          <div className="flex gap-1">
                            {item.tags.map((tag, idx) => (
                              <span key={idx} className="text-xs px-1.5 py-0.5 rounded" style={{
                                backgroundColor: 'var(--color-hover)',
                                color: 'var(--color-text-secondary)'
                              }}>
                                #{tag}
                              </span>
                            ))}
                          </div>
                        )}
                      </div>
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
                        <Star className={`w-4 h-4 ${item.favorite ? 'fill-current' : ''}`} />
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
        <div className="flex justify-center gap-6 text-xs" style={{ color: 'var(--color-text-secondary)' }}>
          <span>{t('clipboard.total')}: {stats.total}</span>
          <span>{t('clipboard.favorites')}: {stats.favorites}</span>
          <span>{t('clipboard.text')}: {stats.text}</span>
          <span>{t('clipboard.images')}: {stats.image}</span>
        </div>
      </div>
    </div>
  );
};

export default ClipboardHistory;
