// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 检查启动参数
    let args: Vec<String> = std::env::args().collect();
    
    // 🔹 MFT Service 模式（全量扫描 + 实时监控）
    if args.contains(&"--mft-service".to_string()) {
        #[cfg(target_os = "windows")]
        {
            ilauncher_lib::run_mft_service(&args);
        }
        #[cfg(not(target_os = "windows"))]
        {
            eprintln!("MFT Service is only available on Windows");
            std::process::exit(1);
        }
        return;
    }
    
    // 🔹 旧版 MFT 扫描器模式（仅为兼容性保留）
    if args.len() > 1 && args[1] == "--mft-scanner" {
        #[cfg(target_os = "windows")]
        {
            println!("⚠️  Warning: --mft-scanner is deprecated, use --mft-service instead");
            ilauncher_lib::run_mft_scanner();
        }
        #[cfg(not(target_os = "windows"))]
        {
            eprintln!("MFT Scanner is only available on Windows");
            std::process::exit(1);
        }
        return;
    }
    
    // 🔹 正常 GUI 模式
    ilauncher_lib::run()
}
