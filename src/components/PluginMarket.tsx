// 插件市场组件（简化版）
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface PluginListItem {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  downloads: number;
  rating: number;
  icon_url: string;
  download_url: string;
  keywords: string[];
}

interface InstalledPlugin {
  manifest: {
    id: string;
    name: string;
    version: string;
    description: string;
    author: { name: string };
  };
  enabled: boolean;
  installed_at: string;
}

export function PluginMarket() {
  const [view, setView] = useState<'discover' | 'installed'>('discover');
  const [plugins, setPlugins] = useState<PluginListItem[]>([]);
  const [installed, setInstalled] = useState<InstalledPlugin[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 加载已安装插件
  useEffect(() => {
    loadInstalled();
  }, []);

  // 加载热门插件
  useEffect(() => {
    if (view === 'discover') {
      loadPopularPlugins();
    }
  }, [view]);

  const loadInstalled = async () => {
    try {
      const data = await invoke<InstalledPlugin[]>('list_installed_plugins');
      setInstalled(data);
    } catch (e) {
      console.error('Failed to load installed plugins:', e);
    }
  };

  const loadPopularPlugins = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<PluginListItem[]>('get_popular_plugins', { limit: 20 });
      setPlugins(data);
    } catch (e: any) {
      setError(e.toString());
    } finally {
      setLoading(false);
    }
  };

  const searchPlugins = async () => {
    if (!searchQuery.trim()) {
      loadPopularPlugins();
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const result = await invoke<any>('search_plugins', {
        query: searchQuery,
        category: null,
        sort: null,
        page: 1,
      });
      setPlugins(result.plugins || []);
    } catch (e: any) {
      setError(e.toString());
    } finally {
      setLoading(false);
    }
  };

  const installPlugin = async (pluginId: string) => {
    try {
      await invoke('install_plugin', { pluginId, version: null });
      alert('插件安装成功！');
      loadInstalled();
    } catch (e: any) {
      alert(`安装失败: ${e}`);
    }
  };

  const uninstallPlugin = async (pluginId: string) => {
    if (!confirm('确定要卸载此插件吗？')) return;
    try {
      await invoke('uninstall_plugin', { pluginId });
      alert('插件已卸载');
      loadInstalled();
    } catch (e: any) {
      alert(`卸载失败: ${e}`);
    }
  };

  const togglePlugin = async (pluginId: string, enabled: boolean) => {
    try {
      await invoke('toggle_plugin', { pluginId, enabled: !enabled });
      loadInstalled();
    } catch (e: any) {
      alert(`切换失败: ${e}`);
    }
  };

  return (
    <div className="plugin-market">
      {/* Header */}
      <div className="market-header">
        <h1>插件市场</h1>
        <div className="view-tabs">
          <button 
            className={view === 'discover' ? 'active' : ''}
            onClick={() => setView('discover')}
          >
            发现 ({plugins.length})
          </button>
          <button 
            className={view === 'installed' ? 'active' : ''}
            onClick={() => setView('installed')}
          >
            已安装 ({installed.length})
          </button>
        </div>
      </div>

      {/* Search Bar (仅在发现页显示) */}
      {view === 'discover' && (
        <div className="search-bar">
          <input
            type="text"
            placeholder="搜索插件..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && searchPlugins()}
          />
          <button onClick={searchPlugins}>搜索</button>
        </div>
      )}

      {/* Content */}
      <div className="market-content">
        {loading && <div className="loading">加载中...</div>}
        {error && <div className="error">{error}</div>}

        {view === 'discover' && !loading && (
          <div className="plugin-grid">
            {plugins.length === 0 ? (
              <div className="empty">暂无插件，插件市场功能正在开发中...</div>
            ) : (
              plugins.map((plugin) => (
                <div key={plugin.id} className="plugin-card">
                  <div className="plugin-icon">
                    {plugin.icon_url ? (
                      <img src={plugin.icon_url} alt={plugin.name} />
                    ) : (
                      <span>🧩</span>
                    )}
                  </div>
                  <div className="plugin-info">
                    <h3>{plugin.name}</h3>
                    <p className="version">v{plugin.version}</p>
                    <p className="description">{plugin.description}</p>
                    <p className="author">作者: {plugin.author}</p>
                    <div className="stats">
                      <span>⭐ {plugin.rating.toFixed(1)}</span>
                      <span>📥 {plugin.downloads}</span>
                    </div>
                  </div>
                  <div className="plugin-actions">
                    {installed.some((p) => p.manifest.id === plugin.id) ? (
                      <button disabled>已安装</button>
                    ) : (
                      <button onClick={() => installPlugin(plugin.id)}>安装</button>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
        )}

        {view === 'installed' && !loading && (
          <div className="installed-list">
            {installed.length === 0 ? (
              <div className="empty">暂无已安装插件</div>
            ) : (
              installed.map((plugin) => (
                <div key={plugin.manifest.id} className="installed-item">
                  <div className="item-info">
                    <h3>{plugin.manifest.name}</h3>
                    <p className="version">v{plugin.manifest.version}</p>
                    <p className="description">{plugin.manifest.description}</p>
                    <p className="author">作者: {plugin.manifest.author.name}</p>
                  </div>
                  <div className="item-actions">
                    <button
                      className={plugin.enabled ? 'enabled' : 'disabled'}
                      onClick={() => togglePlugin(plugin.manifest.id, plugin.enabled)}
                    >
                      {plugin.enabled ? '禁用' : '启用'}
                    </button>
                    <button
                      className="uninstall"
                      onClick={() => uninstallPlugin(plugin.manifest.id)}
                    >
                      卸载
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        )}
      </div>

      <style>{`
        .plugin-market {
          padding: 20px;
          background: var(--color-background);
          color: var(--color-text-primary);
          height: 100vh;
          overflow-y: auto;
        }

        .market-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 20px;
        }

        .market-header h1 {
          font-size: 24px;
          margin: 0;
        }

        .view-tabs {
          display: flex;
          gap: 10px;
        }

        .view-tabs button {
          padding: 8px 16px;
          border: none;
          background: var(--color-surface);
          color: var(--color-text-primary);
          border-radius: 4px;
          cursor: pointer;
          transition: all 0.2s;
        }

        .view-tabs button.active {
          background: var(--color-primary);
          color: white;
        }

        .search-bar {
          display: flex;
          gap: 10px;
          margin-bottom: 20px;
        }

        .search-bar input {
          flex: 1;
          padding: 10px;
          border: 1px solid var(--color-border);
          border-radius: 4px;
          background: var(--color-surface);
          color: var(--color-text-primary);
        }

        .search-bar button {
          padding: 10px 20px;
          border: none;
          background: var(--color-primary);
          color: white;
          border-radius: 4px;
          cursor: pointer;
        }

        .loading, .error, .empty {
          text-align: center;
          padding: 40px;
          color: var(--color-text-muted);
        }

        .error {
          color: #ff4444;
        }

        .plugin-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
          gap: 20px;
        }

        .plugin-card {
          background: var(--color-surface);
          border: 1px solid var(--color-border);
          border-radius: 8px;
          padding: 20px;
          display: flex;
          flex-direction: column;
          gap: 15px;
        }

        .plugin-icon {
          width: 64px;
          height: 64px;
          display: flex;
          align-items: center;
          justify-content: center;
          font-size: 48px;
        }

        .plugin-icon img {
          width: 100%;
          height: 100%;
          object-fit: cover;
          border-radius: 8px;
        }

        .plugin-info h3 {
          margin: 0;
          font-size: 18px;
        }

        .plugin-info .version {
          font-size: 12px;
          color: var(--color-text-muted);
          margin: 4px 0;
        }

        .plugin-info .description {
          font-size: 14px;
          color: var(--color-text-secondary);
          margin: 8px 0;
        }

        .plugin-info .author {
          font-size: 12px;
          color: var(--color-text-muted);
        }

        .stats {
          display: flex;
          gap: 15px;
          font-size: 12px;
          color: var(--color-text-muted);
        }

        .plugin-actions button {
          width: 100%;
          padding: 8px;
          border: none;
          background: var(--color-primary);
          color: white;
          border-radius: 4px;
          cursor: pointer;
          transition: opacity 0.2s;
        }

        .plugin-actions button:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .installed-list {
          display: flex;
          flex-direction: column;
          gap: 15px;
        }

        .installed-item {
          background: var(--color-surface);
          border: 1px solid var(--color-border);
          border-radius: 8px;
          padding: 20px;
          display: flex;
          justify-content: space-between;
          align-items: center;
        }

        .item-info h3 {
          margin: 0 0 8px 0;
          font-size: 18px;
        }

        .item-info .version {
          font-size: 12px;
          color: var(--color-text-muted);
          margin: 4px 0;
        }

        .item-info .description {
          font-size: 14px;
          color: var(--color-text-secondary);
          margin: 8px 0;
        }

        .item-info .author {
          font-size: 12px;
          color: var(--color-text-muted);
        }

        .item-actions {
          display: flex;
          gap: 10px;
        }

        .item-actions button {
          padding: 8px 16px;
          border: none;
          border-radius: 4px;
          cursor: pointer;
          transition: opacity 0.2s;
        }

        .item-actions button.enabled {
          background: var(--color-primary);
          color: white;
        }

        .item-actions button.disabled {
          background: var(--color-surface);
          color: var(--color-text-muted);
          border: 1px solid var(--color-border);
        }

        .item-actions button.uninstall {
          background: #ff4444;
          color: white;
        }
      `}</style>
    </div>
  );
}
