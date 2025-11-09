// 测试 FTS5 BM25 排序效果

use anyhow::Result;
use rusqlite::{Connection, params};

#[derive(Debug)]
struct SearchResult {
    path: String,
    filename: String,
    priority: i32,
    rank: f64,
}

fn search_with_rank(drive: char, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let db_path = format!(
        "{}\\iLauncher\\mft_databases\\{}.db",
        std::env::var("LOCALAPPDATA")?,
        drive
    );
    
    let conn = Connection::open(&db_path)?;
    
    let fts_query = format!("{}*", query);
    
    // 🔥 使用 BM25 排序
    let sql = "
        SELECT path, priority, rank
        FROM files_fts 
        WHERE filename MATCH ?1 
        ORDER BY rank, priority DESC 
        LIMIT ?2
    ";
    
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(params![fts_query, limit])?;
    
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        let path: String = row.get(0)?;
        let priority: i32 = row.get(1)?;
        let rank: f64 = row.get(2)?;
        
        // 提取文件名
        let filename = path.rsplit('\\').next().unwrap_or(&path).to_string();
        
        results.push(SearchResult {
            path,
            filename,
            priority,
            rank,
        });
    }
    
    Ok(results)
}

fn main() -> Result<()> {
    println!("=== FTS5 BM25 排序测试 ===\n");
    
    // 测试查询：通用词和具体词
    let test_queries = vec![
        ("sys", "通用词（预期：完整匹配优先）"),
        ("chrome", "常见程序"),
        ("test", "通用词"),
        ("python", "编程语言"),
    ];
    
    for (query, desc) in test_queries {
        println!("🔍 搜索: {} ({})", query, desc);
        println!("{}", "=".repeat(80));
        
        match search_with_rank('C', query, 10) {
            Ok(results) => {
                println!("✅ 找到 {} 个结果（按 BM25 相关性排序）:\n", results.len());
                
                for (i, result) in results.iter().enumerate() {
                    println!("{}. 📄 {}", i + 1, result.filename);
                    println!("   路径: {}", result.path);
                    println!("   BM25 分数: {:.6} (越小越相关)", result.rank);
                    println!("   优先级: {}", result.priority);
                    
                    // 分析匹配类型
                    let filename_lower = result.filename.to_lowercase();
                    let query_lower = query.to_lowercase();
                    
                    let match_type = if filename_lower == query_lower {
                        "🎯 完全匹配"
                    } else if filename_lower.starts_with(&query_lower) {
                        "⭐ 前缀匹配"
                    } else if filename_lower.contains(&query_lower) {
                        "📌 部分匹配"
                    } else {
                        "❓ 其他匹配"
                    };
                    
                    println!("   匹配类型: {}", match_type);
                    println!();
                }
            }
            Err(e) => println!("❌ 搜索失败: {}\n", e),
        }
        
        println!("\n");
    }
    
    // 对比测试：优先级排序 vs BM25 排序
    println!("\n=== 对比测试：不同排序策略 ===\n");
    
    let db_path = format!(
        "{}\\iLauncher\\mft_databases\\C.db",
        std::env::var("LOCALAPPDATA")?
    );
    let conn = Connection::open(&db_path)?;
    
    let query = "sys";
    let fts_query = format!("{}*", query);
    
    println!("🔍 查询: {}\n", query);
    
    // 策略1：只按优先级排序（旧方案）
    println!("📊 策略1：只按优先级排序");
    println!("{}", "-".repeat(80));
    
    let sql_priority = "
        SELECT path, priority, rank
        FROM files_fts 
        WHERE filename MATCH ?1 
        ORDER BY priority DESC, rank
        LIMIT 5
    ";
    
    let mut stmt = conn.prepare(sql_priority)?;
    let mut rows = stmt.query(params![fts_query])?;
    
    let mut i = 1;
    while let Some(row) = rows.next()? {
        let path: String = row.get(0)?;
        let priority: i32 = row.get(1)?;
        let rank: f64 = row.get(2)?;
        let filename = path.rsplit('\\').next().unwrap_or(&path);
        
        println!("{}. {} (优先级: {}, BM25: {:.6})", i, filename, priority, rank);
        i += 1;
    }
    
    println!("\n📊 策略2：BM25 + 优先级排序（新方案）");
    println!("{}", "-".repeat(80));
    
    let sql_bm25 = "
        SELECT path, priority, rank
        FROM files_fts 
        WHERE filename MATCH ?1 
        ORDER BY rank, priority DESC
        LIMIT 5
    ";
    
    let mut stmt = conn.prepare(sql_bm25)?;
    let mut rows = stmt.query(params![fts_query])?;
    
    let mut i = 1;
    while let Some(row) = rows.next()? {
        let path: String = row.get(0)?;
        let priority: i32 = row.get(1)?;
        let rank: f64 = row.get(2)?;
        let filename = path.rsplit('\\').next().unwrap_or(&path);
        
        println!("{}. {} (BM25: {:.6}, 优先级: {})", i, filename, rank, priority);
        i += 1;
    }
    
    println!("\n✅ 测试完成！");
    println!("\n💡 观察：");
    println!("   - 策略1 可能优先显示不相关的高优先级文件");
    println!("   - 策略2 优先显示最相关的文件（完整匹配 > 前缀匹配）");
    println!("   - BM25 自动权衡：相关性 vs 文件类型优先级");
    
    Ok(())
}
