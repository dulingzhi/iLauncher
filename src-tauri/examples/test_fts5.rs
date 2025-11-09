// SQLite FTS5 性能测试

use anyhow::Result;
use rusqlite::{Connection, params};
use std::time::Instant;

fn main() -> Result<()> {
    let db_dir = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
    let test_db = format!("{}\\iLauncher\\fts5_test.db", db_dir);
    
    println!("创建 FTS5 测试数据库: {}\n", test_db);
    
    // 删除旧数据库
    let _ = std::fs::remove_file(&test_db);
    
    let mut conn = Connection::open(&test_db)?;
    
    // 1. 创建 FTS5 虚拟表（使用 ascii 分词器，支持部分匹配）
    println!("1. 创建 FTS5 表...");
    conn.execute_batch("
        CREATE VIRTUAL TABLE files_fts USING fts5(
            path,
            filename,
            priority UNINDEXED,
            tokenize = 'ascii'
        );
    ")?;
    
    // 2. 从现有数据库导入数据（使用 Rust 提取文件名）
    println!("2. 导入测试数据...");
    let source_db = format!("{}\\iLauncher\\mft_databases\\C.db", db_dir);
    
    let start = Instant::now();
    conn.execute(&format!("ATTACH DATABASE '{}' AS source", source_db), [])?;
    
    // 🔥 开启事务批量插入（大幅提升插入性能）
    conn.execute("BEGIN TRANSACTION", [])?;
    
    // 从多个表读取数据并插入到 FTS5 表（每个表采样一些，确保多样性）
    let mut count = 0;
    let samples_per_table = 2000;  // 每个表采样 2000 条
    
    for table_idx in 0..=40 {  // 从所有 41 个表中采样
        let query = format!("SELECT PATH, PRIORITY FROM source.list{} LIMIT {}", table_idx, samples_per_table);
        let mut stmt = conn.prepare(&query)?;
        let mut rows = stmt.query([])?;
        
        // 准备插入语句（重用）
        let mut insert_stmt = conn.prepare(
            "INSERT INTO files_fts(path, filename, priority) VALUES (?1, ?2, ?3)"
        )?;
        
        while let Some(row) = rows.next()? {
            if count >= 50000 {
                break;
            }
            
            let path: String = row.get(0)?;
            let priority: i32 = row.get(1)?;
            
            // 提取文件名（最后一个 \ 之后的部分）
            let filename = path.rsplit('\\').next().unwrap_or(&path);
            
            insert_stmt.execute(params![&path, filename, priority])?;
            
            count += 1;
        }
        
        if count >= 50000 {
            break;
        }
    }
    
    // 提交事务
    conn.execute("COMMIT", [])?;
    conn.execute("DETACH DATABASE source", [])?;
    println!("   导入 {} 条记录，耗时: {:.2}s", count, start.elapsed().as_secs_f64());
    
    // 验证数据导入
    let sample_count: i64 = conn.query_row("SELECT COUNT(*) FROM files_fts", [], |row| row.get(0))?;
    println!("   数据库记录数: {}", sample_count);
    
    // 打印几条示例数据
    println!("   示例数据:");
    let mut stmt = conn.prepare("SELECT path, filename FROM files_fts LIMIT 3")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let path: String = row.get(0)?;
        let filename: String = row.get(1)?;
        println!("     文件名: {} | 路径: {}", filename, path);
    }
    println!();
    
    // 3. 测试 FTS5 匹配功能
    println!("3. 测试 FTS5 匹配功能:");
    
    // 测试几个简单的查询
    let test_searches = vec![
        ("program", "搜索 program"),
        ("microsoft", "搜索 microsoft"),
        ("files", "搜索 files"),
        ("10", "搜索 10"),
    ];
    
    for (keyword, desc) in &test_searches {
        let query = format!("{}*", keyword);
        let mut stmt = conn.prepare("SELECT path, filename FROM files_fts WHERE filename MATCH ?1 LIMIT 3")?;
        let mut rows = stmt.query(params![query])?;
        
        println!("  {} - 查询: '{}'", desc, query);
        let mut count = 0;
        while let Some(row) = rows.next()? {
            let filename: String = row.get(1)?;
            println!("    找到: {}", filename);
            count += 1;
        }
        if count == 0 {
            println!("    （无结果）");
        }
        println!();
    }
    
    // 4. 性能测试
    println!("4. 性能测试:\n");
    
    let test_queries = vec![
        ("chrome", "常见程序"),
        ("sys", "系统文件"),
        ("opera", "少见程序"),
        ("test", "通用词"),
    ];
    
    for (query, desc) in &test_queries {
        // FTS5 搜索（支持前缀匹配）
        let fts_query = format!("{}*", query);  // 添加 * 支持前缀匹配
        
        let start = Instant::now();
        let mut stmt = conn.prepare("
            SELECT path, priority 
            FROM files_fts 
            WHERE filename MATCH ?1 
            ORDER BY priority DESC 
            LIMIT 50
        ")?;
        
        let results: Vec<(String, i32)> = stmt
            .query_map(params![fts_query], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        
        let fts5_time = start.elapsed();
        
        println!("  {} ({})", query, desc);
        println!("    FTS5 搜索: {:.2} ms | {} 个结果", 
            fts5_time.as_secs_f64() * 1000.0, 
            results.len()
        );
        
        if !results.is_empty() {
            println!("      示例: {}", results[0].0);
        }
        println!();
    }
    
    // 4. 对比传统 LIKE 搜索（如果有原始表）
    println!("    // 5. 对比传统搜索（从原始数据库）:");
    
    let source_conn = Connection::open(&source_db)?;
    
    for (query, desc) in &test_queries[0..2] {  // 只测试前2个
        let start = Instant::now();
        
        let mut total = 0;
        for i in 0..=40 {
            let sql = format!(
                "SELECT PATH, PRIORITY FROM list{} 
                 WHERE PATH LIKE '%{}%' 
                 ORDER BY PRIORITY DESC 
                 LIMIT 50",
                i, query
            );
            
            if let Ok(mut stmt) = source_conn.prepare(&sql) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
                }) {
                    total += rows.count();
                }
            }
        }
        
        let like_time = start.elapsed();
        
        println!("  {} ({})", query, desc);
        println!("    LIKE 搜索: {:.2} ms | {} 个结果", 
            like_time.as_secs_f64() * 1000.0, 
            total
        );
        println!();
    }
    
    println!("\n测试完成！");
    println!("FTS5 数据库大小: {:.2} MB", 
        std::fs::metadata(&test_db)?.len() as f64 / 1024.0 / 1024.0
    );
    
    Ok(())
}
