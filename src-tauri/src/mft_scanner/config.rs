// 配置文件管理

use crate::mft_scanner::types::ScanConfig;
use anyhow::Result;
use std::path::Path;

const DEFAULT_CONFIG_PATH: &str = "scan_config.json";

/// 加载扫描配置
pub fn load_config() -> Result<ScanConfig> {
    let config_path = Path::new(DEFAULT_CONFIG_PATH);
    
    let mut config = if config_path.exists() {
        ScanConfig::load_from_file(DEFAULT_CONFIG_PATH)?
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
            config.save_to_file(DEFAULT_CONFIG_PATH)?;
        }
    }
    
    Ok(config)
}

/// 保存扫描配置
pub fn save_config(config: &ScanConfig) -> Result<()> {
    config.save_to_file(DEFAULT_CONFIG_PATH)
}
