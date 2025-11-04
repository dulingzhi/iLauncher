// iLauncher - 核心模块
mod commands;
mod core;
mod hotkey;
mod plugin;
mod storage;
mod statistics;

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
        ])
        .setup(|app| {
            // 初始化存储管理器
            let storage_manager = storage::StorageManager::new()
                .expect("Failed to create storage manager");
            app.manage(storage_manager);
            
            // 初始化统计管理器
            let statistics_manager = statistics::StatisticsManager::new()
                .expect("Failed to create statistics manager");
            app.manage(statistics_manager);
            
            // 初始化插件管理器（阻塞等待异步初始化）
            let plugin_manager = tauri::async_runtime::block_on(async {
                plugin::PluginManager::new().await
            });
            app.manage(plugin_manager);
            
            // 初始化热键管理器
            let mut hotkey_manager = hotkey::HotkeyManager::new()
                .expect("Failed to create hotkey manager");
            
            // 注册主热键
            hotkey_manager.register_main_hotkey()
                .expect("Failed to register main hotkey");
            
            // 使用 Box::leak 让热键管理器永久存活，防止全局热键注册失效
            Box::leak(Box::new(hotkey_manager));
            
            // 启动热键监听器
            let app_handle = app.handle().clone();
            hotkey::HotkeyManager::start_listener(app_handle);
            
            tracing::info!("iLauncher setup completed");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
