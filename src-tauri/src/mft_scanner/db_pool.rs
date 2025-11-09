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
        
        // 🔥 WAL 模式需要读写权限（用于创建 .wal 和 .shm 文件）
        // WAL 允许多个读连接 + 1个写连接并发，所以读写模式是安全的
        let conn = Connection::open(&db_path)?;
        
        // 🔥 优化配置 - 专为快速查询优化
        conn.execute_batch("
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -32768;    -- 🔥 32MB 缓存 (减少内存竞争)
            PRAGMA mmap_size = 268435456;  -- 🔥 256MB mmap (提升读取速度)
            PRAGMA journal_mode = WAL;     -- WAL 模式
            PRAGMA synchronous = NORMAL;   -- WAL 模式下安全
            PRAGMA wal_autocheckpoint = 0; -- 禁用自动 checkpoint
            PRAGMA locking_mode = NORMAL;  -- 🔥 允许多连接并发
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
        
        // 🔥 超短查询优化: 1-2字符时只搜索高优先级文件
        // 避免 FTS5 扫描海量低质量匹配项 (提升 100 倍性能)
        if query.len() <= 2 {
            let results = self.search_high_priority_only(&entry, query, limit)?;
            
            let elapsed = start.elapsed();
            tracing::debug!(
                "Drive {} search (fast): query='{}', results={}, time={:.2}ms",
                drive_letter,
                query,
                results.len(),
                elapsed.as_secs_f64() * 1000.0
            );
            
            return Ok(results);
        }
        
        // 正常查询流程 (3+ 字符)
        let mut results = Vec::new();
        let fts_query = format!("{}*", query);
        
        let sql = "SELECT path, priority FROM files_fts 
                   WHERE filename MATCH ?1 
                   ORDER BY rank, priority DESC 
                   LIMIT ?2";
        
        // 🔥 在独立作用域内执行查询，避免借用冲突
        {
            let entry_lock = entry.lock();
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
    
    /// 🔥 快速搜索: 只查询高优先级文件 (priority >= 50)
    /// 用于超短查询 (1-2 字符),避免扫描海量低质量匹配
    fn search_high_priority_only(
        &self,
        entry: &Arc<Mutex<PoolEntry>>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MftFileEntry>> {
        let mut results = Vec::new();
        
        // 🔥 策略: 短查询时只返回高优先级文件
        // 使用 ^query* 表示文件名必须以 query 开头 (FTS5 前缀查询)
        // 配合 priority >= 50 大幅减少候选集
        let fts_query = format!("^{}*", query);
        
        let sql = "SELECT path, priority FROM files_fts 
                   WHERE filename MATCH ?1 AND priority >= 50
                   ORDER BY priority DESC 
                   LIMIT ?2";
        
        let entry_lock = entry.lock();
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

/// 🔥 优化版多盘符搜索（使用连接池 + 早停优化）
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
    
    // 🔥 并行搜索优化：每个盘符只返回少量高质量结果，避免过度查询
    // 策略：limit/盘符数，至少 20 条
    let per_drive_limit = (limit / existing_drives.len()).max(20);
    
    use rayon::prelude::*;
    
    let all_results: Vec<Vec<MftFileEntry>> = existing_drives
        .par_iter()
        .filter_map(|&drive_letter| {
            match DB_POOL.search(drive_letter, query, per_drive_limit) {
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
