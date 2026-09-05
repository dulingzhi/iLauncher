// 索引服务：常驻 MFT 服务（提权子进程）+ UI 侧后台加载/重建
//
// 架构（复用 ilauncher-index 的 V3DriveService，不重新实现扫描编排）：
//   ilauncher-gpui --mft-service --ui-pid <pid>   ← ShellExecuteW runas 提权启动，常驻
//     · 单实例互斥锁（Global\iLauncherMftServiceV3），重复启动直接退出
//     · 每盘一个 V3DriveService::run：启动决策（打开/重建）+ 2s 周期 catch-up + compact
//     · UI 进程监控：UI 退出 → 服务退出（最终 compact 由 V3DriveService 保证）
//   ilauncher-gpui --rebuild --ui-pid <pid>      ← 提权一次性全量重扫，完成后转常驻
//     · 取互斥锁失败（服务在跑）→ 报错退出
//     · 删除全部快照 → 逐盘 StreamingBuilder 全量重建 → 进入常驻服务模式
//   UI 进程：
//     · init_index_loader：增量加载（已有盘立即打开，缺失盘服务构建完补上）；
//       快照齐全但服务未运行时也静默拉起服务（保证持续 catch-up）
//     · request_rebuild：清 LiveSet（释放 mmap）→ 写 rebuild.request 哨兵
//       （服务在跑，就地重建）或提权 --rebuild（服务未跑）→
//       轮询快照 mtime 变化 → 重新加载
//
// 仅 Windows + feature ilauncher 编译。

#[cfg(all(feature = "ilauncher", target_os = "windows"))]
pub mod imp {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime};

    use ilauncher_index::index_v2::LiveIndex;
    use ilauncher_index::mft_scanner::types::ScanConfig;
    use ilauncher_index::mft_scanner::v3_service::V3DriveService;

    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::{
        CreateMutexW, GetExitCodeProcess, OpenMutexW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        SYNCHRONIZATION_SYNCHRONIZE,
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

    /// 就地重建哨兵路径：UI 写、服务读（服务持有快照文件，UI 无法独占重建，
    /// 运行时重建改由服务在 catch-up 循环里发现哨兵后就地执行）
    fn rebuild_request_path() -> PathBuf {
        snapshot_dir().join("rebuild.request")
    }

    /// 服务互斥锁名（NUL 结尾宽字符串，调用期间有效）
    fn mutex_name_wide() -> Vec<u16> {
        SERVICE_MUTEX_NAME.encode_utf16().chain(std::iter::once(0)).collect()
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

    /// MFT 服务是否在运行（探测单实例互斥锁，无需管理员权限）
    pub fn service_running() -> bool {
        let name = mutex_name_wide();
        unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(name.as_ptr())).is_ok() }
    }

    /// ShellExecuteW runas 提权启动本程序（带自定义参数；与旧版同一手法）
    fn spawn_elevated(args: &str) {
        let exe = std::env::current_exe().expect("current exe");
        let result = unsafe {
            ShellExecuteW(
                None,
                &HSTRING::from("runas"),
                &HSTRING::from(exe.to_string_lossy().as_ref()),
                &HSTRING::from(args),
                None,
                SW_HIDE,
            )
        };
        if result.0 as isize > 32 {
            println!("✓ 已请求提权启动: {}", args);
        } else {
            eprintln!("⚠ ShellExecuteW 提权失败，错误码 {}（用户可能取消了 UAC）", result.0 as isize);
        }
    }

    fn spawn_elevated_service(ui_pid: u32) {
        spawn_elevated(&format!("--mft-service --ui-pid {}", ui_pid));
    }

    // ── 常驻服务模式（提权子进程） ─────────────────────────────────────────

    pub fn run_mft_service(args: &[String]) {
        service_main(args, false);
    }

    /// --rebuild：取互斥锁 → 删全部快照 → 全量重扫 → 转常驻服务
    pub fn run_rebuild_service(args: &[String]) {
        // 服务在跑时拒绝重建（否则两边抢文件）；用户应先让服务退出
        if service_running() {
            eprintln!("❌ MFT 服务正在运行，无法重建索引。请先退出服务（关闭主程序即可）。");
            std::process::exit(1);
        }
        for drive in detect_drives() {
            let path = snapshot_path(drive);
            if path.exists() {
                if let Err(e) = std::fs::remove_file(&path) {
                    eprintln!("❌ 删除 {} 失败: {:#}", path.display(), e);
                    std::process::exit(1);
                }
                println!("🗑 已删除旧快照 {}", path.display());
            }
        }
        service_main(args, true);
    }

    fn service_main(args: &[String], just_rebuilt: bool) {
        // 单实例守卫：服务已运行则直接退出（UI 重复拉起无害）
        let name = mutex_name_wide();
        let mutex = unsafe { CreateMutexW(None, true, PCWSTR(name.as_ptr())) };
        if mutex.is_ok() && unsafe { windows::Win32::Foundation::GetLastError() } == ERROR_ALREADY_EXISTS {
            println!("⚠ MFT 服务已在运行（互斥锁占用），本实例退出");
            return;
        }

        // args[i] == "--ui-pid"，其值在 i+1（原实现误取 i+2 导致永远解析失败、服务永不退出）
        let ui_pid = args
            .windows(2)
            .position(|w| w[0] == "--ui-pid")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<u32>().ok());

        let drives = detect_drives();
        let out_dir = snapshot_dir();
        std::fs::create_dir_all(&out_dir).expect("create snapshot dir");
        let out_dir = out_dir.to_string_lossy().to_string();

        if just_rebuilt {
            println!("🔨 重建模式：全部快照已删除，开始全量扫描");
        }

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

        // 每盘一个线程跑 V3DriveService（启动决策 + 周期 catch-up + 就地重建哨兵 + compact）
        let watch_path = rebuild_request_path();
        let mut handles = Vec::new();
        for drive in drives {
            let out = out_dir.clone();
            let flag = running.clone();
            let watch = ilauncher_index::mft_scanner::v3_service::RebuildWatch::new(watch_path.clone());
            handles.push(std::thread::spawn(move || {
                let mut service =
                    V3DriveService::new(drive, out).with_rebuild_watch(watch);
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

    // ── UI 侧后台加载 ──────────────────────────────────────────────────────

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

    /// 打开当前所有盘的快照进 set（增量追加已加载的盘会重复，调用方需先 clear）
    fn load_all(set: &LiveSet) {
        for drive in detect_drives() {
            let path = snapshot_path(drive);
            if !path.exists() {
                continue;
            }
            match open_with_retry(&path) {
                Some(idx) => {
                    println!("✓ 盘 {} 索引已加载（{} 行）", drive, idx.snapshot().row_count());
                    set.push_index(idx);
                }
                None => eprintln!("❌ 盘 {} 索引打开失败，该盘不可用", drive),
            }
        }
        println!("🎯 索引加载完成：{} 盘可用", set.drive_count());
    }

    /// 启动后台加载线程，返回立即可用的 LiveSet（初始为空 = "索引加载中"）。
    /// 快照齐全但服务未运行时也静默拉起服务——修复"重启 UI 后无人做 catch-up"的边界。
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
                spawn_elevated_service(std::process::id());
            } else if !service_running() {
                println!("🔄 索引齐全但服务未运行，静默拉起服务（持续 catch-up）");
                spawn_elevated_service(std::process::id());
            }

            // 增量加载：现成快照立即打开（UI 马上可用），缺失盘等服务构建完再补上
            let deadline = Instant::now() + SCAN_WAIT_TIMEOUT;
            let mut pending: Vec<char> = missing;
            load_all(&set_for_thread);
            // 再等缺失盘出现并补上
            loop {
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
        });
        set
    }

    /// 托盘/设置页"重建索引"：
    ///   服务在跑（常态）→ 写哨兵文件，服务 catch-up 循环发现后就地全量重建
    ///   服务未跑（边界：启动时 UAC 被拒/服务崩溃）→ 提权 --rebuild 起新服务重建
    /// 两条路都以「全部盘快照 mtime 变化」为完成信号，随后重新加载进 LiveSet
    pub fn request_rebuild(set: &LiveSet) {
        // 记录当前快照 mtime 作为变化基准（None = 文件不存在）
        let marks: Vec<(char, Option<SystemTime>)> = detect_drives()
            .into_iter()
            .map(|d| (d, snapshot_path(d).exists().then(|| snapshot_path(d).metadata().unwrap().modified().unwrap())))
            .collect();
        if marks.is_empty() {
            eprintln!("⚠ 未检测到驱动器，跳过重建");
            return;
        }

        set.clear();

        let request_path = rebuild_request_path();
        if service_running() {
            // 先删再写，保证重复点击（哪怕同一毫秒内）mtime 也严格更新
            let _ = std::fs::remove_file(&request_path);
            match std::fs::write(&request_path, format!("rebuild at {:?}\n", std::time::SystemTime::now())) {
                Ok(()) => println!("✓ 已通知索引服务就地重建，等待完成…"),
                Err(e) => {
                    eprintln!("❌ 写入重建哨兵失败: {:#}", e);
                    return;
                }
            }
        } else {
            println!("🗑 已释放内存索引，请求提权重建…");
            spawn_elevated(&format!("--rebuild --ui-pid {}", std::process::id()));
        }

        let set = set.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + SCAN_WAIT_TIMEOUT;
            loop {
                let all_changed = marks.iter().all(|(d, mark)| {
                    let path = snapshot_path(*d);
                    let current = path
                        .exists()
                        .then(|| path.metadata().and_then(|m| m.modified()).ok())
                        .flatten();
                    current.is_some() && current.as_ref() != mark.as_ref()
                });
                if all_changed {
                    break;
                }
                if Instant::now() > deadline {
                    eprintln!("⚠ 等待重建完成超时");
                    return;
                }
                std::thread::sleep(WAIT_POLL_INTERVAL);
            }
            println!("✓ 重建完成，重新加载索引…");
            // 服务端已消费（水位推进），清理哨兵
            let _ = std::fs::remove_file(&request_path);
            load_all(&set);
        });
    }
}
