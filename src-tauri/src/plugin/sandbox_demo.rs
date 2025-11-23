// 沙盒演示插件
// 展示如何在插件中使用沙盒权限检查

use crate::plugin::Plugin;
use crate::plugin::sandbox::{PluginPermission, NetworkScope};
use crate::core::types::{PluginMetadata, QueryContext, QueryResult, Action, WoxImage};
use anyhow::Result;
use std::sync::Arc;

pub struct SandboxDemoPlugin {
    metadata: PluginMetadata,
    sandbox_manager: Arc<crate::plugin::sandbox::SandboxManager>,
}

impl SandboxDemoPlugin {
    pub fn new(sandbox_manager: Arc<crate::plugin::sandbox::SandboxManager>) -> Self {
        Self {
            metadata: PluginMetadata {
                id: "sandbox_demo".to_string(),
                name: "沙盒演示".to_string(),
                description: "演示插件沙盒隔离功能".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                trigger_keywords: vec!["sandbox".to_string(), "沙盒".to_string()],
                icon: WoxImage::emoji("🔒".to_string()),
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: crate::core::types::PluginType::Native,
            },
            sandbox_manager,
        }
    }

    /// 尝试读取文件（需要权限检查）
    async fn try_read_file(&self, path: &str) -> Result<String> {
        // 检查文件读取权限
        let permission = PluginPermission::FileSystemRead(std::path::PathBuf::from(path));
        self.sandbox_manager.check_permission(&self.metadata.id, &permission)?;
        
        // 如果有权限，执行操作
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(format!("✅ 文件读取成功: {} 字节", content.len())),
            Err(e) => Ok(format!("❌ 文件读取失败: {}", e)),
        }
    }

    /// 尝试网络访问（需要权限检查）
    async fn try_network_access(&self, domain: &str) -> Result<String> {
        // 检查网络访问权限
        let permission = PluginPermission::NetworkAccess(NetworkScope::Domain(domain.to_string()));
        self.sandbox_manager.check_permission(&self.metadata.id, &permission)?;
        
        Ok(format!("✅ 网络访问权限验证通过: {}", domain))
    }

    /// 尝试执行程序（需要权限检查）
    async fn try_execute_program(&self, program: &str) -> Result<String> {
        // 检查程序执行权限
        self.sandbox_manager.check_permission(&self.metadata.id, &PluginPermission::ExecuteProgram)?;
        
        Ok(format!("✅ 程序执行权限验证通过: {}", program))
    }
}

#[async_trait::async_trait]
impl Plugin for SandboxDemoPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query_lower = ctx.search.to_lowercase();
        let mut results = Vec::new();

        if !query_lower.starts_with("sandbox") && !query_lower.starts_with("沙盒") {
            return Ok(results);
        }

        // 测试各种权限
        let tests = vec![
            ("file_read", "测试文件读取", "尝试读取配置文件"),
            ("network", "测试网络访问", "尝试访问 api.example.com"),
            ("execute", "测试程序执行", "尝试执行外部程序"),
            ("clipboard", "测试剪贴板访问", "尝试访问系统剪贴板"),
        ];

        for (action_id, title, subtitle) in tests {
            results.push(
                QueryResult::new(title.to_string())
                    .with_subtitle(subtitle.to_string())
                    .with_icon(WoxImage::emoji("🔒".to_string()))
                    .with_action(Action::new(action_id.to_string()).default())
            );
        }

        Ok(results)
    }

    async fn execute(&self, _result_id: &str, action_id: &str) -> Result<()> {
        let result_msg = match action_id {
            "file_read" => {
                match self.try_read_file("config.json").await {
                    Ok(msg) => msg,
                    Err(e) => format!("❌ 权限被拒绝: {}", e),
                }
            }
            "network" => {
                match self.try_network_access("api.example.com").await {
                    Ok(msg) => msg,
                    Err(e) => format!("❌ 权限被拒绝: {}", e),
                }
            }
            "execute" => {
                match self.try_execute_program("notepad.exe").await {
                    Ok(msg) => msg,
                    Err(e) => format!("❌ 权限被拒绝: {}", e),
                }
            }
            "clipboard" => {
                match self.sandbox_manager.check_permission(
                    &self.metadata.id,
                    &PluginPermission::ClipboardAccess,
                ) {
                    Ok(_) => "✅ 剪贴板访问权限验证通过".to_string(),
                    Err(e) => format!("❌ 权限被拒绝: {}", e),
                }
            }
            _ => "未知操作".to_string(),
        };

        tracing::info!("🔒 Sandbox Demo: {}", result_msg);
        Ok(())
    }
}
