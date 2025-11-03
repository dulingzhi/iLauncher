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
    
    // 监听窗口失焦事件，自动隐藏
    const setupListener = async () => {
      const unlisten = await appWindow.onFocusChanged(({ payload: focused }) => {
        if (!focused) {
          setTimeout(() => {
            invoke("hide_app");
          }, 150);
        }
      });
      return unlisten;
    };
    
    const unlistenPromise = setupListener();
    
    return () => {
      unlistenPromise.then(fn => fn());
    };
  }, []);

  return (
    <div className="w-full h-screen flex items-start justify-center pt-4 px-4">
      {currentView === 'search' ? (
        <div className="w-full max-w-2xl rounded-lg shadow-2xl overflow-hidden bg-white/95 backdrop-blur-sm">
          <SearchBox onOpenSettings={() => setCurrentView('settings')} onOpenPlugins={() => setCurrentView('plugins')} />
        </div>
      ) : (
        <div className="w-full h-full overflow-auto">
          {currentView === 'settings' && <Settings onClose={() => setCurrentView('search')} />}
          {currentView === 'plugins' && <PluginManager onClose={() => setCurrentView('search')} />}
        </div>
      )}
    </div>
  );
}

export default App;
