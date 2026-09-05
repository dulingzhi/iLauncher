// 新 crate 内的应用路径工具（从 src-tauri/src/utils/paths.rs 原样复制，
// 保持 ilauncher-index 无跨 crate 依赖；改动请同步两处）

use std::path::PathBuf;
use anyhow::{Context, Result};

pub fn get_app_data_dir() -> Result<PathBuf> {
    let local_appdata = std::env::var("LOCALAPPDATA")
        .context("Failed to get LOCALAPPDATA environment variable")?;

    let app_dir = PathBuf::from(local_appdata).join("iLauncher");

    if !app_dir.exists() {
        std::fs::create_dir_all(&app_dir)
            .context("Failed to create app data directory")?;
    }

    Ok(app_dir)
}
