// V3 索引端到端验证程序（真实 NTFS 卷）
//
// 流程：
//   1. 非管理员启动 → ShellExecuteExW("runas") 自提权重启，带 --elevated 标记
//   2. scan_mft_streaming_v3：真实 MFT 扫描 → {out}\{D}.snapshot（含 USN 水位）
//   3. 在 C:\Users\Public 下制造真实文件系统变更（新建/改名/删除/级联目录）
//   4. LiveIndex::open + catch_up_volume：从水位 replay journal
//   5. 校验搜索结果（改名跟随、删除消失、级联删除）
//   6. compact → 重开快照 → 校验水位持久化 + 结果一致
//   7. 输出 PASS/FAIL 摘要（exit code 0/1），日志同时写 stdout 和 e2e.log
//
// 用法: v3_e2e [DRIVE] [--elevated]     默认 DRIVE=C

use std::fmt::Arguments;
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ilauncher_lib::index_v2::{CatchUpOutcome, LiveIndex};
use ilauncher_lib::mft_scanner::StreamingBuilder;

static LOG_PATH: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);
static CHECKS: AtomicUsize = AtomicUsize::new(0);
static FAILURES: AtomicUsize = AtomicUsize::new(0);

fn log(args: Arguments) {
    let line = format!("{}\n", args);
    print!("{}", line);
    let _ = std::io::stdout().flush();
    if let Some(p) = LOG_PATH.lock().ok().and_then(|g| g.clone()) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

macro_rules! log {
    ($($arg:tt)*) => { log(format_args!($($arg)*)) };
}

macro_rules! check {
    ($cond:expr, $($msg:tt)*) => {{
        CHECKS.fetch_add(1, Ordering::SeqCst);
        if $cond {
            log(format_args!("  ✓ {}", format!($($msg)*)));
        } else {
            FAILURES.fetch_add(1, Ordering::SeqCst);
            log(format_args!("  ✗ FAIL: {}", format!($($msg)*)));
        }
    }};
}

// ── 提权 ────────────────────────────────────────────────────────────────────

fn is_elevated() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        let result = GetTokenInformation(
            token,
            windows::Win32::Security::TokenElevation,
            Some(&mut elevation as *mut _ as *mut std::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        let elevated = result.is_ok() && elevation.TokenIsElevated != 0;
        let _: windows::core::Result<()> = CloseHandle(token);
        elevated
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// 以管理员重启自身（UAC），等待子进程退出并透传退出码
fn relaunch_elevated(drive: char) -> i32 {
    use windows::Win32::System::Threading::{WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let params = format!("{} --elevated", drive);
    let verb = to_wide("runas");
    let file = to_wide(&exe);
    let par = to_wide(&params);
    let dir = to_wide("");

    let mut sei = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: windows::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS,
        hwnd: windows::Win32::Foundation::HWND::default(),
        lpVerb: windows::core::PCWSTR(verb.as_ptr()),
        lpFile: windows::core::PCWSTR(file.as_ptr()),
        lpParameters: windows::core::PCWSTR(par.as_ptr()),
        lpDirectory: windows::core::PCWSTR(dir.as_ptr()),
        nShow: SW_SHOWNORMAL.0 as i32,
        ..Default::default()
    };

    unsafe {
        if let Err(e) = ShellExecuteExW(&mut sei) {
            log!("❌ 提权失败（UAC 被拒绝或错误）: {:#}", e);
            return 2;
        }
        if !sei.hProcess.is_invalid() {
            let proc = sei.hProcess;
            let _ = WaitForSingleObject(proc, INFINITE);
            let mut code = 0u32;
            let _ = windows::Win32::System::Threading::GetExitCodeProcess(proc, &mut code);
            let _: windows::core::Result<()> = windows::Win32::Foundation::CloseHandle(proc);
            code as i32
        } else {
            0
        }
    }
}

// ── 测试主流程 ──────────────────────────────────────────────────────────────

fn run(drive: char) -> i32 {
    let out_dir = std::env::temp_dir().join(format!("ilauncher_v3_e2e_{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).expect("create output dir");
    *LOG_PATH.lock().unwrap() = Some(out_dir.join("e2e.log"));

    log!("═══════════════════════════════════════════════");
    log!("  iLauncher v3 索引端到端验证（drive {}）", drive);
    log!("═══════════════════════════════════════════════");

    // ── 1. 真实 MFT 扫描 → v3 快照 ────────────────────────────────────────
    log!("\n[1/5] 全量扫描（streaming_builder v3 路径）...");
    let scan_start = std::time::Instant::now();
    let snapshot_path = match (|| -> anyhow::Result<PathBuf> {
        let mut builder = StreamingBuilder::new(drive, &out_dir.to_string_lossy())?;
        builder.scan_mft_streaming_v3(&out_dir.to_string_lossy())
    })() {
        Ok(p) => p,
        Err(e) => {
            log!("❌ 扫描失败: {:#}", e);
            return 1;
        }
    };
    log!("  ✓ 扫描完成 {:?}（{:?}）", snapshot_path, scan_start.elapsed());

    let snap = match LiveIndex::open(&snapshot_path) {
        Ok(idx) => idx,
        Err(e) => {
            log!("❌ 快照打开失败: {:#}", e);
            return 1;
        }
    };
    log!("  ✓ 快照打开：{} 行，水位 journal={:#X} next_usn={}",
        snap.snapshot().row_count(), snap.journal_id(), snap.next_usn());
    check!(snap.snapshot().row_count() > 1000, "真实卷扫描行数应 > 1000（实际 {}）", snap.snapshot().row_count());
    check!(snap.snapshot().is_complete(), "快照应标记完整");
    check!(snap.journal_id() != 0 && snap.next_usn() > 0, "扫描应写入 USN 水位");
    drop(snap);

    // ── 2. 制造真实文件系统变更 ────────────────────────────────────────────
    log!("\n[2/5] 制造文件系统变更（C:\\Users\\Public 下）...");
    let base = std::path::PathBuf::from(r"C:\Users\Public")
        .join(format!("ilauncher_v3_e2e_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("sub\\deep")).unwrap();
    std::fs::write(base.join("hello_v3.txt"), "e2e").unwrap();
    std::fs::write(base.join("sub\\nested_v3.txt"), "e2e").unwrap();
    std::fs::write(base.join("sub\\deep\\leaf_v3.txt"), "e2e").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    // rename 文件
    std::fs::rename(base.join("hello_v3.txt"), base.join("hello_v3_renamed.txt")).unwrap();
    // 新建再删除（应完全消失）
    std::fs::write(base.join("ghost_v3.txt"), "e2e").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    std::fs::remove_file(base.join("ghost_v3.txt")).unwrap();
    // 删除整个 sub 目录（级联：nested_v3 / leaf_v3 应消失）
    std::fs::remove_dir_all(base.join("sub")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    log!("  ✓ 变更完成：新建/改名/瞬时新建删除/级联目录删除");

    // ── 3. catch-up：从水位 replay journal ─────────────────────────────────
    log!("\n[3/5] LiveIndex::catch_up_volume（真实 journal replay）...");
    let mut idx = LiveIndex::open(&snapshot_path).unwrap();
    let entries = match idx.catch_up_volume(drive) {
        Ok(CatchUpOutcome::CaughtUp(entries)) => entries,
        Ok(other) => {
            log!("❌ catch-up 未达预期: {:?}", other);
            return 1;
        }
        Err(e) => {
            log!("❌ catch-up 失败: {:#}", e);
            return 1;
        }
    };
    let caught = entries.len();
    log!("  ✓ replay 了 {} 条 USN 增量", caught);
    check!(caught >= 5, "应 replay ≥5 条变更（实际 {}）", caught);
    check!(idx.overlay().pending_len() > 0, "overlay 应有 pending 变更");

    // 诊断：reason 分布 + 目标条目详情
    {
        use ilauncher_lib::index_v2::usn_journal as uj;
        let mut hist: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
        for e in &entries {
            *hist.entry(e.reason).or_insert(0) += 1;
        }
        log!("  [diag] reason 分布:");
        for (reason, n) in &hist {
            let names: Vec<&str> = [
                (uj::REASON_FILE_CREATE, "CREATE"),
                (uj::REASON_FILE_DELETE, "DELETE"),
                (uj::REASON_RENAME_OLD_NAME, "RENAME_OLD"),
                (uj::REASON_RENAME_NEW_NAME, "RENAME_NEW"),
                (uj::REASON_BASIC_INFO_CHANGE, "BASIC_INFO"),
                (uj::REASON_DATA_OVERWRITE, "DATA_OVERWRITE"),
                (uj::REASON_DATA_EXTEND, "DATA_EXTEND"),
                (uj::REASON_DATA_TRUNCATION, "DATA_TRUNC"),
            ]
            .iter()
            .filter(|(bit, _)| reason & bit != 0)
            .map(|(_, n)| *n)
            .collect();
            log!("    {:#06x} × {:>3}  {}", reason, n, names.join("|"));
        }
    }

    // ── 4. 校验搜索语义 ────────────────────────────────────────────────────
    log!("\n[4/5] 校验搜索结果...");
    let base_str = base.to_string_lossy().replace('/', "\\");
    let hits = idx.search("hello_v3_renamed", 10).unwrap();
    check!(hits.len() == 1, "改名后文件应恰好命中 1 次（实际 {}）", hits.len());
    if !hits.is_empty() {
        check!(
            hits[0].path == base.join("hello_v3_renamed.txt").to_string_lossy().replace('/', "\\"),
            "改名后路径正确: {}", hits[0].path
        );
    }
    check!(
        idx.search("hello_v3.txt", 10).unwrap().iter().all(|h| h.name != "hello_v3.txt"),
        "旧名记录不得复活（模糊子序列命中新名属正常）"
    );
    // 注意：真实卷上模糊搜索会子序列命中无关文件（如 ghost_030_inv.png），
    // 断言应为"测试目录下不得出现"，而非全局为空
    for kw in ["ghost_v3", "nested_v3", "leaf_v3"] {
        let leaked = idx
            .search(kw, 50)
            .unwrap()
            .iter()
            .any(|h| h.path.starts_with(base_str.as_str()));
        check!(!leaked, "已删除的 {} 不得出现在测试目录下", kw);
    }

    // ── 5. compact → 重开 → 一致性 ───────────────────────────────────────
    log!("\n[5/5] compact → 重开快照校验...");
    let water_before = (idx.journal_id(), idx.next_usn());
    let counts_before = idx.counts();
    idx.compact(&snapshot_path).unwrap();
    let idx2 = LiveIndex::open(&snapshot_path).unwrap();
    check!(idx2.journal_id() == water_before.0 && idx2.next_usn() == water_before.1,
        "水位应随 compact 持久化（journal={:#X} next_usn={}）", idx2.journal_id(), idx2.next_usn());
    check!(idx2.overlay().pending_len() == 0, "compact 后 overlay 应清空");
    check!(idx2.counts() == counts_before, "compact 前后计数一致 {:?} vs {:?}", counts_before, idx2.counts());
    let hits2 = idx2.search("hello_v3_renamed", 10).unwrap();
    check!(hits2.len() == 1, "compact 后改名文件仍可搜到");
    if !hits2.is_empty() {
        check!(hits2[0].path == base.join("hello_v3_renamed.txt").to_string_lossy().replace('/', "\\"),
            "compact 后路径一致: {}", hits2[0].path);
    }
    let ghost_after = idx2
        .search("ghost_v3", 50)
        .unwrap()
        .iter()
        .any(|h| h.path.starts_with(base_str.as_str()));
    check!(!ghost_after, "compact 后 ghost_v3 仍不得出现在测试目录下");
    drop(idx2);

    // ── 摘要 ────────────────────────────────────────────────────────────────
    let checks = CHECKS.load(Ordering::SeqCst);
    let failures = FAILURES.load(Ordering::SeqCst);
    log!("\n═══════════════════════════════════════════════");
    log!("  结果: {}/{} 通过, {} 失败", checks - failures, checks, failures);
    log!("  日志: {:?}", out_dir.join("e2e.log"));
    log!("═══════════════════════════════════════════════");

    // 清理测试文件（索引文件保留供人工检查）
    let _ = std::fs::remove_dir_all(&base);

    if failures == 0 { 0 } else { 1 }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let drive = args
        .get(1)
        .and_then(|s| s.chars().next())
        .unwrap_or('C');
    let elevated_flag = args.iter().any(|a| a == "--elevated");

    if !elevated_flag && !is_elevated() {
        println!("🔐 非管理员运行，请求 UAC 提权...");
        std::process::exit(relaunch_elevated(drive));
    }

    std::process::exit(run(drive));
}
