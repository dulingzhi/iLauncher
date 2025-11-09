// 测试 opera 搜索

use anyhow::Result;
use rusqlite::{Connection, params};

fn main() -> Result<()> {
    let db_path = format!(
        "{}\\iLauncher\\mft_databases\\C.db",
        std::env::var("LOCALAPPDATA")?
    );
    
    let conn = Connection::open(&db_path)?;
    
    println!("=== 测试 Opera 搜索 ===\n");
    
    // 测试1：直接 SQL 查询
    println!("1. 直接 SQL 查询 (LIKE):");
    let mut stmt = conn.prepare("SELECT path FROM files_fts WHERE path LIKE '%opera%' LIMIT 10")?;
    let mut rows = stmt.query([])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        let path: String = row.get(0)?;
        println!("   {}", path);
        count += 1;
    }
    println!("   找到: {} 个结果\n", count);
    
    // 测试2：FTS5 搜索
    println!("2. FTS5 MATCH 搜索 (opera*):");
    let mut stmt = conn.prepare("
        SELECT path, priority, rank
        FROM files_fts 
        WHERE filename MATCH ?1 
        ORDER BY rank, priority DESC 
        LIMIT 10
    ")?;
    let mut rows = stmt.query(params!["opera*"])?;
    count = 0;
    while let Some(row) = rows.next()? {
        let path: String = row.get(0)?;
        let priority: i32 = row.get(1)?;
        let rank: f64 = row.get(2)?;
        let filename = path.rsplit('\\').next().unwrap_or(&path);
        println!("   {} (priority: {}, rank: {:.6})", filename, priority, rank);
        count += 1;
    }
    println!("   找到: {} 个结果\n", count);
    
    // 测试3：查询 opera.exe
    println!("3. FTS5 搜索 'opera.exe':");
    let mut stmt = conn.prepare("
        SELECT path, priority, rank
        FROM files_fts 
        WHERE filename MATCH ?1 
        ORDER BY rank, priority DESC 
        LIMIT 10
    ")?;
    let mut rows = stmt.query(params!["opera.exe*"])?;
    count = 0;
    while let Some(row) = rows.next()? {
        let path: String = row.get(0)?;
        let priority: i32 = row.get(1)?;
        let rank: f64 = row.get(2)?;
        let filename = path.rsplit('\\').next().unwrap_or(&path);
        println!("   {} (priority: {}, rank: {:.6})", filename, priority, rank);
        count += 1;
    }
    println!("   找到: {} 个结果\n", count);
    
    // 测试4：统计总数
    println!("4. 统计数据库总记录数:");
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM files_fts", [], |row| row.get(0))?;
    println!("   总计: {} 条记录\n", count);
    
    Ok(())
}
