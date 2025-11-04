// 使用统计系统 - 记录用户行为，智能排序结果

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct QueryStat {
    pub query: String,
    pub count: i32,
    pub last_used: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ResultStat {
    pub result_id: String,
    pub plugin_id: String,
    pub title: String,
    pub count: i32,
    pub last_used: DateTime<Utc>,
}

pub struct StatisticsManager {
    db: Arc<Mutex<Connection>>,
}

impl StatisticsManager {
    /// 创建统计管理器
    pub fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        let conn = Connection::open(db_path)?;
        
        // 创建表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS queries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                count INTEGER DEFAULT 1,
                last_used TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_queries_query ON queries(query)",
            [],
        )?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS result_clicks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                result_id TEXT NOT NULL,
                plugin_id TEXT NOT NULL,
                title TEXT NOT NULL,
                count INTEGER DEFAULT 1,
                last_used TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_result_clicks_result ON result_clicks(result_id)",
            [],
        )?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS plugin_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                plugin_id TEXT NOT NULL,
                count INTEGER DEFAULT 1,
                last_used TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_plugin_usage_plugin ON plugin_usage(plugin_id)",
            [],
        )?;
        
        tracing::info!("Statistics database initialized");
        
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
        })
    }
    
    /// 获取数据库路径
    fn get_db_path() -> Result<PathBuf> {
        let app_data = directories::ProjectDirs::from("", "", "iLauncher")
            .ok_or_else(|| anyhow::anyhow!("Failed to get app data directory"))?;
        
        let data_dir = app_data.data_dir();
        std::fs::create_dir_all(data_dir)?;
        
        Ok(data_dir.join("statistics.db"))
    }
    
    /// 记录查询
    pub async fn record_query(&self, query: &str) -> Result<()> {
        let query = query.to_string();
        let db = self.db.clone();
        
        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let now = Utc::now().to_rfc3339();
            
            // 查找是否已存在
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM queries WHERE query = ?1)",
                params![&query],
                |row| row.get(0),
            )?;
            
            if exists {
                // 更新计数和时间
                conn.execute(
                    "UPDATE queries SET count = count + 1, last_used = ?1 WHERE query = ?2",
                    params![&now, &query],
                )?;
            } else {
                // 插入新记录
                conn.execute(
                    "INSERT INTO queries (query, count, last_used, created_at) VALUES (?1, 1, ?2, ?2)",
                    params![&query, &now],
                )?;
            }
            
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        
        Ok(())
    }
    
    /// 记录结果点击
    pub async fn record_result_click(&self, result_id: &str, plugin_id: &str, title: &str) -> Result<()> {
        let result_id = result_id.to_string();
        let plugin_id = plugin_id.to_string();
        let title = title.to_string();
        let db = self.db.clone();
        
        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let now = Utc::now().to_rfc3339();
            
            // 查找是否已存在
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM result_clicks WHERE result_id = ?1 AND plugin_id = ?2)",
                params![&result_id, &plugin_id],
                |row| row.get(0),
            )?;
            
            if exists {
                // 更新计数和时间
                conn.execute(
                    "UPDATE result_clicks SET count = count + 1, last_used = ?1, title = ?2 WHERE result_id = ?3 AND plugin_id = ?4",
                    params![&now, &title, &result_id, &plugin_id],
                )?;
            } else {
                // 插入新记录
                conn.execute(
                    "INSERT INTO result_clicks (result_id, plugin_id, title, count, last_used, created_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)",
                    params![&result_id, &plugin_id, &title, &now],
                )?;
            }
            
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        
        Ok(())
    }
    
    /// 记录插件使用
    pub async fn record_plugin_usage(&self, plugin_id: &str) -> Result<()> {
        let plugin_id = plugin_id.to_string();
        let db = self.db.clone();
        
        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let now = Utc::now().to_rfc3339();
            
            // 查找是否已存在
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM plugin_usage WHERE plugin_id = ?1)",
                params![&plugin_id],
                |row| row.get(0),
            )?;
            
            if exists {
                // 更新计数和时间
                conn.execute(
                    "UPDATE plugin_usage SET count = count + 1, last_used = ?1 WHERE plugin_id = ?2",
                    params![&now, &plugin_id],
                )?;
            } else {
                // 插入新记录
                conn.execute(
                    "INSERT INTO plugin_usage (plugin_id, count, last_used, created_at) VALUES (?1, 1, ?2, ?2)",
                    params![&plugin_id, &now],
                )?;
            }
            
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        
        Ok(())
    }
    
    /// 获取结果的使用次数
    pub async fn get_result_score(&self, result_id: &str, plugin_id: &str) -> Result<i32> {
        let result_id = result_id.to_string();
        let plugin_id = plugin_id.to_string();
        let db = self.db.clone();
        
        let count = tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            
            let count: Option<i32> = conn.query_row(
                "SELECT count FROM result_clicks WHERE result_id = ?1 AND plugin_id = ?2",
                params![&result_id, &plugin_id],
                |row| row.get(0),
            ).ok();
            
            Ok::<i32, anyhow::Error>(count.unwrap_or(0))
        })
        .await??;
        
        Ok(count)
    }
    
    /// 获取热门查询
    pub async fn get_top_queries(&self, limit: usize) -> Result<Vec<QueryStat>> {
        let db = self.db.clone();
        
        let queries = tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let mut stmt = conn.prepare(
                "SELECT query, count, last_used FROM queries ORDER BY count DESC, last_used DESC LIMIT ?1"
            )?;
            
            let rows = stmt.query_map(params![limit as i32], |row| {
                Ok(QueryStat {
                    query: row.get(0)?,
                    count: row.get(1)?,
                    last_used: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })?;
            
            let mut results = Vec::new();
            for row in rows {
                results.push(row?);
            }
            
            Ok::<Vec<QueryStat>, anyhow::Error>(results)
        })
        .await??;
        
        Ok(queries)
    }
    
    /// 获取热门结果
    pub async fn get_top_results(&self, limit: usize) -> Result<Vec<ResultStat>> {
        let db = self.db.clone();
        
        let results = tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let mut stmt = conn.prepare(
                "SELECT result_id, plugin_id, title, count, last_used FROM result_clicks ORDER BY count DESC, last_used DESC LIMIT ?1"
            )?;
            
            let rows = stmt.query_map(params![limit as i32], |row| {
                Ok(ResultStat {
                    result_id: row.get(0)?,
                    plugin_id: row.get(1)?,
                    title: row.get(2)?,
                    count: row.get(3)?,
                    last_used: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })?;
            
            let mut results = Vec::new();
            for row in rows {
                results.push(row?);
            }
            
            Ok::<Vec<ResultStat>, anyhow::Error>(results)
        })
        .await??;
        
        Ok(results)
    }
    
    /// 清除旧数据（保留最近90天）
    pub async fn cleanup_old_data(&self) -> Result<()> {
        let db = self.db.clone();
        
        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let cutoff = (Utc::now() - chrono::Duration::days(90)).to_rfc3339();
            
            conn.execute("DELETE FROM queries WHERE last_used < ?1", params![&cutoff])?;
            conn.execute("DELETE FROM result_clicks WHERE last_used < ?1", params![&cutoff])?;
            conn.execute("DELETE FROM plugin_usage WHERE last_used < ?1", params![&cutoff])?;
            
            // 压缩数据库
            conn.execute("VACUUM", [])?;
            
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        
        tracing::info!("Cleaned up old statistics data");
        Ok(())
    }
}
