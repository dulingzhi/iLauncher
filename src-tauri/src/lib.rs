// iLauncher - 核心模块
mod clipboard;
mod commands;
mod core;
mod hotkey;
mod plugin;
mod preview;
mod storage;
mod statistics;
mod utils;

// MFT 扫描器模块
#[cfg(target_os = "windows")]
pub mod mft_scanner;

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
            commands::get_plugin_config,
            commands::save_plugin_config,
            commands::show_app,
            commands::hide_app,
            commands::toggle_app,
            commands::load_config,
            commands::save_config,
            commands::toggle_mft,
            commands::get_mft_status,
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
            
            // 如果启用了 MFT，启动 MFT Service 子进程（需要管理员权限）
            #[cfg(target_os = "windows")]
            {
                // 读取 file_search 插件配置
                let storage_for_config = crate::storage::StorageManager::new()
                    .expect("Failed to create storage manager");
                    
                let file_search_config = tauri::async_runtime::block_on(async {
                    storage_for_config.get_plugin_config("file_search").await.ok()
                });
                
                let use_mft = file_search_config
                    .as_ref()
                    .and_then(|cfg| cfg.get("use_mft"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true); // 默认启用
                
                if use_mft {
                    tracing::info!("🚀 MFT is enabled in file_search plugin, starting MFT service with admin rights...");
                    
                    let exe_path = std::env::current_exe()
                        .expect("Failed to get current exe path");
                    
                    // 获取当前 UI 进程的 PID
                    let ui_pid = std::process::id();
                    
                    // 使用 PowerShell Start-Process -Verb RunAs 请求管理员权限
                    // 传递 UI 进程 PID，让 Service 可以监控 UI 进程
                    let ps_command = format!(
                        "Start-Process -FilePath '{}' -ArgumentList '--mft-service','--ui-pid','{}' -Verb RunAs -WindowStyle Hidden",
                        exe_path.display(),
                        ui_pid
                    );
                    
                    match std::process::Command::new("powershell.exe")
                        .args(["-WindowStyle", "Hidden", "-Command", &ps_command])
                        .spawn()
                    {
                        Ok(child) => {
                            tracing::info!("✓ MFT service launch requested with admin elevation (PowerShell PID: {})", child.id());
                            tracing::info!("  UI PID: {}, Service will auto-exit when UI closes", ui_pid);
                            tracing::info!("  User will see UAC prompt if not running as admin");
                        }
                        Err(e) => {
                            tracing::error!("❌ Failed to start MFT service: {}", e);
                            tracing::warn!("  Falling back to BFS mode");
                        }
                    }
                } else {
                    tracing::info!("⚡ MFT is disabled in file_search plugin, will use BFS scanning mode");
                }
            }
            
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
            
            // 预渲染窗口：在后台触发 React 初始化，不抢夺焦点
            // WebView 会在后台加载，窗口保持不可见状态
            std::thread::spawn(move || {
                // 等待前端完全加载
                std::thread::sleep(std::time::Duration::from_millis(800));
                
                tracing::info!("Window pre-rendering completed (background load)");
            });
            
            tracing::info!("iLauncher setup completed");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// MFT 扫描器模式入口（管理员权限）
#[cfg(target_os = "windows")]
pub fn run_mft_scanner() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, fmt};
    use tracing_appender::rolling;
    use crate::utils::paths;
    
    // 创建日志目录（统一到 AppData\Local\iLauncher\logs）
    let log_dir = paths::get_log_dir()
        .expect("Failed to create log directory");
    let file_appender = rolling::daily(&log_dir, "mft_scanner.log");
    
    // 初始化日志（写入文件）
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ilauncher=debug".into()),
        )
        .with(fmt::layer().with_writer(file_appender).with_ansi(false))
        .init();
    
    tracing::info!("========== MFT Scanner Started at {} ==========", 
                   chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    tracing::info!("🚀 Starting MFT Scanner in privileged mode...");
    tracing::info!("📝 Log file: {:?}", log_dir.join("mft_scanner.log"));
    tracing::info!("📂 Database dir: {:?}", paths::get_mft_database_dir().unwrap());
    
    // TODO: 重新实现 MFT Scanner 启动逻辑
    tracing::error!("❌ MFT Scanner has been refactored - use standalone binaries (scanner.exe / monitor.exe)");
    std::process::exit(1);
    
    // // 检查管理员权限
    // if !mft_scanner::UsnScanner::check_admin_rights() {
    //     tracing::error!("❌ Error: MFT Scanner requires administrator rights");
    //     std::process::exit(1);
    // }
    // 
    // // 启动 IPC 服务器
    // if let Err(e) = mft_scanner::ScannerServer::run() {
    //     tracing::error!("❌ Scanner server error: {:#}", e);
    //     std::process::exit(1);
    // }
}

#[cfg(not(target_os = "windows"))]
pub fn run_mft_scanner() {
    eprintln!("MFT Scanner is only available on Windows");
    std::process::exit(1);
}

/// 🔹 运行 MFT Service（全量扫描 + 实时监控）
#[cfg(target_os = "windows")]
pub fn run_mft_service(args: &[String]) {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing::{info, error, warn};
    
    // 初始化日志
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ilauncher=info,mft=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    info!("🚀 MFT Service starting...");
    info!("📅 {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    
    // 解析命令行参数（简单解析，不使用 clap）
    let mut output_dir: Option<String> = None;
    let mut drives_str: Option<String> = None;
    let mut scan_only = false;
    let mut ui_pid: Option<u32> = None;
    
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_dir = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--drives" | "-d" => {
                if i + 1 < args.len() {
                    drives_str = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--scan-only" => {
                scan_only = true;
            }
            "--ui-pid" => {
                if i + 1 < args.len() {
                    ui_pid = args[i + 1].parse::<u32>().ok();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    // 加载配置文件
    let config = match mft_scanner::load_config() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load config: {:#}", e);
            std::process::exit(1);
        }
    };
    info!("✓ Config loaded");
    
    // 确定输出目录（优先使用统一的AppData目录）
    let output_dir = if let Some(dir) = output_dir {
        dir
    } else {
        match crate::utils::paths::get_mft_database_dir() {
            Ok(path) => path.to_string_lossy().to_string(),
            Err(e) => {
                error!("Failed to get MFT database directory: {:#}", e);
                config.output_dir.clone()
            }
        }
    };
    info!("✓ Output directory: {}", output_dir);
    
    // 确定要处理的驱动器
    let drives: Vec<char> = if let Some(drives_str) = drives_str {
        drives_str.split(',')
            .filter_map(|s| s.trim().chars().next())
            .collect()
    } else {
        config.drives.clone()
    };
    
    info!("✓ Drives to process: {:?}", drives);
    
    // ============ 阶段 1: 全量扫描 ============
    info!("");
    info!("╔═══════════════════════════════════════════╗");
    info!("║    Phase 1: Full Disk Scan                ║");
    info!("╚═══════════════════════════════════════════╝");
    info!("");
    
    let scan_start = std::time::Instant::now();
    
    // 多线程扫描所有驱动器
    let handles: Vec<_> = drives
        .iter()
        .map(|&drive| {
            let output_dir_clone = output_dir.clone();
            let config_clone = config.clone();
            
            std::thread::spawn(move || {
                info!("📀 Starting scan for drive {}:", drive);
                
                let mut scanner = mft_scanner::UsnScanner::new(drive);
                
                match scanner.scan_to_database(&output_dir_clone, &config_clone) {
                    Ok(_) => {
                        info!("✅ Drive {} scan completed", drive);
                        Ok(drive)
                    }
                    Err(e) => {
                        error!("❌ Drive {} scan failed: {:#}", drive, e);
                        Err(e)
                    }
                }
            })
        })
        .collect();
    
    // 等待所有扫描完成
    let mut scanned_drives = Vec::new();
    for handle in handles {
        if let Ok(Ok(drive)) = handle.join() {
            scanned_drives.push(drive);
        }
    }
    
    let scan_elapsed = scan_start.elapsed();
    info!("");
    info!("╔═══════════════════════════════════════════╗");
    info!("║    Scan Phase Complete                    ║");
    info!("╚═══════════════════════════════════════════╝");
    info!("⏱️  Total scan time: {:.2}s", scan_elapsed.as_secs_f32());
    info!("✓ Successfully scanned drives: {:?}", scanned_drives);
    info!("");
    
    // 如果只需要扫描，则退出
    if scan_only {
        info!("🏁 Scan-only mode, exiting...");
        std::process::exit(0);
    }
    
    // ============ 阶段 2: 实时监控 ============
    info!("╔═══════════════════════════════════════════╗");
    info!("║    Phase 2: Real-time Monitoring          ║");
    info!("╚═══════════════════════════════════════════╝");
    info!("");
    
    // 为每个成功扫描的驱动器启动监控线程
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    // 启动 UI 进程监控线程
    if let Some(pid) = ui_pid {
        info!("🔍 UI process PID: {}, will auto-exit when UI closes", pid);
        
        let running_for_monitor = running.clone();
        std::thread::spawn(move || {
            monitor_ui_process(pid, running_for_monitor);
        });
    } else {
        warn!("⚠️  No UI PID provided, service will run until manually stopped");
    }
    
    // 设置 Ctrl+C 处理器
    if let Err(e) = ctrlc::set_handler(move || {
        info!("");
        info!("🛑 Received shutdown signal, stopping monitors...");
        r.store(false, Ordering::SeqCst);
    }) {
        error!("Failed to set Ctrl+C handler: {:#}", e);
    }
    
    let monitor_handles: Vec<_> = scanned_drives
        .iter()
        .map(|&drive| {
            let output_dir_clone = output_dir.clone();
            let config_clone = config.clone();
            let running_clone = running.clone();
            
            std::thread::spawn(move || {
                info!("👀 Starting monitor for drive {}:", drive);
                
                let mut monitor = mft_scanner::UsnMonitor::new(drive);
                
                // 启动监控（阻塞式运行，直到收到停止信号）
                match monitor.start_monitoring_with_signal(&output_dir_clone, &config_clone, running_clone) {
                    Ok(_) => {
                        info!("✓ Monitor for drive {} stopped gracefully", drive);
                    }
                    Err(e) => {
                        error!("❌ Monitor for drive {} error: {:#}", drive, e);
                    }
                }
            })
        })
        .collect();
    
    info!("✓ All monitors started");
    info!("💡 Press Ctrl+C to stop monitoring and exit");
    info!("");
    
    // 等待所有监控线程退出
    for handle in monitor_handles {
        handle.join().unwrap();
    }
    
    info!("");
    info!("🎉 MFT Service stopped successfully");
    
    std::process::exit(0);
}

#[cfg(not(target_os = "windows"))]
pub fn run_mft_service(_args: &[String]) {
    eprintln!("MFT Service is only available on Windows");
    std::process::exit(1);
}

/// 监控 UI 进程，当 UI 退出时自动退出 Service
#[cfg(target_os = "windows")]
fn monitor_ui_process(ui_pid: u32, running: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    use std::time::Duration;
    use std::thread;
    use std::sync::atomic::Ordering;
    use tracing::info;
    
    info!("🔍 Starting UI process monitor thread (PID: {})", ui_pid);
    
    loop {
        // 检查进程是否还存在
        let process_exists = check_process_exists(ui_pid);
        
        if !process_exists {
            info!("⚠️  UI process (PID: {}) has exited, shutting down MFT Service...", ui_pid);
            
            // 设置停止标志，让监控线程优雅退出
            running.store(false, Ordering::SeqCst);
            
            // 等待 3 秒让监控线程清理
            thread::sleep(Duration::from_secs(3));
            
            info!("👋 MFT Service exiting due to UI process termination");
            std::process::exit(0);
        }
        
        // 每 2 秒检查一次（缩短检查间隔，更快响应）
        thread::sleep(Duration::from_secs(2));
    }
}

/// 检查 Windows 进程是否存在
#[cfg(target_os = "windows")]
fn check_process_exists(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION};
    
    unsafe {
        // 尝试打开进程句柄
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid);
        
        if let Ok(h) = handle {
            if h.is_invalid() {
                return false;
            }
            
            // 成功打开说明进程存在，关闭句柄
            let _ = CloseHandle(h);
            true
        } else {
            // 无法打开说明进程不存在
            false
        }
    }
}

