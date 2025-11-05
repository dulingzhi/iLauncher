import { useEffect, useState } from "react";
import { getCurrentWindow, LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { SearchBox } from "./components/SearchBox";
import { Settings } from "./components/Settings";
import { PluginManager } from "./components/PluginManager";
import ClipboardHistory from "./components/ClipboardHistory";
import { PreviewPanel } from "./components/PreviewPanel";
import { useAppStore } from "./store/useAppStore";
import { useConfigStore } from "./store/useConfigStore";
import "./index.css";

type View = 'search' | 'settings' | 'plugins' | 'clipboard';

// 不同视图的窗口配置
const VIEW_CONFIGS = {
  search: { width: 1000, height: 650 },
  settings: { width: 1000, height: 700 },
  plugins: { width: 1000, height: 700 },
  clipboard: { width: 800, height: 600 },
};

function App() {
  const [currentView, setCurrentView] = useState<View>('search');
  const [previewPath, setPreviewPath] = useState<string | null>(null);
  const results = useAppStore((state) => state.results);
  const selectedIndex = useAppStore((state) => state.selectedIndex);
  const { config, loadConfig } = useConfigStore();
  const showPreview = config?.appearance.show_preview ?? true;

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

  // 加载配置（仅在应用启动时加载一次）
  useEffect(() => {
    loadConfig();
    
    // 初始化时确保窗口居中
    const initializeWindow = async () => {
      const appWindow = getCurrentWindow();
      try {
        await appWindow.center();
        console.log('Window centered on initialization');
      } catch (error) {
        console.error('Failed to center window:', error);
      }
    };
    
    initializeWindow();
  }, []);

  // 当视图切换时，调整窗口尺寸并保持中心位置
  useEffect(() => {
    const adjustWindowSize = async () => {
      const appWindow = getCurrentWindow();
      const config = VIEW_CONFIGS[currentView];
      
      try {
        // 获取当前窗口位置和尺寸
        const currentPosition = await appWindow.outerPosition();
        const currentSize = await appWindow.outerSize();
        
        // 计算当前窗口中心点
        const centerX = currentPosition.x + currentSize.width / 2;
        const centerY = currentPosition.y + currentSize.height / 2;
        
        // 计算新窗口的左上角位置，保持中心点不变
        const newX = Math.round(centerX - config.width / 2);
        const newY = Math.round(centerY - config.height / 2);
        
        // 先调整尺寸
        await appWindow.setSize(new LogicalSize(config.width, config.height));
        
        // 再调整位置，保持中心点
        await appWindow.setPosition(new PhysicalPosition(newX, newY));
        
        console.log(`Window adjusted for ${currentView}: ${config.width}x${config.height}, center: (${centerX}, ${centerY})`);
      } catch (error) {
        console.error('Failed to adjust window:', error);
      }
    };

    adjustWindowSize();
  }, [currentView]);
  
  useEffect(() => {
    const appWindow = getCurrentWindow();
    
    // 监听窗口显示事件，重置为搜索视图并居中
    const setupShowListener = async () => {
      const unlisten = await appWindow.listen('tauri://focus', async () => {
        setCurrentView('search');
        
        // 每次显示时重新居中窗口
        try {
          await appWindow.center();
          console.log('Window centered on show');
        } catch (error) {
          console.error('Failed to center window on show:', error);
        }
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
