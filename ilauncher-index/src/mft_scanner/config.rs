// 配置文件管理

use crate::{mft_scanner::types::ScanConfig, paths::get_app_data_dir};
use anyhow::Result;

const DEFAULT_CONFIG_PATH: &str = "scan_config.json";

/// 加载扫描配置
pub fn load_config() -> Result<ScanConfig> {
    let app_dir = get_app_data_dir()?;
    let config_dir = app_dir.join("config");
    let config_path = config_dir.join(DEFAULT_CONFIG_PATH);

    let mut config = if config_path.exists() {
        ScanConfig::load_from_file(config_path.to_str().unwrap())?
    } else {
        ScanConfig::default()
    };
    
    // 自动检测并更新驱动器列表（如果检测到新的 NTFS 驱动器）
    #[cfg(target_os = "windows")]
    {
        let detected_drives = ScanConfig::detect_ntfs_drives();
        
        // 如果检测到的驱动器比配置中的多，或者配置为空，则更新
        if config.drives.is_empty() || detected_drives.len() > config.drives.len() {
            tracing::info!("🔍 Auto-detected NTFS drives: {:?}", detected_drives);
            tracing::info!("📝 Updating config with new drives (old: {:?})", config.drives);
            config.drives = detected_drives;
            config.save_to_file(config_path.to_str().unwrap())?;
        }
    }
    
    Ok(config)
}

/// 保存扫描配置
pub fn save_config(config: &ScanConfig) -> Result<()> {
    let app_dir = get_app_data_dir()?;
    let config_dir = app_dir.join("config");
    let config_path = config_dir.join(DEFAULT_CONFIG_PATH);
    config.save_to_file(config_path.to_str().unwrap())
}
