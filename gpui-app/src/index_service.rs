// 索引服务：常驻 MFT 服务（提权子进程）+ UI 侧后台加载线程
//
// 架构（复用 ilauncher-index 的 V3DriveService，不重新实现扫描编排）：
//   ilauncher-gpui --mft-service --ui-pid <pid>   ← ShellExecuteW runas 提权启动，常驻
//     · 单实例互斥锁（Global\iLauncherMftServiceV3），重复启动直接退出
//     · 每盘一个 V3DriveService::run：启动决策（打开/重建）+ 2s 周期 catch-up + compact
//     · UI 进程监控：UI 退出 → 服务退出（最终 compact 由 V3DriveService 保证）
//   UI 进程：
//     · init_index_loader：快照缺失 → 提权拉起服务；等所有盘快照就绪 → 打开进 LiveSet
//     · 已加载的索引放 Arc<RwLock<Vec<LiveIndex>>>，UI 搜索读、后台线程写
//
// 仅 Windows + feature ilauncher 编译。

#[cfg(all(feature = "ilauncher", target_os = "windows"))]
pub mod imp {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use ilauncher_index::index_v2::LiveIndex;
    use ilauncher_index::mft_scanner::types::ScanConfig;
    use ilauncher_index::mft_scanner::v3_service::V3DriveService;

    use windows::core::HSTRING;
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::{
        CreateMutexW, GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    use crate::search::LiveSet;

    const SERVICE_MUTEX_NAME: &str = "Global\\iLauncherMftServiceV3";
    /// 等待服务扫描完成的超时（大容量机械盘全量扫描可能很久）
    const SCAN_WAIT_TIMEOUT: Duration = Duration::from_secs(900);
    const WAIT_POLL_INTERVAL: Duration = Duration::from_secs(1);

    fn snapshot_dir() -> PathBuf {
        ilauncher_index::paths::get_app_data_dir()
            .expect("app data dir")
            .join("mft_databases")
    }

    fn snapshot_path(drive: char) -> PathBuf {
        snapshot_dir().join(format!("{}.snapshot", drive))
    }

    fn detect_drives() -> Vec<char> {
        let drives = ScanConfig::detect_ntfs_drives();
        println!("✓ 检测到 NTFS 驱动器: {:?}", drives);
        drives
    }

    fn check_process_exists(pid: u32) -> bool {
        unsafe {
            let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return false;
            };
            let mut code = 0u32;
            let active = GetExitCodeProcess(handle, &mut code).is_ok() && code == 259; // STILL_ACTIVE
            let _ = CloseHandle(handle);
            active
        }
    }

    // ── 常驻服务模式（提权子进程） ─────────────────────────────────────────

    pub fn run_mft_service(args: &[String]) {
        // 单实例守卫：服务已运行则直接退出（UI 重复拉起无害）
        // CreateMutexW 需要以 NUL 结尾的宽字符串（Win32 拷贝名称，调用后即可释放）
        let name_wide: Vec<u16> = SERVICE_MUTEX_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        let mutex = unsafe { CreateMutexW(None, true, windows::core::PCWSTR(name_wide.as_ptr())) };
        if mutex.is_ok() && unsafe { windows::Win32::Foundation::GetLastError() } == ERROR_ALREADY_EXISTS {
            println!("⚠ MFT 服务已在运行（互斥锁占用），本实例退出");
            return;
        }

        let ui_pid = args
            .windows(2)
            .position(|w| w[0] == "--ui-pid")
            .and_then(|i| args.get(i + 2))
            .and_then(|s| s.parse::<u32>().ok());

        let drives = detect_drives();
        let out_dir = snapshot_dir();
        std::fs::create_dir_all(&out_dir).expect("create snapshot dir");
        let out_dir = out_dir.to_string_lossy().to_string();

        let running = Arc::new(AtomicBool::new(true));
        if let Some(pid) = ui_pid {
            println!("✓ UI 进程监控: PID {}，UI 退出后服务自动停止", pid);
            let flag = running.clone();
            std::thread::spawn(move || {
                while check_process_exists(pid) {
                    std::thread::sleep(Duration::from_secs(1));
                }
                println!("👋 UI 进程 {} 已退出，服务停止中（最终 compact…）", pid);
                flag.store(false, Ordering::SeqCst);
            });
        } else {
            println!("⚠ 未提供 --ui-pid，服务将持续运行直到手动结束");
        }

        // 每盘一个线程跑 V3DriveService（启动决策 + 周期 catch-up + compact）
        let mut handles = Vec::new();
        for drive in drives {
            let out = out_dir.clone();
            let flag = running.clone();
            handles.push(std::thread::spawn(move || {
                let service = V3DriveService::new(drive, out);
                if let Err(e) = service.run(flag) {
                    eprintln!("❌ [v3] 盘 {} 服务线程出错: {:#}", drive, e);
                }
            }));
        }

        for h in handles {
            let _ = h.join();
        }
        println!("✓ MFT 服务已退出");
    }

    /// ShellExecuteW runas 提权拉起服务子进程（与现行 Tauri 版同一手法）
    fn spawn_elevated_service() {
        let exe = std::env::current_exe().expect("current exe");
        let params = format!("--mft-service --ui-pid {}", std::process::id());
        let result = unsafe {
            ShellExecuteW(
                None,
                &HSTRING::from("runas"),
                &HSTRING::from(exe.to_string_lossy().as_ref()),
                &HSTRING::from(&params),
                None,
                SW_HIDE,
            )
        };
        if result.0 as isize > 32 {
            println!("✓ 已请求提权启动 MFT 服务（{}）", params);
        } else {
            eprintln!("⚠ ShellExecuteW 提权失败，错误码 {}（用户可能取消了 UAC）", result.0 as isize);
        }
    }

    // ── UI 侧后台加载 ──────────────────────────────────────────────────────

    /// 启动后台加载线程：快照缺失 → 提权拉起服务 → 等全部就绪 → 打开进 LiveSet。
    /// 返回的 LiveSet 立即交给 UI 使用（初始为空 = "索引加载中"）。
    pub fn init_index_loader() -> LiveSet {
        let set = LiveSet::empty();
        let set_for_thread = set.clone();
        std::thread::spawn(move || {
            let drives = detect_drives();
            if drives.is_empty() {
                eprintln!("⚠ 未检测到 NTFS 驱动器");
                return;
            }

            let missing: Vec<char> = drives
                .iter()
                .copied()
                .filter(|d| !snapshot_path(*d).exists())
                .collect();
            if !missing.is_empty() {
                println!("📦 索引缺失（{:?}），正在请求管理员构建…", missing);
                spawn_elevated_service();
            }

            // 增量加载：已有的盘立即打开（UI 马上可用），缺失的盘等服务构建完再补上
            let deadline = Instant::now() + SCAN_WAIT_TIMEOUT;
            let mut pending: Vec<char> = missing;
            loop {
                // 尝试打开当前所有 pending 盘（存在才打开，允许打开失败下次再试）
                let mut still_pending = Vec::new();
                for drive in pending {
                    let path = snapshot_path(drive);
                    if !path.exists() {
                        still_pending.push(drive);
                        continue;
                    }
                    match open_with_retry(&path) {
                        Some(idx) => {
                            println!("✓ 盘 {} 索引已加载（{} 行）", drive, idx.snapshot().row_count());
                            set_for_thread.push_index(idx);
                        }
                        None => eprintln!("❌ 盘 {} 索引打开失败，该盘不可用", drive),
                    }
                }
                pending = still_pending;
                if pending.is_empty() {
                    break;
                }
                if Instant::now() > deadline {
                    eprintln!("⚠ 等待 {:?} 快照超时，服务可能未获管理员授权", pending);
                    break;
                }
                std::thread::sleep(WAIT_POLL_INTERVAL);
            }

            println!("🎯 索引加载完成：{} 盘可用", set_for_thread.drive_count());
        });
        set
    }

    /// 打开快照（服务写盘刚结束时文件可能短暂占用，重试 3 次）
    fn open_with_retry(path: &std::path::Path) -> Option<LiveIndex> {
        for attempt in 1..=3 {
            match LiveIndex::open(path) {
                Ok(idx) => return Some(idx),
                Err(e) => {
                    eprintln!("⚠ 打开 {} 失败（第 {} 次）: {:#}", path.display(), attempt, e);
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
        }
        None
    }
}
