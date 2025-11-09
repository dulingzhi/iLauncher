// 测试 FTS5 搜索结果是否正确

use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;

#[derive(Debug)]
struct MftFileEntry {
    path: String,
    priority: i32,
}

impl MftFileEntry {
    fn name(&self) -> String {
        self.path
            .trim_end_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or("")
            .to_string()
    }
    
    fn is_dir(&self) -> bool {
        self.path.ends_with('\\')
    }
}

fn search_database(drive: char, query: &str, limit: usize) -> Result<Vec<MftFileEntry>> {
    let db_path = format!(
        "{}\\iLauncher\\mft_databases\\{}.db",
        std::env::var("LOCALAPPDATA")?,
        drive
    );
    
    let conn = Connection::open(&db_path)?;
    
    let fts_query = format!("{}*", query);
    let sql = "
        SELECT path, priority 
        FROM files_fts 
        WHERE filename MATCH ?1 
        ORDER BY priority DESC 
        LIMIT ?2
    ";
    
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(params![fts_query, limit])?;
    
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(MftFileEntry {
            path: row.get(0)?,
            priority: row.get(1)?,
        });
    }
    
    Ok(results)
}

fn main() -> Result<()> {
    println!("=== 测试搜索结果路径正确性 ===\n");
    
    // 测试查询
    let test_queries = vec![
        ("chrome", "Chrome 浏览器"),
        ("notepad", "记事本"),
        ("cmd", "命令提示符"),
    ];
    
    for (query, desc) in test_queries {
        println!("🔍 搜索: {} ({})", query, desc);
        
        // 搜索 C 盘
        match search_database('C', query, 5) {
            Ok(results) => {
                println!("  ✅ 找到 {} 个结果:", results.len());
                for (i, entry) in results.iter().enumerate() {
                    // 检查路径是否存在
                    let exists = Path::new(&entry.path).exists();
                    let status = if exists { "✓" } else { "✗" };
                    
                    println!("    {}. {} [{}] {}", 
                        i + 1, 
                        status,
                        entry.path,
                        if exists { "" } else { "（文件不存在）" }
                    );
                    
                    // 检查文件名是否正确提取
                    let expected_filename = entry.path
                        .trim_end_matches('\\')
                        .rsplit('\\')
                        .next()
                        .unwrap_or("");
                    let actual_filename = entry.name();
                    
                    if expected_filename != actual_filename {
                        println!("      ⚠️  文件名提取错误: 期望 '{}', 实际 '{}'", 
                            expected_filename, actual_filename);
                    }
                }
            }
            Err(e) => println!("  ❌ 搜索失败: {}", e),
        }
        println!();
    }
    
    // 额外测试：检查数据库中的原始数据
    println!("=== 检查数据库原始数据 ===\n");
    
    let db_path = format!(
        "{}\\iLauncher\\mft_databases\\C.db",
        std::env::var("LOCALAPPDATA")?
    );
    
    let conn = Connection::open(&db_path)?;
    
    // 读取前 10 条记录
    let mut stmt = conn.prepare("SELECT path, priority FROM files_fts LIMIT 10")?;
    let mut rows = stmt.query([])?;
    
    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        entries.push(MftFileEntry {
            path: row.get(0)?,
            priority: row.get(1)?,
        });
    }
    
    println!("✅ 数据库总记录数统计...");
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM files_fts", [], |row| row.get(0))?;
    println!("   总计: {} 条记录\n", count);
    
    println!("📋 前 10 条记录样本:");
    for (i, entry) in entries.iter().enumerate() {
        println!("  {}. 路径: {}", i + 1, entry.path);
        println!("     文件名: {}", entry.name());
        println!("     是否目录: {}", entry.is_dir());
        println!("     优先级: {}", entry.priority);
        println!();
    }
    
    Ok(())
}
