// 为现有数据库添加 PRIORITY 索引

use anyhow::Result;
use rusqlite::Connection;
use std::time::Instant;

fn main() -> Result<()> {
    let db_dir = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
    let db_dir = format!("{}\\iLauncher\\mft_databases", db_dir);
    
    println!("数据库目录: {}", db_dir);
    
    // 处理所有驱动器
    for drive in ['C', 'D', 'E', 'F', 'G'] {
        let db_path = format!("{}\\{}.db", db_dir, drive);
        
        if !std::path::Path::new(&db_path).exists() {
            continue;
        }
        
        println!("\n处理驱动器 {}: ...", drive);
        let start = Instant::now();
        
        let mut conn = Connection::open(&db_path)?;
        
        // 检查是否已有索引
        let has_index: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_priority_0'",
            [],
            |row| row.get(0)
        ).unwrap_or(0) > 0;
        
        if has_index {
            println!("  ✓ 索引已存在，跳过");
            continue;
        }
        
        println!("  开始创建索引...");
        conn.execute("BEGIN", [])?;
        
        for i in 0..=40 {
            let sql = format!(
                "CREATE INDEX IF NOT EXISTS idx_priority_{} ON list{}(PRIORITY)",
                i, i
            );
            conn.execute(&sql, [])?;
            
            if i % 10 == 0 {
                print!("  进度: {}/41\r", i + 1);
            }
        }
        
        conn.execute("COMMIT", [])?;
        
        let elapsed = start.elapsed();
        println!("  ✓ 完成！耗时: {:.2}s", elapsed.as_secs_f64());
    }
    
    println!("\n所有索引创建完成！");
    Ok(())
}
