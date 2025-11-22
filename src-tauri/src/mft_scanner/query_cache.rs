// MFT 查询缓存管理器
// 避免每次查询重新加载索引(60-70ms) -> 直接使用缓存(<1ms)

use anyhow::Result;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use super::{IndexQuery, PathReader};

/// 全局查询缓存管理器
pub static QUERY_CACHE: Lazy<QueryCacheManager> = Lazy::new(QueryCacheManager::new);

/// 缓存的查询器和路径读取器
pub struct CachedQuery {
    pub query: Arc<IndexQuery>,
    pub path_reader: Arc<PathReader>,
}

/// 查询缓存管理器
pub struct QueryCacheManager {
    cache: RwLock<HashMap<char, CachedQuery>>,
    output_dir: String,
}

impl QueryCacheManager {
    pub fn new() -> Self {
        // 从环境或默认路径获取输出目录
        let output_dir = std::env::var("MFT_INDEX_DIR")
            .unwrap_or_else(|_| {
                let local_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
                format!("{}\\iLauncher\\mft_databases", local_data)
            });
        
        Self {
            cache: RwLock::new(HashMap::new()),
            output_dir,
        }
    }
    
    /// 获取或创建查询器(带缓存)
    pub fn get_query(&self, drive_letter: char) -> Result<Arc<IndexQuery>> {
        // 尝试从缓存读取
        {
            let cache = self.cache.read().unwrap();
            if let Some(cached) = cache.get(&drive_letter) {
                // 检查是否需要重新加载
                if !cached.query.needs_reload() {
                    tracing::trace!("✓ Using cached IndexQuery for drive {}", drive_letter);
                    return Ok(Arc::clone(&cached.query));
                }
                tracing::debug!("🔄 Index version changed for drive {}, reloading...", drive_letter);
            }
        }
        
        // 需要重新加载或首次加载
        let mut cache = self.cache.write().unwrap();
        
        // Double-check (避免并发重复加载)
        if let Some(cached) = cache.get(&drive_letter) {
            if !cached.query.needs_reload() {
                return Ok(Arc::clone(&cached.query));
            }
        }
        
        tracing::info!("📥 Loading IndexQuery for drive {} (not in cache)", drive_letter);
        let start = std::time::Instant::now();
        
        let query = IndexQuery::open(drive_letter, &self.output_dir)?;
        let path_reader = PathReader::open(drive_letter, &self.output_dir)?;
        
        let elapsed = start.elapsed();
        tracing::info!(
            "✓ IndexQuery loaded for drive {} in {:.2}ms",
            drive_letter,
            elapsed.as_secs_f64() * 1000.0
        );
        
        let cached = CachedQuery {
            query: Arc::new(query),
            path_reader: Arc::new(path_reader),
        };
        
        let query_arc = Arc::clone(&cached.query);
        cache.insert(drive_letter, cached);
        
        Ok(query_arc)
    }
    
    /// 获取或创建路径读取器(带缓存)
    pub fn get_path_reader(&self, drive_letter: char) -> Result<Arc<PathReader>> {
        // 尝试从缓存读取
        {
            let cache = self.cache.read().unwrap();
            if let Some(cached) = cache.get(&drive_letter) {
                tracing::trace!("✓ Using cached PathReader for drive {}", drive_letter);
                return Ok(Arc::clone(&cached.path_reader));
            }
        }
        
        // 需要加载
        let mut cache = self.cache.write().unwrap();
        
        // Double-check
        if let Some(cached) = cache.get(&drive_letter) {
            return Ok(Arc::clone(&cached.path_reader));
        }
        
        tracing::info!("📥 Loading PathReader for drive {} (not in cache)", drive_letter);
        let start = std::time::Instant::now();
        
        let query = IndexQuery::open(drive_letter, &self.output_dir)?;
        let path_reader = PathReader::open(drive_letter, &self.output_dir)?;
        
        let elapsed = start.elapsed();
        tracing::info!(
            "✓ PathReader loaded for drive {} in {:.2}ms",
            drive_letter,
            elapsed.as_secs_f64() * 1000.0
        );
        
        let cached = CachedQuery {
            query: Arc::new(query),
            path_reader: Arc::new(path_reader),
        };
        
        let path_reader_arc = Arc::clone(&cached.path_reader);
        cache.insert(drive_letter, cached);
        
        Ok(path_reader_arc)
    }
    
    /// 获取查询器和路径读取器(带缓存)
    pub fn get_both(&self, drive_letter: char) -> Result<(Arc<IndexQuery>, Arc<PathReader>)> {
        // 尝试从缓存读取
        {
            let cache = self.cache.read().unwrap();
            if let Some(cached) = cache.get(&drive_letter) {
                if !cached.query.needs_reload() {
                    tracing::trace!("✓ Using cached Query+PathReader for drive {}", drive_letter);
                    return Ok((Arc::clone(&cached.query), Arc::clone(&cached.path_reader)));
                }
            }
        }
        
        // 需要加载
        let mut cache = self.cache.write().unwrap();
        
        // Double-check
        if let Some(cached) = cache.get(&drive_letter) {
            if !cached.query.needs_reload() {
                return Ok((Arc::clone(&cached.query), Arc::clone(&cached.path_reader)));
            }
        }
        
        tracing::info!("📥 Loading Query+PathReader for drive {} (not in cache or needs reload)", drive_letter);
        let start = std::time::Instant::now();
        
        let query = IndexQuery::open(drive_letter, &self.output_dir)?;
        let path_reader = PathReader::open(drive_letter, &self.output_dir)?;
        
        let elapsed = start.elapsed();
        tracing::info!(
            "✓ Query+PathReader loaded for drive {} in {:.2}ms",
            drive_letter,
            elapsed.as_secs_f64() * 1000.0
        );
        
        let cached = CachedQuery {
            query: Arc::new(query),
            path_reader: Arc::new(path_reader),
        };
        
        let query_arc = Arc::clone(&cached.query);
        let path_reader_arc = Arc::clone(&cached.path_reader);
        
        cache.insert(drive_letter, cached);
        
        Ok((query_arc, path_reader_arc))
    }
    
    /// 清除指定驱动器的缓存
    pub fn clear_drive(&self, drive_letter: char) {
        let mut cache = self.cache.write().unwrap();
        cache.remove(&drive_letter);
        tracing::info!("🗑️  Cleared cache for drive {}", drive_letter);
    }
    
    /// 清除所有缓存
    pub fn clear_all(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
        tracing::info!("🗑️  Cleared all query cache");
    }
    
    /// 预热缓存(提前加载常用驱动器)
    pub fn warmup(&self, drives: &[char]) {
        for &drive in drives {
            if let Err(e) = self.get_both(drive) {
                tracing::warn!("⚠️  Failed to warmup cache for drive {}: {}", drive, e);
            }
        }
    }
    
    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.read().unwrap();
        CacheStats {
            cached_drives: cache.keys().copied().collect(),
            total_cached: cache.len(),
        }
    }
}

/// 缓存统计信息
#[derive(Debug)]
pub struct CacheStats {
    pub cached_drives: Vec<char>,
    pub total_cached: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cache_manager() {
        let manager = QueryCacheManager::new();
        
        // 测试获取查询器
        if let Ok(_query) = manager.get_query('C') {
            // 第二次应该命中缓存
            let start = std::time::Instant::now();
            let _query2 = manager.get_query('C').unwrap();
            let elapsed = start.elapsed();
            
            println!("Cache hit time: {:?}", elapsed);
            assert!(elapsed.as_millis() < 5, "Cache should be very fast");
        }
        
        // 测试统计信息
        let stats = manager.stats();
        println!("Cache stats: {:?}", stats);
    }
}
