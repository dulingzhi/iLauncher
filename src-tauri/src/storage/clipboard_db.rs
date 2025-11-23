// 剪贴板数据库持久化模块

use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct ClipboardRecord {
    pub id: i64,
    pub content_type: String, // text, image, rich_text
    pub content: String,       // 文本内容或图片base64
    pub plain_text: Option<String>, // 富文本的纯文本版本，用于搜索
    pub preview: Option<String>,    // 预览文本
    pub timestamp: DateTime<Local>,
    pub favorite: bool,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub file_path: Option<String>, // 图片文件路径（如果保存为文件）
}

pub struct ClipboardDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl ClipboardDatabase {
    /// 创建或打开数据库
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_tables()?;
        Ok(db)
    }

    /// 初始化数据库表
    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // 剪贴板历史表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS clipboard_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                content TEXT NOT NULL,
                plain_text TEXT,
                preview TEXT,
                timestamp INTEGER NOT NULL,
                favorite INTEGER DEFAULT 0,
                category TEXT,
                tags TEXT,
                file_path TEXT
            )",
            [],
        )?;

        // 创建索引优化查询
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_timestamp ON clipboard_history(timestamp DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_content_type ON clipboard_history(content_type)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_favorite ON clipboard_history(favorite)",
            [],
        )?;

        tracing::info!("Clipboard database tables initialized");
        Ok(())
    }

    /// 添加剪贴板记录
    pub fn add_record(
        &self,
        content_type: &str,
        content: &str,
        plain_text: Option<&str>,
        preview: Option<&str>,
        file_path: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();

        // 检查是否重复（最近10条记录中）
        let mut stmt = conn.prepare(
            "SELECT content FROM clipboard_history 
             ORDER BY timestamp DESC LIMIT 10",
        )?;

        let existing: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(Result::ok)
            .collect();

        if existing.contains(&content.to_string()) {
            return Err(anyhow::anyhow!("Duplicate content"));
        }

        // 插入新记录
        conn.execute(
            "INSERT INTO clipboard_history 
             (content_type, content, plain_text, preview, timestamp, file_path) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                content_type,
                content,
                plain_text,
                preview,
                Local::now().timestamp(),
                file_path,
            ],
        )?;

        let id = conn.last_insert_rowid();
        tracing::debug!("Added clipboard record: id={}, type={}", id, content_type);
        Ok(id)
    }

    /// 获取剪贴板历史（分页）
    pub fn get_history(
        &self,
        limit: usize,
        offset: usize,
        content_type_filter: Option<&str>,
        favorite_only: bool,
    ) -> Result<Vec<ClipboardRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut query = String::from(
            "SELECT id, content_type, content, plain_text, preview, timestamp, 
                    favorite, category, tags, file_path 
             FROM clipboard_history WHERE 1=1",
        );

        if let Some(ct) = content_type_filter {
            query.push_str(&format!(" AND content_type = '{}'", ct));
        }

        if favorite_only {
            query.push_str(" AND favorite = 1");
        }

        query.push_str(" ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2");

        let mut stmt = conn.prepare(&query)?;
        let records = stmt
            .query_map(params![limit, offset], |row| {
                Ok(ClipboardRecord {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content: row.get(2)?,
                    plain_text: row.get(3)?,
                    preview: row.get(4)?,
                    timestamp: Local.timestamp_opt(row.get(5)?, 0).unwrap(),
                    favorite: row.get::<_, i32>(6)? == 1,
                    category: row.get(7)?,
                    tags: row
                        .get::<_, Option<String>>(8)?
                        .map(|s| s.split(',').map(|t| t.to_string()).collect())
                        .unwrap_or_default(),
                    file_path: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// 搜索剪贴板内容
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ClipboardRecord>> {
        let conn = self.conn.lock().unwrap();

        let search_pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            "SELECT id, content_type, content, plain_text, preview, timestamp, 
                    favorite, category, tags, file_path 
             FROM clipboard_history 
             WHERE content LIKE ?1 OR plain_text LIKE ?1 OR tags LIKE ?1
             ORDER BY timestamp DESC LIMIT ?2",
        )?;

        let records = stmt
            .query_map(params![search_pattern, limit], |row| {
                Ok(ClipboardRecord {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content: row.get(2)?,
                    plain_text: row.get(3)?,
                    preview: row.get(4)?,
                    timestamp: Local.timestamp_opt(row.get(5)?, 0).unwrap(),
                    favorite: row.get::<_, i32>(6)? == 1,
                    category: row.get(7)?,
                    tags: row
                        .get::<_, Option<String>>(8)?
                        .map(|s| s.split(',').map(|t| t.to_string()).collect())
                        .unwrap_or_default(),
                    file_path: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// 切换收藏状态
    pub fn toggle_favorite(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();

        // 获取当前状态
        let current: i32 = conn.query_row(
            "SELECT favorite FROM clipboard_history WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;

        let new_state = if current == 1 { 0 } else { 1 };

        conn.execute(
            "UPDATE clipboard_history SET favorite = ?1 WHERE id = ?2",
            params![new_state, id],
        )?;

        Ok(new_state == 1)
    }

    /// 设置分类
    pub fn set_category(&self, id: i64, category: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE clipboard_history SET category = ?1 WHERE id = ?2",
            params![category, id],
        )?;
        Ok(())
    }

    /// 添加标签
    pub fn add_tag(&self, id: i64, tag: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // 获取现有标签
        let existing_tags: Option<String> = conn.query_row(
            "SELECT tags FROM clipboard_history WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;

        let mut tags: Vec<String> = existing_tags
            .map(|s| s.split(',').map(|t| t.to_string()).collect())
            .unwrap_or_default();

        if !tags.contains(&tag.to_string()) {
            tags.push(tag.to_string());
            let tags_str = tags.join(",");

            conn.execute(
                "UPDATE clipboard_history SET tags = ?1 WHERE id = ?2",
                params![tags_str, id],
            )?;
        }

        Ok(())
    }

    /// 删除记录
    pub fn delete_record(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_history WHERE id = ?1", params![id])?;
        tracing::debug!("Deleted clipboard record: id={}", id);
        Ok(())
    }

    /// 清理旧记录（保留最近N条）
    pub fn cleanup_old_records(&self, keep_count: usize) -> Result<usize> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "DELETE FROM clipboard_history WHERE id NOT IN (
                SELECT id FROM clipboard_history 
                ORDER BY timestamp DESC LIMIT ?1
            )",
            params![keep_count],
        )?;

        let deleted = conn.changes();
        if deleted > 0 {
            tracing::info!("Cleaned up {} old clipboard records", deleted);
        }
        Ok(deleted as usize)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> Result<(usize, usize, usize, usize)> {
        let conn = self.conn.lock().unwrap();

        let total: usize = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_history",
            [],
            |row| row.get(0),
        )?;

        let favorites: usize = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_history WHERE favorite = 1",
            [],
            |row| row.get(0),
        )?;

        let text_count: usize = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_history WHERE content_type = 'text'",
            [],
            |row| row.get(0),
        )?;

        let image_count: usize = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_history WHERE content_type = 'image'",
            [],
            |row| row.get(0),
        )?;

        Ok((total, favorites, text_count, image_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_clipboard_database() -> Result<()> {
        use uuid::Uuid;
        let db_path = PathBuf::from(format!("test_clipboard_{}.db", Uuid::new_v4()));
        
        // 清理测试数据库
        let _ = fs::remove_file(&db_path);

        let db = ClipboardDatabase::new(db_path.clone())?;

        // 测试添加记录
        let id1 = db.add_record("text", "Hello World", None, Some("Hello..."), None)?;
        assert!(id1 > 0);

        let id2 = db.add_record("image", "base64data", None, Some("Image"), Some("/path/to/img.png"))?;
        assert!(id2 > 0);

        // 测试获取历史
        let history = db.get_history(10, 0, None, false)?;
        assert_eq!(history.len(), 2);

        // 测试搜索
        let results = db.search("Hello", 10)?;
        assert_eq!(results.len(), 1);

        // 测试收藏
        let is_fav = db.toggle_favorite(id1)?;
        assert!(is_fav);

        // 测试分类和标签
        db.set_category(id1, Some("work"))?;
        db.add_tag(id1, "important")?;

        // 测试统计
        let (total, favorites, text_count, image_count) = db.get_stats()?;
        assert_eq!(total, 2);
        assert_eq!(favorites, 1);
        assert_eq!(text_count, 1);
        assert_eq!(image_count, 1);

        // 清理
        fs::remove_file(&db_path)?;
        Ok(())
    }
}
