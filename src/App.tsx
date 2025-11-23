import { useEffect, useState, useRef } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { SearchBox } from "./components/SearchBox";
import { Settings } from "./components/Settings";
import { PluginManager } from "./components/PluginManager";
import ClipboardHistory from "./components/ClipboardHistory";
import AIChat from "./components/AIChat";
import { PreviewPanel } from "./components/PreviewPanel";
import { Toast } from "./components/Toast";
import { HotkeyGuide } from "./components/HotkeyGuide";
import { WelcomeGuide } from "./components/WelcomeGuide";
import { useAppStore } from "./store/useAppStore";
import { useConfigStore } from "./store/useConfigStore";
import { useToast } from "./hooks/useToast";
import "./index.css";

type View = 'search' | 'settings' | 'plugins' | 'clipboard' | 'ai-chat';

// 不同视图的窗口配置
const VIEW_CONFIGS = {
  search: { width: 700, height: 580 },      // 🔥 增加高度以完整显示结果列表（搜索框60px + 列表0px + 边襰7px）
  settings: { width: 1000, height: 700 },   // 设置页面使用宽窗口
  plugins: { width: 1000, height: 700 },    // 插件管理使用宽窗口
  clipboard: { width: 900, height: 650 },   // 剪贴板历史使用中等宽度
  'ai-chat': { width: 1200, height: 800 },  // AI 聊天使用大窗口
};

function App() {
  const [currentView, setCurrentView] = useState<View>('search');
  const [previewPath, setPreviewPath] = useState<string | null>(null);
  const [showHotkeyGuide, setShowHotkeyGuide] = useState(false);
  const [showWelcomeGuide, setShowWelcomeGuide] = useState(false);
  const results = useAppStore((state) => state.results);
  const selectedIndex = useAppStore((state) => state.selectedIndex);
  const { config, loadConfig } = useConfigStore();
  const { message, type, visible, hideToast } = useToast();
  const showPreview = config?.appearance.show_preview ?? true;
  
  // 使用ref来保存最新的currentView，避免闭包陈旧问题
  const currentViewRef = useRef(currentView);
  useEffect(() => {
    console.log('[App] Current view changed to:', currentView);
    currentViewRef.current = currentView;
  }, [currentView]);

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
    const initialize = async () => {
      await loadConfig();
      
      // 检查是否是首次启动
      const hasShownWelcome = localStorage.getItem('ilauncher_welcome_shown');
      if (!hasShownWelcome) {
        setShowWelcomeGuide(true);
      }
      
      // 🔥 移除初始化时的居中逻辑，避免窗口闪现
      // 窗口会在首次通过热键显示时自动居中
      console.log('App initialized and ready');
    };
    
    initialize();
  }, []);

  // 当视图切换时，调整窗口尺寸并居中
  useEffect(() => {
    const adjustWindowSize = async () => {
      const appWindow = getCurrentWindow();
      const config = VIEW_CONFIGS[currentView];
      
      try {
        // 临时允许调整大小
        await appWindow.setResizable(true);
        
        // 调整尺寸
        await appWindow.setSize(new LogicalSize(config.width, config.height));
        
        // 居中窗口
        await appWindow.center();
        
        // 禁止用户手动调整大小
        await appWindow.setResizable(false);
        
        console.log(`Window adjusted for ${currentView}: ${config.width}x${config.height} (centered)`);
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
        // setCurrentView('search');
        
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
    
    // 监听打开设置事件（从托盘菜单触发）
    const setupOpenSettingsListener = async () => {
      const unlisten = await appWindow.listen('open-settings', () => {
        console.log('Opening settings from tray menu');
        setCurrentView('settings');
      });
      return unlisten;
    };
    
    // 监听窗口失焦事件，自动隐藏并切换回搜索视图（但设置界面除外）
    const setupBlurListener = async () => {
      const unlisten = await appWindow.onFocusChanged(({ payload: focused }) => {
        console.log('[Blur Listener] Focus changed:', { focused, currentView: currentViewRef.current });
        // 只在搜索视图失焦时隐藏窗口，设置界面失焦不隐藏
        if (!focused && currentViewRef.current === 'search') {
          console.log('[Blur Listener] Hiding app because in search view');
          setTimeout(() => {
            invoke("hide_app");
          }, 100);
        } else if (!focused) {
          console.log('[Blur Listener] Not hiding app, current view is:', currentViewRef.current);
        }
      });
      return unlisten;
    };
    
    const showListenerPromise = setupShowListener();
    const openSettingsListenerPromise = setupOpenSettingsListener();
    const blurListenerPromise = setupBlurListener();
    
    return () => {
      showListenerPromise.then(fn => fn());
      openSettingsListenerPromise.then(fn => fn());
      blurListenerPromise.then(fn => fn());
    };
  }, []);

  return (
    <div className="w-full h-screen flex items-start justify-center">
      {/* Toast 通知 */}
      {visible && (
        <Toast
          message={message}
          type={type}
          onClose={hideToast}
        />
      )}
      
      {/* 快捷键指南 */}
      {showHotkeyGuide && (
        <HotkeyGuide onClose={() => setShowHotkeyGuide(false)} />
      )}
      
      {/* 欢迎指南 */}
      {showWelcomeGuide && (
        <WelcomeGuide onClose={() => setShowWelcomeGuide(false)} />
      )}
      
      {currentView === 'search' ? (
        <div className="w-full flex gap-4 p-4">
          {/* 搜索框区域 */}
          <div 
            className={`rounded-lg shadow-2xl overflow-hidden ${showPreview && previewPath ? 'flex-1' : 'w-full'}`}
            style={{ backgroundColor: 'var(--color-surface)', opacity: 0.98 }}
          >
            <SearchBox 
              onOpenSettings={() => setCurrentView('settings')}
              onOpenPlugins={() => setCurrentView('plugins')}
              onOpenClipboard={() => setCurrentView('clipboard')}
              onOpenAIChat={async () => {
                // 确保窗口显示
                await invoke("show_app");
                setCurrentView('ai-chat');
              }}
              onShowHotkeyGuide={() => setShowHotkeyGuide(true)}
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
          {currentView === 'settings' && <Settings onClose={() => { setCurrentView('search'); }} />}
          {currentView === 'plugins' && <PluginManager onClose={() => { invoke("hide_app"); setCurrentView('search'); }} />}
          {currentView === 'clipboard' && <ClipboardHistory onClose={() => { invoke("hide_app"); setCurrentView('search'); }} />}
          {currentView === 'ai-chat' && <AIChat onClose={() => { setCurrentView('search'); invoke("show_app"); }} />}
        </div>
      )}
    </div>
  );
}

export default App;
