// MFT 扫描性能分析工具 - 真实代码测试
// 运行: cargo run --release --example profile_mft_scan

use anyhow::Result;
use std::time::Instant;
use ilauncher_lib::mft_scanner::{UsnScanner, ScanConfig};


fn format_duration(ms: u128) -> String {
    if ms < 1000 {
        format!("{} ms", ms)
    } else {
        format!("{:.2} s", ms as f64 / 1000.0)
    }
}

fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("\n🔬 MFT 扫描性能分析工具 (真实代码)");
    println!("═══════════════════════════════════════════════════════════\n");

    // 检查管理员权限
    if !UsnScanner::check_admin_rights() {
        eprintln!("❌ 需要管理员权限运行此工具");
        eprintln!("   请以管理员身份运行 PowerShell 后重试");
        return Ok(());
    }

    // 配置
    let drives = vec!['C', 'D', 'E'];
    let output_dir = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| ".".to_string())
        + "\\iLauncher\\mft_databases";
    let config = ScanConfig::default();

    println!("📋 测试配置:");
    println!("   - 扫描驱动器: {:?}", drives);
    println!("   - 输出目录: {}", output_dir);
    println!("   - 批量大小: 100,000\n");

    // ═══════════════════════════════════════════════════════════
    // 阶段 1: 完整扫描性能分析 (单驱动器)
    // ═══════════════════════════════════════════════════════════
    
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ 阶段 1: 完整扫描性能分析 (驱动器 {})                   │", drives[0]);
    println!("└─────────────────────────────────────────────────────────┘\n");

    let drive = drives[0];
    let mut scanner = UsnScanner::new(drive);

    // 测量总时间
    let total_start = Instant::now();
    
    println!("⏱️  开始扫描...");
    
    // 执行完整扫描
    match scanner.scan_to_database(&output_dir, &config) {
        Ok(_) => {
            let total_time = total_start.elapsed().as_millis();
            
            println!("\n✅ 扫描完成!");
            println!("\n📊 性能总结:");
            println!("   总耗时: {}", format_duration(total_time));
            
            // 读取数据库统计
            use ilauncher_lib::mft_scanner::Database;
            if let Ok(db) = Database::open(drive, &output_dir) {
                match db.search("", 1) {
                    Ok(results) => {
                        println!("   数据库已创建");
                    }
                    Err(e) => {
                        println!("   数据库状态: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("❌ 扫描失败: {}", e);
        }
    }

    println!("\n═══════════════════════════════════════════════════════════");
    println!("💡 性能优化建议:");
    println!("   1. 检查日志中的详细阶段耗时");
    println!("   2. 如果 'Building FRN map' 慢 → 优化 HashMap 插入");
    println!("   3. 如果 'Rebuilding paths' 慢 → 优化路径拼接");
    println!("   4. 如果 'insert_batch' 慢 → 增大批量大小或优化 SQL");
    println!("═══════════════════════════════════════════════════════════\n");

    // ═══════════════════════════════════════════════════════════
    // 阶段 2: 并行扫描性能分析
    // ═══════════════════════════════════════════════════════════
    
    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ 阶段 2: 并行扫描性能分析 (所有驱动器)                 │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    let parallel_start = Instant::now();
    
    let handles: Vec<_> = drives
        .iter()
        .map(|&drive| {
            let output_dir_clone = output_dir.clone();
            let config_clone = config.clone();
            
            std::thread::spawn(move || {
                println!("🚀 开始扫描驱动器 {}:", drive);
                let start = Instant::now();
                
                let mut scanner = UsnScanner::new(drive);
                let result = scanner.scan_to_database(&output_dir_clone, &config_clone);
                
                let elapsed = start.elapsed().as_millis();
                
                match result {
                    Ok(_) => {
                        println!("✅ 驱动器 {} 完成: {}", drive, format_duration(elapsed));
                        Ok((drive, elapsed))
                    }
                    Err(e) => {
                        eprintln!("❌ 驱动器 {} 失败: {}", drive, e);
                        Err(e)
                    }
                }
            })
        })
        .collect();

    // 等待所有线程完成
    let mut results = Vec::new();
    for handle in handles {
        if let Ok(Ok((drive, time))) = handle.join() {
            results.push((drive, time));
        }
    }

    let parallel_total = parallel_start.elapsed().as_millis();

    println!("\n� 并行扫描结果:");
    println!("┌──────────┬────────────┐");
    println!("│ 驱动器   │ 耗时       │");
    println!("├──────────┼────────────┤");
    for (drive, time) in &results {
        println!("│ {}:       │ {:>10} │", drive, format_duration(*time));
    }
    println!("└──────────┴────────────┘");
    
    println!("\n⏱️  并行总耗时: {}", format_duration(parallel_total));
    
    if !results.is_empty() {
        let serial_time: u128 = results.iter().map(|(_, t)| t).sum();
        println!("📈 串行总耗时: {} (预估)", format_duration(serial_time));
        
        if parallel_total > 0 {
            let speedup = serial_time as f64 / parallel_total as f64;
            println!("🚀 并行加速比: {:.2}x", speedup);
        }
    }

    println!("\n═══════════════════════════════════════════════════════════");
    println!("🎯 性能目标: 30s 以内");
    println!("📍 当前性能: {}", format_duration(parallel_total));
    
    if parallel_total > 30000 {
        let gap = parallel_total - 30000;
        println!("📉 需要优化: {} ({:.1}%)", 
                 format_duration(gap),
                 gap as f64 / parallel_total as f64 * 100.0);
    } else {
        println!("✅ 已达到目标!");
    }
    println!("═══════════════════════════════════════════════════════════\n");

    Ok(())
}
