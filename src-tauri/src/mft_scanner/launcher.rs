// 扫描器进程启动器 - UAC 提权

use anyhow::{Result, Context};
use std::env;
use std::process::{Command, Stdio};
use tracing::info;

pub struct ScannerLauncher;

impl ScannerLauncher {
    /// 启动 MFT 扫描器进程（以管理员权限）
    pub fn launch() -> Result<()> {
        info!("🚀 Launching MFT Scanner with admin rights...");
        
        // 获取当前程序路径
        let exe_path = env::current_exe()
            .context("Failed to get current executable path")?;
        
        // 使用 PowerShell Start-Process -Verb RunAs 来请求管理员权限
        let ps_command = format!(
            "Start-Process -FilePath '{}' -ArgumentList '--mft-scanner' -Verb RunAs",
            exe_path.display()
        );
        
        let output = Command::new("powershell.exe")
            .args(["-WindowStyle", "Hidden", "-Command", &ps_command])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to launch scanner process")?;
        
        info!("✅ Scanner process launched (PID: {})", output.id());
        
        Ok(())
    }
    
    /// 检查扫描器进程是否正在运行
    pub fn is_running() -> bool {
        use super::ipc::ScannerClient;
        
        // 尝试连接到扫描器
        match ScannerClient::connect() {
            Ok(mut client) => {
                // 发送 ping 测试
                client.ping().is_ok()
            }
            Err(_) => false,
        }
    }
}
