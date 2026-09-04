// iLauncher - 核心模块
mod clipboard;
mod commands;
mod core;
mod hotkey;
mod plugin;
mod preview;
mod ranking;
mod search_history;
mod storage;
mod statistics;
mod utils;

// MFT 扫描器模块
#[cfg(target_os = "windows")]
pub mod mft_scanner;

/// IndexV2 列式快照索引（v3 磁盘格式，Phase 1 引入）
pub mod index_v2;

use std::sync::Arc; // 用于 PluginMarketState
use tauri::Manager;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, fmt::time::OffsetTime};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tracing_appender::rolling;
    use crate::utils::paths;
    
    // 🔥 创建日志目录和文件写入器
    let log_dir = paths::get_log_dir()
        .expect("Failed to create log directory");
    let file_appender = rolling::never(&log_dir, "ilauncher.log");
    
    // 初始化日志（同时输出到控制台和文件）
    let local_timer = OffsetTime::local_rfc_3339().expect("Failed to get local offset");
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ilauncher=debug,tauri=info".into()),
        )
        .with(tracing_subscriber::fmt::layer()
            .with_timer(local_timer.clone())) // 控制台输出（本地时区）
        .with(tracing_subscriber::fmt::layer() // 文件输出（无颜色，本地时区）
            .with_writer(file_appender)
            .with_ansi(false)
            .with_timer(local_timer))
        .init();

    tracing::info!("========== iLauncher Started at {} ==========", 
                   chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    tracing::info!("📝 Log file: {:?}", log_dir.join("ilauncher.log"));
    tracing::info!("Starting iLauncher...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            commands::get_config,
            commands::save_config,
            commands::toggle_mft,
            commands::get_mft_status,
            commands::clear_cache,
            commands::get_storage_paths,
            commands::get_statistics,
            commands::clear_statistics,
            commands::read_file_preview,
            commands::get_clipboard_history,
            commands::search_clipboard,
            commands::get_clipboard_favorites,
            commands::copy_to_clipboard,
            commands::delete_clipboard_item,
            commands::toggle_clipboard_favorite,
            commands::set_clipboard_category,
            commands::add_clipboard_tag,
            commands::clear_clipboard_history,
            commands::get_clipboard_stats,
            commands::enable_autostart,
            commands::disable_autostart,
            commands::is_autostart_enabled,
            commands::set_autostart,
            commands::get_search_history,
            commands::clear_search_history,
            commands::remove_search_history,
            commands::get_search_suggestions,
            commands::record_search_execution,
            commands::get_sandbox_config,
            commands::update_sandbox_config,
            commands::get_plugin_permissions,
            commands::check_plugin_permission,
            commands::audit::get_audit_log,
            commands::audit::get_plugin_audit_log,
            commands::audit::get_violations,
            commands::audit::get_audit_statistics,
            commands::audit::clear_audit_log,
            commands::audit::export_audit_log,
            commands::ai::get_ai_config,
            commands::ai::save_ai_config,
            commands::ai::send_ai_message,
            commands::ai::create_ai_conversation,
            commands::ai::get_ai_conversations,
            commands::ai::switch_ai_conversation,
            commands::ai::delete_ai_conversation,
            commands::plugin_market::search_plugins,
            commands::plugin_market::get_plugin_details,
            commands::plugin_market::install_plugin,
            commands::plugin_market::uninstall_plugin,
            commands::plugin_market::update_plugin,
            commands::plugin_market::list_installed_plugins,
            commands::plugin_market::toggle_plugin,
            commands::plugin_market::update_plugin_settings,
            commands::plugin_market::check_plugin_updates,
            commands::plugin_market::get_popular_plugins,
            commands::plugin_market::get_recent_plugins,
            commands::plugin_market::get_plugins_by_category,
            commands::plugin_market::clear_plugin_cache,
            commands::plugin_market::install_plugin_from_file,
            commands::workflow::list_workflows,
            commands::workflow::get_workflow,
            commands::workflow::save_workflow,
            commands::workflow::delete_workflow,
            commands::workflow::execute_workflow,
            commands::workflow::find_workflows_by_keyword,
            commands::suggestion::get_smart_suggestions,
            commands::suggestion::get_frequent_suggestions,
            commands::suggestion::get_time_based_suggestions,
            commands::suggestion::get_recent_suggestions,
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
            
            // 🔥 同步开机自启状态
            if let Err(e) = utils::autostart::sync_with_config(config.advanced.start_on_boot) {
                tracing::warn!("Failed to sync autostart with config: {}", e);
            } else {
                tracing::info!("✓ Autostart synced: {}", config.advanced.start_on_boot);
            }
            
            // 如果启用了 MFT，启动 MFT Service 子进程（需要管理员权限）
            #[cfg(target_os = "windows")]
            let actual_use_mft = {
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
                
                let mut mft_launch_success = false;
                
                if use_mft {
                    tracing::info!("🚀 MFT is enabled in file_search plugin, starting MFT service with admin rights...");
                    
                    let exe_path = std::env::current_exe()
                        .expect("Failed to get current exe path");
                    
                    // 获取当前 UI 进程的 PID
                    let ui_pid = std::process::id();
                    
                    tracing::info!("📂 Current exe path: {:?}", exe_path);
                    tracing::info!("🔢 UI PID: {}", ui_pid);
                    
                    // 🔥 检查可执行文件是否存在
                    if !exe_path.exists() {
                        tracing::error!("❌ Executable not found: {:?}", exe_path);
                        tracing::warn!("  Falling back to BFS mode");
                    } else {
                        // 🔥 使用 Windows ShellExecuteW API 直接请求管理员权限
                        // 这比通过 PowerShell 更可靠
                        use windows::core::HSTRING;
                        use windows::Win32::UI::Shell::ShellExecuteW;
                        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
                        
                        let exe_path_str = exe_path.to_string_lossy().to_string();
                        
                        // 🆕 Debug 模式下添加 --skip-scan 参数
                        let parameters = if cfg!(debug_assertions) {
                            format!("--mft-service --skip-scan --ui-pid {}", ui_pid)
                        } else {
                            format!("--mft-service --ui-pid {}", ui_pid)
                        };
                        
                        tracing::debug!("ShellExecuteW: exe={}, params={}", exe_path_str, parameters);
                        
                        unsafe {
                            let operation = HSTRING::from("runas");  // 请求管理员权限
                            let file = HSTRING::from(exe_path_str.as_str());
                            let params = HSTRING::from(parameters.as_str());
                            
                            let result = ShellExecuteW(
                                None,                // hwnd
                                &operation,          // "runas" = 请求管理员权限
                                &file,               // 可执行文件路径
                                &params,             // 参数
                                None,                // 工作目录
                                SW_HIDE,             // 隐藏窗口
                            );
                            
                            // ShellExecuteW 返回值 > 32 表示成功
                            if result.0 as isize > 32 {
                                tracing::info!("✓ MFT service launch requested with admin elevation via ShellExecuteW");
                                tracing::info!("  UI PID: {}, Service will auto-exit when UI closes", ui_pid);
                                tracing::info!("  User will see UAC prompt if not running as admin");
                                mft_launch_success = true;
                            } else {
                                tracing::error!("❌ ShellExecuteW failed with code: {:?}", result.0 as isize);
                                tracing::warn!("  Falling back to BFS mode");
                            }
                        }
                    }
                } else {
                    tracing::info!("⚡ MFT is disabled in file_search plugin, will use BFS scanning mode");
                }
                
                // 🔥 返回实际是否使用 MFT (只有配置启用且启动成功才返回 true)
                use_mft && mft_launch_success
            };
            
            #[cfg(not(target_os = "windows"))]
            let actual_use_mft = false;
            
            // 初始化统计管理器
            let statistics_manager = statistics::StatisticsManager::new()
                .expect("Failed to create statistics manager");
            app.manage(statistics_manager);
            
            // 初始化搜索历史管理器
            let data_dir = utils::paths::get_data_dir()
                .expect("Failed to get data directory");
            let history_path = data_dir.join("search_history.json");
            let search_history = search_history::SearchHistoryManager::new(
                history_path.to_string_lossy().to_string()
            );
            app.manage(search_history);
            
            // 初始化剪贴板管理器
            let clipboard_manager = clipboard::ClipboardManager::new()
                .expect("Failed to create clipboard manager");
            
            // 启动剪贴板监听
            let app_handle_for_clipboard = app.handle().clone();
            clipboard_manager.start_monitoring(app_handle_for_clipboard);
            
            app.manage(clipboard_manager);
            
            // 初始化插件管理器（阻塞等待异步初始化）
            // 🔥 传入实际的 MFT 状态（启动失败则强制为 false）
            let plugin_manager = tauri::async_runtime::block_on(async {
                plugin::PluginManager::new_with_mft_override(Some(actual_use_mft)).await
            });
            app.manage(plugin_manager);
            
            // 🔥 Phase 3: 初始化插件市场状态
            let plugins_dir = storage::get_plugins_dir().expect("Failed to get plugins directory");
            let cache_dir = storage::get_cache_dir().expect("Failed to get cache directory");
            let plugin_market_state = Arc::new(tokio::sync::RwLock::new(
                commands::plugin_market::PluginMarketState::new(
                    plugins_dir,
                    cache_dir.join("plugins")
                )
            ));
            // 🔥 加载已安装插件
            tauri::async_runtime::block_on(async {
                if let Err(e) = plugin_market_state.read().await.registry.load_installed_plugins().await {
                    tracing::warn!("Failed to load installed plugins: {}", e);
                }
            });
            app.manage(plugin_market_state);
            
            // 🔥 Phase 4: 初始化工作流引擎
            let workflows_dir = storage::get_cache_dir()
                .expect("Failed to get cache directory")
                .join("workflows");
            let workflow_engine = Arc::new(tokio::sync::RwLock::new(
                plugin::workflow_engine::WorkflowEngine::new(workflows_dir)
            ));
            // 加载已有工作流
            tauri::async_runtime::block_on(async {
                if let Err(e) = workflow_engine.read().await.load_workflows().await {
                    tracing::warn!("Failed to load workflows: {}", e);
                }
            });
            app.manage(workflow_engine);
            
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
            
            // 🔥 创建系统托盘图标和菜单
            setup_tray_icon(app)?;
            
            // 🔥 移除预渲染逻辑，避免启动时窗口闪现
            // WebView 会在首次调用 show_app 时自动加载
            // 配置中的 "visible": false 确保窗口启动时完全隐藏
            
            tracing::info!("iLauncher setup completed");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 🔹 运行 MFT Service（全量扫描 + 实时监控）
#[cfg(target_os = "windows")]
pub fn run_mft_service(args: &[String]) {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use std::thread;
    use tracing::{info, error, warn};
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
    use tracing_appender::rolling;
    
    // 🔥 初始化文件日志（写入 AppData\Local\iLauncher\logs\mft_service.log）
    let log_dir = match crate::utils::paths::get_log_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to create log directory: {}", e);
            std::process::exit(1);
        }
    };
    
    let file_appender = rolling::never(&log_dir, "mft_service.log");
    
    // 初始化日志（同时输出到文件）
    let local_timer = OffsetTime::local_rfc_3339().expect("Failed to get local offset");
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ilauncher=info,mft=info".into()),
        )
        .with(fmt::layer()
            .with_writer(file_appender)
            .with_ansi(false)
            .with_timer(local_timer))
        .init();
    
    info!("╔════════════════════════════════════════════════════════════╗");
    info!("║          MFT Service Starting                              ║");
    info!("╚════════════════════════════════════════════════════════════╝");
    info!("🚀 MFT Service starting...");
    info!("📅 {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    info!("📝 Log file: {:?}", log_dir.join("mft_service.log"));
    
    // 解析命令行参数（简单解析，不使用 clap）
    let mut output_dir: Option<String> = None;
    let mut drives_str: Option<String> = None;
    let mut scan_only = false;
    let mut ui_pid: Option<u32> = None;
    let mut skip_scan = false;  // 🆕 debug 模式：跳过扫描，直接使用已有索引
    
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
            "--skip-scan" => {
                skip_scan = true;
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
    
    // 🔥 获取当前 MFT Service 进程 PID
    let process_id = std::process::id();
    info!("✓ MFT Service PID: {}", process_id);
    
    // 🔥 清理旧的 .ready 标记文件
    for drive in &drives {
        let ready_file = format!("{}\\{}.ready", output_dir, drive);
        if std::path::Path::new(&ready_file).exists() {
            if let Err(e) = std::fs::remove_file(&ready_file) {
                warn!("Failed to remove old ready file {}: {}", ready_file, e);
            } else {
                info!("✓ Cleaned up old ready file: {}.ready", drive);
            }
        }
    }
    
    // 启动 UI 进程监控线程
    let running = Arc::new(AtomicBool::new(true));
    if let Some(pid) = ui_pid {
        info!("🔍 UI process PID: {}, will auto-exit when UI closes", pid);
        
        let running_for_monitor = running.clone();
        std::thread::spawn(move || {
            monitor_ui_process(pid, running_for_monitor);
        });
    } else {
        warn!("⚠️  No UI PID provided, service will run until manually stopped");
    }
    
    // ============ 阶段 1: 全量扫描 (使用新的 prompt.txt 方案) ============
    let scanned_drives = if skip_scan {
        info!("");
        info!("╔═══════════════════════════════════════════╗");
        info!("║    Phase 1: Skipping Scan (--skip-scan)  ║");
        info!("║    Using Existing Index Files             ║");
        info!("╚═══════════════════════════════════════════╝");
        info!("");
        info!("⏭️  Skipping MFT scan, using existing index files...");
        
        // 检查哪些驱动器有有效的索引文件
        let mut existing_drives = Vec::new();
        for drive in &drives {
            let fst_file = format!("{}\\{}_index.fst", output_dir, drive);
            let dat_file = format!("{}\\{}_bitmaps.dat", output_dir, drive);
            let paths_file = format!("{}\\{}_paths.dat", output_dir, drive);
            
            if std::path::Path::new(&fst_file).exists() 
                && std::path::Path::new(&dat_file).exists()
                && std::path::Path::new(&paths_file).exists() {
                info!("✓ Drive {}: Found existing index files", drive);
                existing_drives.push(*drive);
                
                // 创建 .ready 标记文件
                let ready_file = format!("{}\\{}.ready", output_dir, drive);
                if let Err(e) = std::fs::write(&ready_file, format!("{}", process_id)) {
                    warn!("Failed to create ready file {}: {}", ready_file, e);
                } else {
                    info!("✓ Created ready marker: {}.ready", drive);
                }
            } else {
                warn!("⚠️  Drive {}: Missing index files, skipping", drive);
            }
        }
        
        if existing_drives.is_empty() {
            error!("❌ No valid index files found! Please run without --skip-scan first.");
            std::process::exit(1);
        }
        
        info!("✅ Using existing indexes for drives: {:?}", existing_drives);
        existing_drives
    } else {
        info!("");
        info!("╔═══════════════════════════════════════════╗");
        info!("║    Phase 1: Full Disk Scan                ║");
        info!("║    (StreamingBuilder + 3-gram Index)      ║");
        info!("╚═══════════════════════════════════════════╝");
        info!("");
        
        let _scan_start = std::time::Instant::now();
        
        // 🔥 使用新的 MultiDriveScanner（基于 prompt.txt）
        let mut scan_config = config.clone();
        scan_config.drives = drives.clone();
        scan_config.output_dir = output_dir.clone();
        
        let scanner = mft_scanner::MultiDriveScanner::new(&scan_config);
        
        match scanner.scan_all() {
            Ok(_) => {
                info!("✅ All drives scanned successfully");
            }
            Err(e) => {
                error!("❌ Scan failed: {:#}", e);
                std::process::exit(1);
            }
        }
        
        drives.clone()
    };
    
    info!("");
    info!("╔═══════════════════════════════════════════╗");
    info!("║    Scan Phase Complete                    ║");
    info!("╚═══════════════════════════════════════════╝");
    info!("✓ Successfully scanned drives: {:?}", scanned_drives);
    info!("");
    
    // 🔥 为每个成功扫描的驱动器创建 .ready 标记文件（如果还没创建的话）
    if !skip_scan {
        for drive in &scanned_drives {
            let ready_file = format!("{}\\{}.ready", output_dir, drive);
            if let Err(e) = std::fs::write(&ready_file, format!("{}", process_id)) {
                error!("❌ Failed to create ready file {}: {}", ready_file, e);
            } else {
                info!("✓ Created ready file: {}.ready (PID: {})", drive, process_id);
            }

            // 🔥 全量重建完成后递增主索引版本号：
            // 同会话内 UI 进程若持有旧索引 mmap（如 Service 崩溃后重启重建），
            // 会通过 needs_reload() 检测到此变化并异步重载为新索引
            if let Err(e) = mft_scanner::DeltaMerger::bump_index_version(*drive, &output_dir) {
                warn!("⚠️  Failed to bump index version for drive {}: {:#}", drive, e);
            }
        }
    }
    
    // 如果只需要扫描，则退出
    if scan_only {
        info!("🏁 Scan-only mode, exiting...");
        std::process::exit(0);
    }
    
    // ============ 阶段 2: 实时监控 (使用 USN Incremental Updater) ============
    info!("╔═══════════════════════════════════════════╗");
    info!("║    Phase 2: Real-time Monitoring          ║");
    info!("║    (USN Journal + RoaringBitmap Updates)  ║");
    info!("╚═══════════════════════════════════════════╝");
    info!("");
    
    // 为每个成功扫描的驱动器启动监控线程
    let r = running.clone();
    
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
            let running_clone = running.clone();
            
            // 🔥 启动后台合并任务（每个驱动器独立）
            mft_scanner::DeltaMerger::start_background_merge(drive, output_dir_clone.clone());
            
            std::thread::spawn(move || {
                info!("👀 Starting USN incremental updater for drive {}:", drive);
                
                // 🔥 使用新的 UsnIncrementalUpdater（基于 prompt.txt）
                let mut updater = mft_scanner::UsnIncrementalUpdater::new(drive, output_dir_clone.clone());
                
                // 初始化 USN 位置
                if let Err(e) = updater.initialize() {
                    error!("❌ Failed to initialize USN updater for drive {}: {:#}", drive, e);
                    return;
                }
                
                // 阻塞式监控，直到收到停止信号
                if let Err(e) = updater.start_monitoring(running_clone) {
                    error!("❌ USN monitoring error on drive {}: {:#}", drive, e);
                } else {
                    info!("✓ USN updater for drive {} stopped gracefully", drive);
                }
            })
        })
        .collect();
    
    info!("✓ All monitors started");
    info!("💡 Press Ctrl+C to stop monitoring and exit");
    info!("");
    
    // 🔥 主线程等待停止信号（而不是等待监控线程）
    // 这样可以确保更快地响应退出信号
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(500));
    }
    
    info!("");
    info!("🛑 Shutdown signal received, waiting for monitors to stop...");
    
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
    use tracing::{info, debug};
    
    info!("🔍 Starting UI process monitor thread (PID: {})", ui_pid);
    
    let mut check_count = 0;
    loop {
        check_count += 1;
        
        // 检查进程是否还存在
        let process_exists = check_process_exists(ui_pid);
        
        // 每10秒输出一次心跳日志
        if check_count % 10 == 0 {
            debug!("💓 UI process monitor heartbeat: PID {} exists = {}", ui_pid, process_exists);
        }
        
        if !process_exists {
            info!("⚠️  UI process (PID: {}) has exited, shutting down MFT Service...", ui_pid);
            
            // 🔥 立即设置停止标志
            running.store(false, Ordering::SeqCst);
            
            // 🔥 等待监控线程清理（减少到 2 秒）
            info!("⏳ Waiting 2 seconds for monitors to clean up...");
            thread::sleep(Duration::from_secs(2));
            
            info!("👋 MFT Service exiting due to UI process termination");
            
            // 🔥 强制终止整个进程
            info!("💀 Force terminating process...");
            #[cfg(target_os = "windows")]
            unsafe {
                // Windows: 直接调用 TerminateProcess 终止自己
                use windows::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
                let _ = TerminateProcess(GetCurrentProcess(), 0);
            }
            
            // 如果上面的调用失败，使用标准退出
            std::process::exit(0);
        }
        
        // 每秒检查一次
        thread::sleep(Duration::from_secs(1));
    }
}

/// 检查 Windows 进程是否存在
#[cfg(target_os = "windows")]
fn check_process_exists(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::Threading::{OpenProcess, GetExitCodeProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use tracing::debug;
    
    unsafe {
        // 尝试打开进程句柄（使用更低权限的查询）
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        
        match handle {
            Ok(h) => {
                if h.is_invalid() {
                    debug!("❌ PID {} handle is invalid", pid);
                    return false;
                }
                
                // 检查进程退出码
                let mut exit_code: u32 = 0;
                match GetExitCodeProcess(h, &mut exit_code) {
                    Ok(_) => {
                        let _ = CloseHandle(h);
                        // STILL_ACTIVE (259) 表示进程仍在运行
                        let is_running = exit_code == STILL_ACTIVE.0 as u32;
                        if !is_running {
                            debug!("✓ PID {} has exited with code {}", pid, exit_code);
                        }
                        is_running
                    }
                    Err(e) => {
                        let _ = CloseHandle(h);
                        debug!("❌ Failed to get exit code for PID {}: {:?}", pid, e);
                        false
                    }
                }
            }
            Err(e) => {
                // 无法打开说明进程不存在或无权限访问
                debug!("❌ Failed to open PID {}: {:?}", pid, e);
                false
            }
        }
    }
}

/// 设置系统托盘图标和菜单
fn setup_tray_icon(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::{
        menu::{Menu, MenuItem},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        Manager,
        Emitter,
    };
    
    tracing::info!("🎨 Setting up system tray icon...");
    
    // 创建托盘菜单
    let show_i = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &settings_i, &quit_i])?;
    
    // 创建托盘图标
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)  // 左键点击不显示菜单
        .tooltip("iLauncher")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "show" => {
                    tracing::info!("📋 Tray menu: Show window");
                    if let Some(webview_window) = app.get_webview_window("main") {
                        let window: tauri::Window = webview_window.as_ref().window();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = commands::show_app(window).await {
                                tracing::error!("Failed to show app from tray: {}", e);
                            }
                        });
                    }
                }
                "settings" => {
                    tracing::info!("⚙️  Tray menu: Open settings");
                    if let Some(webview_window) = app.get_webview_window("main") {
                        let window: tauri::Window = webview_window.as_ref().window();
                        tauri::async_runtime::spawn(async move {
                            // 显示窗口
                            if let Err(e) = commands::show_app(window.clone()).await {
                                tracing::error!("Failed to show app from tray: {}", e);
                            }
                            // TODO: 发送事件到前端打开设置页面
                            // 可以通过 window.emit("open-settings", ()) 实现
                            if let Err(e) = window.emit("open-settings", ()) {
                                tracing::error!("Failed to emit open-settings event: {}", e);
                            }
                        });
                    }
                }
                "quit" => {
                    tracing::info!("👋 Tray menu: Quit application");
                    // 优雅退出：先隐藏窗口，然后退出
                    if let Some(webview_window) = app.get_webview_window("main") {
                        let _ = webview_window.hide();
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    std::process::exit(0);
                }
                _ => {
                    tracing::debug!("Unhandled menu event: {:?}", event.id);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    tracing::info!("🖱️  Tray icon left clicked");
                    let app = tray.app_handle();
                    if let Some(webview_window) = app.get_webview_window("main") {
                        let window: tauri::Window = webview_window.as_ref().window();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = commands::toggle_app(window).await {
                                tracing::error!("Failed to toggle app from tray click: {}", e);
                            }
                        });
                    }
                }
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    tracing::info!("🖱️  Tray icon double clicked");
                    let app = tray.app_handle();
                    if let Some(webview_window) = app.get_webview_window("main") {
                        let window: tauri::Window = webview_window.as_ref().window();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = commands::show_app(window).await {
                                tracing::error!("Failed to show app from tray double click: {}", e);
                            }
                        });
                    }
                }
                _ => {}
            }
        })
        .build(app)?;
    
    tracing::info!("✓ System tray icon created successfully");
    Ok(())
}

