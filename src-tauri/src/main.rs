// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 检查启动参数
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 && args[1] == "--mft-scanner" {
        // MFT 扫描器模式（需要管理员权限）
        #[cfg(target_os = "windows")]
        {
            println!("🔧 Starting in MFT Scanner mode...");
            ilauncher_lib::run_mft_scanner();
        }
        #[cfg(not(target_os = "windows"))]
        {
            eprintln!("MFT Scanner is only available on Windows");
            std::process::exit(1);
        }
    } else {
        // 正常 GUI 模式
        ilauncher_lib::run()
    }
}
