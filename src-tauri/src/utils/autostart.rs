// 开机自启动管理模块
use anyhow::{Result, Context};
use auto_launch::AutoLaunch;

/// 获取 AutoLaunch 实例
fn get_auto_launch() -> Result<AutoLaunch> {
    let app_name = "iLauncher";
    let app_path = std::env::current_exe()
        .context("Failed to get current executable path")?;
    
    let auto_launch = AutoLaunch::new(
        app_name,
        &app_path.to_string_lossy(),
        &[] as &[&str], // 启动参数（空）
    );
    
    Ok(auto_launch)
}

/// 启用开机自启
pub fn enable() -> Result<()> {
    let auto_launch = get_auto_launch()?;
    
    // 检查是否已启用
    if auto_launch.is_enabled().unwrap_or(false) {
        tracing::info!("Auto-start is already enabled");
        return Ok(());
    }
    
    auto_launch.enable()
        .context("Failed to enable auto-start")?;
    
    tracing::info!("✓ Auto-start enabled successfully");
    Ok(())
}

/// 禁用开机自启
pub fn disable() -> Result<()> {
    let auto_launch = get_auto_launch()?;
    
    // 检查是否已禁用
    if !auto_launch.is_enabled().unwrap_or(false) {
        tracing::info!("Auto-start is already disabled");
        return Ok(());
    }
    
    auto_launch.disable()
        .context("Failed to disable auto-start")?;
    
    tracing::info!("✓ Auto-start disabled successfully");
    Ok(())
}

/// 检查是否已启用开机自启
pub fn is_enabled() -> Result<bool> {
    let auto_launch = get_auto_launch()?;
    auto_launch.is_enabled()
        .context("Failed to check auto-start status")
}

/// 根据配置同步开机自启状态
pub fn sync_with_config(should_enable: bool) -> Result<()> {
    let current_status = is_enabled().unwrap_or(false);
    
    if should_enable && !current_status {
        enable()?;
    } else if !should_enable && current_status {
        disable()?;
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_auto_launch_creation() {
        let result = get_auto_launch();
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_is_enabled() {
        let result = is_enabled();
        assert!(result.is_ok());
    }
}
