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
        
        // SQLite 优化配置（参考 C++ 实现）
        conn.execute_batch("
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = 262144;
            PRAGMA page_size = 65536;
            PRAGMA auto_vacuum = 0;
            PRAGMA synchronous = OFF;
            PRAGMA journal_mode = WAL;
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
    
    /// 查询文件（模糊匹配）
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MftFileEntry>> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();
        
        // 遍历所有 41 个表
        for i in 0..=40 {
            let table_name = format!("list{}", i);
            let sql = format!(
                "SELECT ASCII, PATH, PRIORITY FROM {} WHERE PATH LIKE ? LIMIT ?",
                table_name
            );
            
            let mut stmt = self.conn.prepare(&sql)?;
            let pattern = format!("%{}%", query_lower);
            
            let rows = stmt.query_map(params![&pattern, limit], |row| {
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
                }
                
                if results.len() >= limit {
                    break;
                }
            }
            
            if results.len() >= limit {
                break;
            }
        }
        
        Ok(results)
    }
}

/// 多盘符搜索（辅助函数）
pub fn search_all_drives(query: &str, output_dir: &str, limit: usize) -> Result<Vec<MftFileEntry>> {
    let mut all_results = Vec::new();
    
    // 遍历所有可能的盘符
    for drive in b'A'..=b'Z' {
        let drive_letter = drive as char;
        let db_path = format!("{}\\{}.db", output_dir, drive_letter);
        
        if Path::new(&db_path).exists() {
            match Database::open(drive_letter, output_dir) {
                Ok(db) => {
                    if let Ok(mut results) = db.search(query, limit) {
                        all_results.append(&mut results);
                        
                        if all_results.len() >= limit {
                            all_results.truncate(limit);
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to open database for drive {}: {}", drive_letter, e);
                }
            }
        }
    }
    
    Ok(all_results)
}

