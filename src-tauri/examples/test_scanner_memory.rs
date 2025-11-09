// 测试 Scanner 内存占用
mod mft_scanner;
use mft_scanner::{UsnScanner, ScanConfig};
use std::time::Instant;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("=== Scanner Memory Test ===");
    println!("Press Enter to start scanning D: drive...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    
    let start = Instant::now();
    
    let mut scanner = UsnScanner::new('D');
    let config = ScanConfig::default();
    
    println!("🚀 Starting scan...");
    match scanner.scan_to_database("./test_db", &config) {
        Ok(_) => {
            let duration = start.elapsed();
            println!("✅ Scan completed in {:.2}s", duration.as_secs_f64());
        }
        Err(e) => {
            eprintln!("❌ Scan failed: {}", e);
        }
    }
    
    println!("\nPress Enter to exit (to measure final memory)...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
}
