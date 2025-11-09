// 测试修复后的搜索

use anyhow::Result;
use rusqlite::{Connection, params};

fn main() -> Result<()> {
    let db_path = format!(
        "{}\\iLauncher\\mft_databases\\C.db",
        std::env::var("LOCALAPPDATA")?
    );
    
    let conn = Connection::open(&db_path)?;
    
    println!("=== 测试修复后的搜索 ===\n");
    
    let test_queries = vec![
        ("opera.exe", "完整文件名"),
        ("opera", "部分文件名"),
        ("chrome", "常见程序"),
        ("notepad.exe", "记事本"),
    ];
    
    for (query, desc) in test_queries {
        println!("🔍 搜索: {} ({})", query, desc);
        
        // 使用新的 OR 查询逻辑（双引号包裹前缀匹配）
        let fts_query = format!("\"{}\" OR \"{}*\"", query, query);
        println!("   FTS5 查询: {}", fts_query);
        
        let mut stmt = conn.prepare("
            SELECT path, priority, rank
            FROM files_fts 
            WHERE filename MATCH ?1 
            ORDER BY rank, priority DESC 
            LIMIT 10
        ")?;
        
        let mut rows = stmt.query(params![fts_query])?;
        
        let mut count = 0;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let priority: i32 = row.get(1)?;
            let rank: f64 = row.get(2)?;
            let filename = path.rsplit('\\').next().unwrap_or(&path);
            
            if count == 0 {
                println!("   结果:");
            }
            
            println!("     {}. {} (rank: {:.6}, priority: {})", 
                count + 1, filename, rank, priority);
            count += 1;
        }
        
        if count == 0 {
            println!("   ❌ 没有结果");
        } else {
            println!("   ✅ 找到 {} 个结果", count);
        }
        
        println!();
    }
    
    Ok(())
}
