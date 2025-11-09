use rusqlite::{Connection, params};

fn main() {
    let db_path = format!(
        "{}\\iLauncher\\mft_databases\\C.db",
        std::env::var("LOCALAPPDATA").unwrap()
    );
    
    let conn = Connection::open(&db_path).unwrap();
    
    println!("=== 测试 GROUP BY 去重效果 ===\n");
    
    // 测试去重查询
    let query = "opera.exe";
    let fts_query = format!("\"{}\" OR \"{}*\"", query, query);
    
    println!("🔍 搜索: {}", query);
    println!("   FTS5 查询: {}\n", fts_query);
    
    // 1. 不去重的查询
    println!("📊 不去重查询（旧方案）：");
    let mut stmt = conn.prepare("
        SELECT path, priority, rank
        FROM files_fts 
        WHERE filename MATCH ?1 
        ORDER BY rank, priority DESC 
        LIMIT 10
    ").unwrap();
    
    let mut rows = stmt.query(params![fts_query]).unwrap();
    let mut paths = Vec::new();
    
    while let Some(row) = rows.next().unwrap() {
        let path: String = row.get(0).unwrap();
        paths.push(path.clone());
    }
    
    println!("   结果数: {}", paths.len());
    let unique_paths: std::collections::HashSet<_> = paths.iter().collect();
    println!("   唯一路径数: {}", unique_paths.len());
    println!("   重复率: {:.1}%\n", (1.0 - unique_paths.len() as f64 / paths.len() as f64) * 100.0);
    
    // 2. GROUP BY 去重查询
    println!("📊 GROUP BY 去重查询（新方案）：");
    let mut stmt = conn.prepare("
        SELECT path, priority, MIN(rank) as best_rank
        FROM files_fts 
        WHERE filename MATCH ?1 
        GROUP BY path
        ORDER BY best_rank, priority DESC 
        LIMIT 10
    ").unwrap();
    
    let mut rows = stmt.query(params![fts_query]).unwrap();
    let mut count = 0;
    
    println!("   结果:");
    while let Some(row) = rows.next().unwrap() {
        let path: String = row.get(0).unwrap();
        let priority: i32 = row.get(1).unwrap();
        let rank: f64 = row.get(2).unwrap();
        let filename = path.rsplit('\\').next().unwrap_or(&path);
        
        println!("     {}. {} (rank: {:.6}, priority: {})", 
            count + 1, filename, rank, priority);
        count += 1;
    }
    
    println!("\n   ✅ 找到 {} 个唯一结果", count);
}
