import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { SearchBox } from "./components/SearchBox";
import { Settings } from "./components/Settings";
import { PluginManager } from "./components/PluginManager";
import ClipboardHistory from "./components/ClipboardHistory";
import { PreviewPanel } from "./components/PreviewPanel";
import { useAppStore } from "./store/useAppStore";
import "./index.css";

type View = 'search' | 'settings' | 'plugins' | 'clipboard';

function App() {
  const [currentView, setCurrentView] = useState<View>('search');
  const [showPreview, setShowPreview] = useState(true);
  const [previewPath, setPreviewPath] = useState<string | null>(null);
  const results = useAppStore((state) => state.results);
  const selectedIndex = useAppStore((state) => state.selectedIndex);

  // 当选中项变化时更新预览
  useEffect(() => {
    if (showPreview && results.length > 0 && selectedIndex >= 0 && selectedIndex < results.length) {
      const result = results[selectedIndex];
      // 只预览文件插件的结果
      if (result.plugin_id === 'file_search') {
        setPreviewPath(result.id);
      } else {
        setPreviewPath(null);
      }
    } else {
      setPreviewPath(null);
    }
  }, [selectedIndex, results, showPreview]);

  // 加载预览设置
  useEffect(() => {
    const loadPreviewSetting = async () => {
      try {
        const config = await invoke<any>('load_config');
        setShowPreview(config.appearance.show_preview ?? true);
      } catch (error) {
        console.error('Failed to load preview setting:', error);
      }
    };
    loadPreviewSetting();
  }, []);

  // 当从设置页面返回时重新加载预览设置
  useEffect(() => {
    if (currentView === 'search') {
      const reloadPreviewSetting = async () => {
        try {
          const config = await invoke<any>('load_config');
          setShowPreview(config.appearance.show_preview ?? true);
        } catch (error) {
          console.error('Failed to reload preview setting:', error);
        }
      };
      reloadPreviewSetting();
    }
  }, [currentView]);
  
  useEffect(() => {
    const appWindow = getCurrentWindow();
    
    // 监听窗口显示事件，重置为搜索视图
    const setupShowListener = async () => {
      const unlisten = await appWindow.listen('tauri://focus', () => {
        setCurrentView('search');
      });
      return unlisten;
    };
    
    // 监听窗口失焦事件，自动隐藏
    const setupBlurListener = async () => {
      const unlisten = await appWindow.onFocusChanged(({ payload: focused }) => {
        if (!focused) {
          setTimeout(() => {
            invoke("hide_app");
          }, 150);
        }
      });
      return unlisten;
    };
    
    const showListenerPromise = setupShowListener();
    const blurListenerPromise = setupBlurListener();
    
    return () => {
      showListenerPromise.then(fn => fn());
      blurListenerPromise.then(fn => fn());
    };
  }, []);

  return (
    <div className="w-full h-screen flex items-start justify-center pt-4 px-4">
      {currentView === 'search' ? (
        <div className="w-full max-w-5xl flex gap-4">
          {/* 搜索框区域 */}
          <div 
            className={`rounded-lg shadow-2xl overflow-hidden ${showPreview && previewPath ? 'w-2/3' : 'w-full max-w-2xl'}`}
            style={{ backgroundColor: 'var(--color-surface)', opacity: 0.98 }}
          >
            <SearchBox 
              onOpenSettings={() => setCurrentView('settings')} 
              onOpenPlugins={() => setCurrentView('plugins')}
              onOpenClipboard={() => setCurrentView('clipboard')}
            />
          </div>
          
          {/* 预览面板区域 */}
          {showPreview && previewPath && (
            <div 
              className="w-1/3 rounded-lg shadow-2xl overflow-hidden"
              style={{ backgroundColor: 'var(--color-surface)', opacity: 0.98, maxHeight: '600px' }}
            >
              <PreviewPanel filePath={previewPath} />
            </div>
          )}
        </div>
      ) : (
        <div className="w-full h-full overflow-auto rounded-lg" style={{ backgroundColor: 'var(--color-background)', opacity: 0.98 }}>
          {currentView === 'settings' && <Settings onClose={() => setCurrentView('search')} />}
          {currentView === 'plugins' && <PluginManager onClose={() => setCurrentView('search')} />}
          {currentView === 'clipboard' && <ClipboardHistory />}
        </div>
      )}
    </div>
  );
}

export default App;
