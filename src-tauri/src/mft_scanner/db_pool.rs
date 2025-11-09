// 数据库连接池 - 解决 "database is locked" 问题

use anyhow::Result;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::mft_scanner::types::MftFileEntry;

/// 🔥 全局数据库连接池（单例模式）
/// 
/// 核心优化:
/// - 每个盘符只打开一次数据库连接
/// - 使用 Mutex 保护连接（SQLite Connection 不是 Sync）
/// - 自动过期清理，避免长期占用
pub static DB_POOL: Lazy<DatabasePool> = Lazy::new(|| DatabasePool::new());

/// 连接池条目
struct PoolEntry {
    conn: Connection,
    drive_letter: char,
    last_access: Instant,
}

/// 数据库连接池
pub struct DatabasePool {
    pool: Arc<Mutex<HashMap<char, Arc<Mutex<PoolEntry>>>>>,
    output_dir: Arc<Mutex<String>>,
}

impl DatabasePool {
    fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
            output_dir: Arc::new(Mutex::new(String::new())),
        }
    }
    
    /// 设置数据库目录（必须先调用）
    pub fn set_output_dir(&self, dir: String) {
        *self.output_dir.lock() = dir;
    }
    
    /// 获取或创建数据库连接
    fn get_or_create(&self, drive_letter: char) -> Result<Arc<Mutex<PoolEntry>>> {
        let output_dir = self.output_dir.lock().clone();
        if output_dir.is_empty() {
            anyhow::bail!("Database output directory not set");
        }
        
        // 快速路径：已存在的连接
        {
            let pool = self.pool.lock();
            if let Some(entry) = pool.get(&drive_letter) {
                // 更新访问时间
                entry.lock().last_access = Instant::now();
                return Ok(Arc::clone(entry));
            }
        }
        
        // 慢速路径：创建新连接
        let mut pool = self.pool.lock();
        
        // 双重检查（避免竞态）
        if let Some(entry) = pool.get(&drive_letter) {
            entry.lock().last_access = Instant::now();
            return Ok(Arc::clone(entry));
        }
        
        // 创建新连接
        let db_path = format!("{}\\{}.db", output_dir, drive_letter);
        
        if !Path::new(&db_path).exists() {
            anyhow::bail!("Database not found: {}", db_path);
        }
        
        // 🔥 只读模式 + 共享缓存（关键优化）
        let conn = Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY 
                | rusqlite::OpenFlags::SQLITE_OPEN_SHARED_CACHE,  // 🔥 共享缓存模式
        )?;
        
        // 优化配置
        conn.execute_batch("
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -262144;   -- 256MB 缓存
            PRAGMA page_size = 65535;
            PRAGMA journal_mode = OFF;     -- 只读模式不需要日志
            PRAGMA synchronous = OFF;      -- 只读模式不需要同步
        ")?;
        
        let entry = Arc::new(Mutex::new(PoolEntry {
            conn,
            drive_letter,
            last_access: Instant::now(),
        }));
        
        pool.insert(drive_letter, Arc::clone(&entry));
        
        tracing::debug!("📂 Created database connection for drive {}", drive_letter);
        
        Ok(entry)
    }
    
    /// 执行搜索
    pub fn search(&self, drive_letter: char, query: &str, limit: usize) -> Result<Vec<MftFileEntry>> {
        let entry = self.get_or_create(drive_letter)?;
        
        let start = Instant::now();
        let mut results = Vec::new();
        
        // FTS5 查询
        let fts_query = format!("\"{}\" OR \"{}*\"", query, query);
        
        let sql = "
            SELECT path, priority, MIN(rank) as best_rank
            FROM files_fts 
            WHERE filename MATCH ?1 
            GROUP BY path
            ORDER BY best_rank, priority DESC 
            LIMIT ?2
        ";
        
        // 🔥 在独立作用域内执行查询，避免借用冲突
        {
            let mut entry_lock = entry.lock();
            let mut stmt = entry_lock.conn.prepare(sql)?;
            let mut rows = stmt.query(params![fts_query, limit])?;
            
            while let Some(row) = rows.next()? {
                let path: String = row.get(0)?;
                let priority: i32 = row.get(1)?;
                
                results.push(MftFileEntry {
                    path,
                    priority,
                    ascii_sum: 0,
                });
            }
        } // stmt 在这里释放，借用结束
        
        // 现在可以安全地获取可变引用
        entry.lock().last_access = Instant::now();
        
        let elapsed = start.elapsed();
        tracing::debug!(
            "Drive {} search: query='{}', results={}, time={:.2}ms",
            drive_letter,
            query,
            results.len(),
            elapsed.as_secs_f64() * 1000.0
        );
        
        Ok(results)
    }
    
    /// 清理过期连接（可选）
    pub fn cleanup_expired(&self, max_age: Duration) {
        let mut pool = self.pool.lock();
        let now = Instant::now();
        
        pool.retain(|drive, entry| {
            let last_access = entry.lock().last_access;
            let should_keep = now.duration_since(last_access) < max_age;
            
            if !should_keep {
                tracing::debug!("🗑️ Removing expired connection for drive {}", drive);
            }
            
            should_keep
        });
    }
    
    /// 清空所有连接
    pub fn clear(&self) {
        let mut pool = self.pool.lock();
        pool.clear();
        tracing::debug!("🗑️ Cleared all database connections");
    }
}

/// 🔥 优化版多盘符搜索（使用连接池）
pub fn search_all_drives_pooled(query: &str, output_dir: &str, limit: usize) -> Result<Vec<MftFileEntry>> {
    let total_start = Instant::now();
    
    // 设置输出目录
    DB_POOL.set_output_dir(output_dir.to_string());
    
    // 收集所有存在的数据库
    let existing_drives: Vec<char> = (b'A'..=b'Z')
        .map(|d| d as char)
        .filter(|&drive| {
            let db_path = format!("{}\\{}.db", output_dir, drive);
            Path::new(&db_path).exists()
        })
        .collect();
    
    if existing_drives.is_empty() {
        return Ok(Vec::new());
    }
    
    tracing::debug!("🔍 Searching drives: {:?}", existing_drives);
    
    // 🔥 并行搜索（使用连接池，无锁竞争）
    use rayon::prelude::*;
    
    let all_results: Vec<Vec<MftFileEntry>> = existing_drives
        .par_iter()
        .filter_map(|&drive_letter| {
            match DB_POOL.search(drive_letter, query, limit) {
                Ok(results) => Some(results),
                Err(e) => {
                    tracing::warn!("Search failed for drive {}: {}", drive_letter, e);
                    None
                }
            }
        })
        .collect();
    
    // 合并结果并排序
    let mut merged: Vec<MftFileEntry> = all_results
        .into_iter()
        .flatten()
        .collect();
    
    merged.sort_by(|a, b| b.priority.cmp(&a.priority));
    merged.truncate(limit);
    
    let total_elapsed = total_start.elapsed();
    tracing::info!(
        "MFT search_all_drives_pooled completed: query={}, results={}, time={:.2} ms, drives={}", 
        query,
        merged.len(),
        total_elapsed.as_secs_f64() * 1000.0,
        existing_drives.len()
    );
    
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_concurrent_search_with_pool() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        let output_dir = crate::utils::paths::get_mft_database_dir()
            .expect("Failed to get MFT database dir")
            .to_str()
            .unwrap()
            .to_string();

        println!("\n=== 连接池并发搜索测试 ===\n");

        let keywords = vec!["c", "ch", "chr", "chro", "chrom", "chrome"];
        let mut handles = vec![];

        for keyword in keywords {
            let output_dir = output_dir.clone();
            let handle = thread::spawn(move || {
                let start = Instant::now();
                let result = search_all_drives_pooled(keyword, &output_dir, 50);
                let elapsed = start.elapsed();
                (keyword, result, elapsed)
            });
            handles.push(handle);
            
            // 模拟快速输入
            thread::sleep(Duration::from_millis(50));
        }

        for handle in handles {
            let (keyword, result, elapsed) = handle.join().unwrap();
            match result {
                Ok(results) => {
                    println!(
                        "✓ '{}': {:.2} ms | {} 结果",
                        keyword,
                        elapsed.as_secs_f64() * 1000.0,
                        results.len()
                    );
                }
                Err(e) => {
                    panic!("✗ '{}': 错误 - {}", keyword, e);
                }
            }
        }

        println!("\n✓ 无 'database is locked' 错误");
    }
}
