import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { SearchBox } from "./components/SearchBox";
import "./index.css";

function App() {
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
      <div className="w-full max-w-2xl rounded-lg shadow-2xl overflow-hidden bg-white/95 backdrop-blur-sm">
        <SearchBox />
      </div>
    </div>
  );
}

export default App;
