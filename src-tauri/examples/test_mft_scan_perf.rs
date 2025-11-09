use ilauncher::mft_scanner::{Scanner, Database};
use std::time::Instant;

fn main() {
    println!("\n=== MFT Scan Performance Analysis ===\n");
    
    let drives = vec!['C', 'D', 'E', 'F', 'G', 'H'];
    
    for drive_letter in drives {
        let drive_path = format!("{}:\\", drive_letter);
        
        if !std::path::Path::new(&drive_path).exists() {
            continue;
        }
        
        println!("Drive {}:", drive_letter);
        
        // 1. MFT Scan
        let scan_start = Instant::now();
        let scanner = match Scanner::new(drive_letter) {
            Ok(s) => s,
            Err(e) => {
                println!("   Failed to create scanner: {}", e);
                continue;
            }
        };
        
        let entries = match scanner.scan() {
            Ok(e) => e,
            Err(e) => {
                println!("   Scan failed: {}", e);
                continue;
            }
        };
        let scan_duration = scan_start.elapsed();
        
        println!("   MFT scan time: {:.2?}", scan_duration);
        println!("   Files scanned: {}", entries.len());
        
        // 2. Database write
        let db_path = format!(
            "{}\\iLauncher\\mft_databases\\{}.db",
            std::env::var("LOCALAPPDATA").unwrap(),
            drive_letter
        );
        
        let db_start = Instant::now();
        let mut db = match Database::new(drive_letter) {
            Ok(d) => d,
            Err(e) => {
                println!("   Failed to create database: {}", e);
                continue;
            }
        };
        
        if let Err(e) = db.insert_batch(&entries) {
            println!("   Database write failed: {}", e);
            continue;
        }
        
        if let Err(e) = db.create_indexes() {
            println!("   Index creation failed: {}", e);
        }
        
        let db_duration = db_start.elapsed();
        
        println!("   Database write time: {:.2?}", db_duration);
        
        // 3. Database file size
        if let Ok(metadata) = std::fs::metadata(&db_path) {
            let size_mb = metadata.len() as f64 / 1024.0 / 1024.0;
            println!("   Database size: {:.2} MB", size_mb);
        }
        
        // 4. Total time
        let total_duration = scan_start.elapsed();
        println!("   Total time: {:.2?}", total_duration);
        
        // 5. Performance breakdown
        let scan_percent = (scan_duration.as_secs_f64() / total_duration.as_secs_f64() * 100.0) as u32;
        let db_percent = (db_duration.as_secs_f64() / total_duration.as_secs_f64() * 100.0) as u32;
        
        println!("   MFT scan: {}%", scan_percent);
        println!("   DB write: {}%", db_percent);
        println!();
    }
}
