// iLauncher - 核心模块
mod clipboard;
mod commands;
mod core;
mod hotkey;
mod plugin;
mod preview;
mod storage;
mod statistics;

use storage::StorageManager;
use tauri::Manager;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ilauncher=debug,tauri=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting iLauncher...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::query,
            commands::execute_action,
            commands::get_plugins,
            commands::show_app,
            commands::hide_app,
            commands::toggle_app,
            commands::load_config,
            commands::save_config,
            commands::clear_cache,
            commands::get_storage_paths,
            commands::get_statistics,
            commands::clear_statistics,
            commands::read_file_preview,
            commands::get_clipboard_history,
            commands::copy_to_clipboard,
            commands::update_clipboard_timestamp,
            commands::delete_clipboard_item,
            commands::toggle_clipboard_favorite,
            commands::clear_clipboard_history,
        ])
        .setup(|app| {
            // 初始化存储管理器
            let storage_manager = storage::StorageManager::new()
                .expect("Failed to create storage manager");
            
            // 加载配置（用于初始化热键）
            let config = tauri::async_runtime::block_on(async {
                storage_manager.load_config().await.unwrap_or_default()
            });
            
            // 将存储管理器添加到应用状态
            app.manage(storage_manager);
            
            // 初始化统计管理器
            let statistics_manager = statistics::StatisticsManager::new()
                .expect("Failed to create statistics manager");
            app.manage(statistics_manager);
            
            // 初始化剪贴板管理器
            let clipboard_manager = clipboard::ClipboardManager::new();
            app.manage(clipboard_manager);
            
            // 启动剪贴板监听
            let app_handle = app.handle().clone();
            clipboard::ClipboardManager::start_monitoring(app_handle);
            
            // 初始化插件管理器（阻塞等待异步初始化）
            let plugin_manager = tauri::async_runtime::block_on(async {
                plugin::PluginManager::new().await
            });
            app.manage(plugin_manager);
            
            // 初始化热键管理器
            let mut hotkey_manager = hotkey::HotkeyManager::new()
                .expect("Failed to create hotkey manager");
            
            // 从配置注册热键
            let hotkey_str = &config.general.hotkey;
            if let Err(e) = hotkey_manager.register_from_string(hotkey_str) {
                tracing::warn!("Failed to register hotkey from config: {}, using default", e);
                hotkey_manager.register_main_hotkey()
                    .expect("Failed to register main hotkey");
            }
            
            // 使用 Box::leak 让热键管理器永久存活
            Box::leak(Box::new(hotkey_manager));
            
            // 启动热键监听器
            let app_handle = app.handle().clone();
            hotkey::HotkeyManager::start_listener(app_handle);
            
            // 预渲染窗口：显示窗口让 React 完成初始化，然后立即隐藏
            // 这样首次按热键时，UI 已经准备好了
            let window = app.get_webview_window("main").unwrap();
            let window_clone = window.clone();
            std::thread::spawn(move || {
                // 等待前端加载完成
                std::thread::sleep(std::time::Duration::from_millis(500));
                
                // 显示窗口触发 React 渲染
                let _ = window_clone.show();
                
                // 立即隐藏
                std::thread::sleep(std::time::Duration::from_millis(100));
                let _ = window_clone.hide();
                
                tracing::info!("Window pre-rendered and hidden");
            });
            
            tracing::info!("iLauncher setup completed");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
