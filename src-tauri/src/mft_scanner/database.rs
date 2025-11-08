// SQLite 数据库模块

use anyhow::Result;
use rusqlite::{Connection, params};
use crate::mft_scanner::types::MftFileEntry;
use std::path::Path;

pub struct Database {
    conn: Connection,
    drive_letter: char,
}

impl Database {
    /// 打开数据库
    pub fn open(drive_letter: char, output_dir: &str) -> Result<Self> {
        let db_path = format!("{}\\{}.db", output_dir, drive_letter);
        
        // 确保目录存在
        if let Some(parent) = Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let conn = Connection::open(&db_path)?;
        
        // 🔥 SQLite 性能优化配置（针对快速查询）
        conn.execute_batch("
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -64000;     -- 64MB 缓存（负数表示KB）
            PRAGMA page_size = 65536;       -- 64KB 页大小
            PRAGMA auto_vacuum = 0;
            PRAGMA synchronous = OFF;       -- 关闭同步写入（查询不需要）
            PRAGMA journal_mode = WAL;      -- WAL 模式，读写不阻塞
            PRAGMA locking_mode = NORMAL;   -- 允许并发
            PRAGMA mmap_size = 268435456;   -- 256MB 内存映射（加速读取）
        ")?;
        
        let mut db = Self { conn, drive_letter };
        db.init_tables()?;
        
        Ok(db)
    }
    
    /// 创建 41 个分组表 (list0-list40)
    fn init_tables(&mut self) -> Result<()> {
        self.conn.execute("BEGIN", [])?;
        
        for i in 0..=40 {
            let sql = format!(
                "CREATE TABLE IF NOT EXISTS list{} (
                    ASCII INT,
                    PATH TEXT,
                    PRIORITY INT,
                    PRIMARY KEY(ASCII, PATH, PRIORITY)
                )",
                i
            );
            self.conn.execute(&sql, [])?;
            
            // 🔥 优化：为 PATH 列创建索引（加速 GLOB 查询）
            let index_sql = format!(
                "CREATE INDEX IF NOT EXISTS idx_list{}_path ON list{}(PATH COLLATE NOCASE)",
                i, i
            );
            self.conn.execute(&index_sql, [])?;
        }
        
        self.conn.execute("COMMIT", [])?;
        Ok(())
    }
    
    /// 批量插入文件记录
    pub fn insert_batch(&mut self, entries: &[MftFileEntry]) -> Result<()> {
        self.conn.execute("BEGIN", [])?;
        
        // 预编译 41 个语句
        let mut statements: Vec<_> = (0..=40)
            .map(|i| {
                self.conn.prepare(&format!(
                    "INSERT OR IGNORE INTO list{} (ASCII, PATH, PRIORITY) VALUES (?, ?, ?)",
                    i
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        
        for entry in entries {
            let group = (entry.ascii_sum / 100).min(40) as usize;
            statements[group].execute(params![
                entry.ascii_sum,
                &entry.path,
                entry.priority
            ])?;
        }
        
        self.conn.execute("COMMIT", [])?;
        Ok(())
    }
    
    /// 计算 ASCII 值总和
    pub fn calc_ascii_sum(name: &str) -> i32 {
        name.chars()
            .filter(|c| c.is_ascii())
            .map(|c| c as i32)
            .sum()
    }
    
    /// 查询文件（优化版：利用 ASCII 分区）
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MftFileEntry>> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();
        
        // 🔥 优化1：计算查询字符串的 ASCII 总和，定位到具体的表
        // 这样可以避免遍历所有 41 个表
        let query_ascii = Self::calc_ascii_sum(&query_lower);
        let target_group = (query_ascii / 100).min(40) as usize;
        
        // 🔥 优化2：优先搜索目标表，然后搜索相邻表（ASCII值相近的文件）
        let groups_to_search = vec![
            target_group,
            if target_group > 0 { target_group - 1 } else { 0 },
            if target_group < 40 { target_group + 1 } else { 40 },
        ];
        
        for &group in &groups_to_search {
            if results.len() >= limit {
                break;
            }
            
            let table_name = format!("list{}", group);
            
            // 🔥 优化3：使用 GLOB 代替 LIKE，更快
            // GLOB 是二进制比较，LIKE 是大小写不敏感但更慢
            let sql = format!(
                "SELECT ASCII, PATH, PRIORITY FROM {} WHERE lower(PATH) GLOB ? ORDER BY PRIORITY DESC LIMIT ?",
                table_name
            );
            
            let mut stmt = match self.conn.prepare(&sql) {
                Ok(s) => s,
                Err(_) => continue, // 表可能不存在
            };
            
            // GLOB 模式：*query* 匹配任意位置
            let pattern = format!("*{}*", query_lower);
            
            let rows = stmt.query_map(params![&pattern, limit - results.len()], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                ))
            })?;
            
            for row in rows {
                if let Ok((ascii_sum, path, priority)) = row {
                    // 从路径提取文件名和是否为目录
                    let is_dir = path.ends_with('\\');
                    let name = path
                        .trim_end_matches('\\')
                        .split('\\')
                        .last()
                        .unwrap_or("")
                        .to_string();
                    
                    results.push(MftFileEntry {
                        path,
                        name,
                        is_dir,
                        size: 0,
                        modified: 0,
                        priority,
                        ascii_sum,
                    });
                    
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }
        
        // 如果前3个表没找到足够结果，再搜索其他表（降级策略）
        if results.len() < limit {
            for i in 0..=40 {
                if groups_to_search.contains(&i) || results.len() >= limit {
                    continue;
                }
                
                let table_name = format!("list{}", i);
                let sql = format!(
                    "SELECT ASCII, PATH, PRIORITY FROM {} WHERE lower(PATH) GLOB ? ORDER BY PRIORITY DESC LIMIT ?",
                    table_name
                );
                
                if let Ok(mut stmt) = self.conn.prepare(&sql) {
                    let pattern = format!("*{}*", query_lower);
                    
                    if let Ok(rows) = stmt.query_map(params![&pattern, limit - results.len()], |row| {
                        Ok((
                            row.get::<_, i32>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i32>(2)?,
                        ))
                    }) {
                        for row in rows {
                            if let Ok((ascii_sum, path, priority)) = row {
                                let is_dir = path.ends_with('\\');
                                let name = path
                                    .trim_end_matches('\\')
                                    .split('\\')
                                    .last()
                                    .unwrap_or("")
                                    .to_string();
                                
                                results.push(MftFileEntry {
                                    path,
                                    name,
                                    is_dir,
                                    size: 0,
                                    modified: 0,
                                    priority,
                                    ascii_sum,
                                });
                                
                                if results.len() >= limit {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(results)
    }
}

/// 多盘符搜索（优化版：并行查询 + 连接复用）
pub fn search_all_drives(query: &str, output_dir: &str, limit: usize) -> Result<Vec<MftFileEntry>> {
    use rayon::prelude::*;
    
    // 🔥 优化1：先收集所有存在的数据库路径
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
    
    // 🔥 优化2：并行查询所有盘符（使用 rayon）
    let all_results: Vec<Vec<MftFileEntry>> = existing_drives
        .par_iter()
        .filter_map(|&drive_letter| {
            match Database::open(drive_letter, output_dir) {
                Ok(db) => {
                    // 每个盘符搜索 limit 个结果
                    db.search(query, limit).ok()
                }
                Err(e) => {
                    tracing::warn!("Failed to open database for drive {}: {}", drive_letter, e);
                    None
                }
            }
        })
        .collect();
    
    // 🔥 优化3：合并结果并按优先级排序
    let mut merged: Vec<MftFileEntry> = all_results
        .into_iter()
        .flatten()
        .collect();
    
    // 按优先级降序排序
    merged.sort_by(|a, b| b.priority.cmp(&a.priority));
    
    // 截取前 limit 个
    merged.truncate(limit);
    
    Ok(merged)
}

