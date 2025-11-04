import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { SearchBox } from "./components/SearchBox";
import { Settings } from "./components/Settings";
import { PluginManager } from "./components/PluginManager";
import "./index.css";

type View = 'search' | 'settings' | 'plugins';

function App() {
  const [currentView, setCurrentView] = useState<View>('search');
  
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
        <div className="w-full max-w-2xl rounded-lg shadow-2xl overflow-hidden" style={{ backgroundColor: 'var(--color-surface)', opacity: 0.98 }}>
          <SearchBox onOpenSettings={() => setCurrentView('settings')} onOpenPlugins={() => setCurrentView('plugins')} />
        </div>
      ) : (
        <div className="w-full h-full overflow-auto rounded-lg" style={{ backgroundColor: 'var(--color-background)', opacity: 0.98 }}>
          {currentView === 'settings' && <Settings onClose={() => setCurrentView('search')} />}
          {currentView === 'plugins' && <PluginManager onClose={() => setCurrentView('search')} />}
        </div>
      )}
    </div>
  );
}

export default App;
