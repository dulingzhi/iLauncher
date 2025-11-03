import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { useThemeStore } from "./stores/themeStore";
import { applyTheme, getTheme } from "./theme";

// 初始化主题
const initialTheme = getTheme(useThemeStore.getState().currentTheme);
applyTheme(initialTheme);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
