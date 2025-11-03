// iLauncher - 核心模块
mod commands;
mod core;
mod hotkey;

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
        ])
        .setup(|app| {
            // 初始化热键管理器
            let mut hotkey_manager = hotkey::HotkeyManager::new()
                .expect("Failed to create hotkey manager");
            
            // 注册主热键
            hotkey_manager.register_main_hotkey()
                .expect("Failed to register main hotkey");
            
            // 启动热键监听器
            let app_handle = app.handle().clone();
            hotkey::HotkeyManager::start_listener(app_handle);
            
            tracing::info!("iLauncher setup completed");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
