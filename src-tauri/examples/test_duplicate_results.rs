use rusqlite::{Connection, params};

fn main() {
    let db_path = format!(
        "{}\\iLauncher\\mft_databases\\C.db",
        std::env::var("LOCALAPPDATA").unwrap()
    );
    
    let conn = Connection::open(&db_path).unwrap();
    
    println!("=== 检查重复和不匹配问题 ===\n");
    
    // 1. 检查 opera.exe 的所有记录
    println!("📊 检查 opera.exe 的所有记录：");
    let mut stmt = conn.prepare("
        SELECT path, filename, priority, rowid
        FROM files_fts 
        WHERE filename MATCH '\"opera.exe\" OR \"opera.exe*\"'
        ORDER BY path
        LIMIT 20
    ").unwrap();
    
    let mut rows = stmt.query([]).unwrap();
    let mut count = 0;
    let mut seen_paths = std::collections::HashSet::new();
    
    while let Some(row) = rows.next().unwrap() {
        let path: String = row.get(0).unwrap();
        let filename: String = row.get(1).unwrap();
        let priority: i32 = row.get(2).unwrap();
        let rowid: i64 = row.get(3).unwrap();
        
        let is_duplicate = !seen_paths.insert(path.clone());
        
        println!("  {}. {} | filename='{}' | priority={} | rowid={} {}",
            count + 1,
            path,
            filename,
            priority,
            rowid,
            if is_duplicate { "❌ 重复" } else { "" }
        );
        count += 1;
    }
    
    println!("\n总计: {} 条记录\n", count);
    
    // 2. 检查为什么 _ope 也会匹配
    println!("📊 检查不匹配的结果（_ope）：");
    let mut stmt = conn.prepare("
        SELECT path, filename
        FROM files_fts 
        WHERE filename MATCH '\"opera.exe\" OR \"opera.exe*\"'
        AND (filename LIKE '%_ope%' OR filename LIKE '%parse_oper%')
        LIMIT 5
    ").unwrap();
    
    let mut rows = stmt.query([]).unwrap();
    while let Some(row) = rows.next().unwrap() {
        let path: String = row.get(0).unwrap();
        let filename: String = row.get(1).unwrap();
        println!("  - {} | filename='{}'", path, filename);
    }
    
    // 3. 统计重复记录数
    println!("\n📊 统计重复路径：");
    let count: i64 = conn.query_row("
        SELECT COUNT(*) 
        FROM (
            SELECT path, COUNT(*) as cnt
            FROM files_fts
            GROUP BY path
            HAVING cnt > 1
        )
    ", [], |row| row.get(0)).unwrap();
    
    println!("  重复路径数量: {}", count);
}
