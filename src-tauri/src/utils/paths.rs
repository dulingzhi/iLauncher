// 统一的路径管理模块

use std::path::PathBuf;
use anyhow::{Result, Context};

/// 获取应用数据根目录 (AppData\Local\iLauncher)
pub fn get_app_data_dir() -> Result<PathBuf> {
    let local_appdata = std::env::var("LOCALAPPDATA")
        .context("Failed to get LOCALAPPDATA environment variable")?;
    
    let app_dir = PathBuf::from(local_appdata).join("iLauncher");
    
    // 确保目录存在
    if !app_dir.exists() {
        std::fs::create_dir_all(&app_dir)
            .context("Failed to create app data directory")?;
    }
    
    Ok(app_dir)
}

/// 获取 MFT 数据库目录 (AppData\Local\iLauncher\mft_databases)
pub fn get_mft_database_dir() -> Result<PathBuf> {
    let app_dir = get_app_data_dir()?;
    let mft_dir = app_dir.join("mft_databases");
    
    if !mft_dir.exists() {
        std::fs::create_dir_all(&mft_dir)
            .context("Failed to create MFT database directory")?;
    }
    
    Ok(mft_dir)
}

/// 获取日志文件目录 (AppData\Local\iLauncher\logs)
pub fn get_log_dir() -> Result<PathBuf> {
    let app_dir = get_app_data_dir()?;
    let log_dir = app_dir.join("logs");
    
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir)
            .context("Failed to create log directory")?;
    }
    
    Ok(log_dir)
}

/// 获取缓存目录 (AppData\Local\iLauncher\cache)
pub fn get_cache_dir() -> Result<PathBuf> {
    let app_dir = get_app_data_dir()?;
    let cache_dir = app_dir.join("cache");
    
    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir)
            .context("Failed to create cache directory")?;
    }
    
    Ok(cache_dir)
}

/// 获取配置目录 (AppData\Local\iLauncher\config)
pub fn get_config_dir() -> Result<PathBuf> {
    let app_dir = get_app_data_dir()?;
    let config_dir = app_dir.join("config");
    
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)
            .context("Failed to create config directory")?;
    }
    
    Ok(config_dir)
}

/// 获取数据目录 (AppData\Local\iLauncher\data)
pub fn get_data_dir() -> Result<PathBuf> {
    let app_dir = get_app_data_dir()?;
    let data_dir = app_dir.join("data");
    
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir)
            .context("Failed to create data directory")?;
    }
    
    Ok(data_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_data_dir() {
        let dir = get_app_data_dir().unwrap();
        println!("App data dir: {:?}", dir);
        assert!(dir.to_str().unwrap().contains("iLauncher"));
    }

    #[test]
    fn test_mft_database_dir() {
        let dir = get_mft_database_dir().unwrap();
        println!("MFT database dir: {:?}", dir);
        assert!(dir.exists());
    }
}
