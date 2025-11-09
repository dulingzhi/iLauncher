// 测试 FTS5 搜索性能

use rusqlite::{Connection, params};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_dir = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
    
    println!("=== FTS5 搜索性能测试 ===\n");
    
    // 测试查询列表
    let test_queries = vec![
        ("chrome", "常见程序"),
        ("sys", "系统文件"),
        ("test", "通用词"),
        ("vscode", "开发工具"),
        ("python", "编程语言"),
    ];
    
    // 测试所有驱动器
    for drive in ['C', 'D', 'E'] {
        let db_path = format!("{}\\iLauncher\\mft_databases\\{}.db", db_dir, drive);
        
        if !std::path::Path::new(&db_path).exists() {
            println!("⏭️ 跳过 {}: (数据库不存在)", drive);
            continue;
        }
        
        println!("📀 测试驱动器 {}:", drive);
        
        let conn = Connection::open(&db_path)?;
        
        for (query, desc) in &test_queries {
            // FTS5 搜索
            let fts_query = format!("{}*", query);
            let start = Instant::now();
            
            let sql = "
                SELECT path, priority 
                FROM files_fts 
                WHERE filename MATCH ?1 
                ORDER BY priority DESC 
                LIMIT 50
            ";
            
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query(params![fts_query])?;
            
            let mut count = 0;
            let mut first_result = None;
            
            while let Some(row) = rows.next()? {
                if count == 0 {
                    first_result = Some(row.get::<_, String>(0)?);
                }
                count += 1;
            }
            
            let elapsed = start.elapsed();
            
            println!("  {} ({}): {:.2} ms | {} 个结果", 
                query, 
                desc,
                elapsed.as_secs_f64() * 1000.0,
                count
            );
            
            if let Some(path) = first_result {
                let filename = path.rsplit('\\').next().unwrap_or(&path);
                println!("    示例: {}", filename);
            }
        }
        
        println!();
    }
    
    println!("✅ 测试完成！");
    
    Ok(())
}
