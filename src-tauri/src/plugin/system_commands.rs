// 系统命令插件 - Windows系统操作快捷方式

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use std::process::Command;

pub struct SystemCommandPlugin {
    metadata: PluginMetadata,
    commands: Vec<SystemCommand>,
}

#[derive(Clone)]
struct SystemCommand {
    #[allow(dead_code)]
    id: String,
    name: String,
    description: String,
    icon: String,
    keywords: Vec<String>,
    command_type: CommandType,
}

#[derive(Clone)]
enum CommandType {
    Shutdown,
    Restart,
    Sleep,
    Hibernate,
    Lock,
    SignOut,
    EmptyRecycleBin,
}

impl SystemCommandPlugin {
    pub fn new() -> Self {
        let commands = vec![
            SystemCommand {
                id: "shutdown".to_string(),
                name: "关机".to_string(),
                description: "立即关闭计算机".to_string(),
                icon: "🔴".to_string(),
                keywords: vec!["shutdown".to_string(), "关机".to_string(), "guanji".to_string()],
                command_type: CommandType::Shutdown,
            },
            SystemCommand {
                id: "restart".to_string(),
                name: "重启".to_string(),
                description: "重新启动计算机".to_string(),
                icon: "🔄".to_string(),
                keywords: vec!["restart".to_string(), "reboot".to_string(), "重启".to_string(), "chongqi".to_string()],
                command_type: CommandType::Restart,
            },
            SystemCommand {
                id: "sleep".to_string(),
                name: "睡眠".to_string(),
                description: "使计算机进入睡眠模式".to_string(),
                icon: "💤".to_string(),
                keywords: vec!["sleep".to_string(), "睡眠".to_string(), "shuimian".to_string()],
                command_type: CommandType::Sleep,
            },
            SystemCommand {
                id: "hibernate".to_string(),
                name: "休眠".to_string(),
                description: "使计算机进入休眠状态".to_string(),
                icon: "🌙".to_string(),
                keywords: vec!["hibernate".to_string(), "休眠".to_string(), "xiumian".to_string()],
                command_type: CommandType::Hibernate,
            },
            SystemCommand {
                id: "lock".to_string(),
                name: "锁定".to_string(),
                description: "锁定计算机屏幕".to_string(),
                icon: "🔒".to_string(),
                keywords: vec!["lock".to_string(), "锁定".to_string(), "suoding".to_string()],
                command_type: CommandType::Lock,
            },
            SystemCommand {
                id: "signout".to_string(),
                name: "注销".to_string(),
                description: "注销当前用户".to_string(),
                icon: "👤".to_string(),
                keywords: vec!["signout".to_string(), "logout".to_string(), "注销".to_string(), "zhuxiao".to_string()],
                command_type: CommandType::SignOut,
            },
            SystemCommand {
                id: "empty-recycle-bin".to_string(),
                name: "清空回收站".to_string(),
                description: "永久删除回收站中的所有文件".to_string(),
                icon: "🗑️".to_string(),
                keywords: vec!["empty".to_string(), "recycle".to_string(), "清空".to_string(), "回收站".to_string(), "qingkong".to_string()],
                command_type: CommandType::EmptyRecycleBin,
            },
        ];
        
        Self {
            metadata: PluginMetadata {
                id: "system-commands".to_string(),
                name: "系统命令".to_string(),
                description: "快速执行系统操作".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("⚙️"),
                trigger_keywords: vec!["sys".to_string(), "system".to_string()],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string()],
                plugin_type: PluginType::Native,
            },
            commands,
        }
    }
    
    /// 执行系统命令
    fn execute_system_command(&self, cmd_type: &CommandType) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            match cmd_type {
                CommandType::Shutdown => {
                    Command::new("shutdown")
                        .args(["/s", "/t", "0"])
                        .spawn()?;
                }
                CommandType::Restart => {
                    Command::new("shutdown")
                        .args(["/r", "/t", "0"])
                        .spawn()?;
                }
                CommandType::Sleep => {
                    // 使用 rundll32 触发睡眠
                    Command::new("rundll32.exe")
                        .args(["powrprof.dll,SetSuspendState", "0,1,0"])
                        .spawn()?;
                }
                CommandType::Hibernate => {
                    Command::new("shutdown")
                        .args(["/h"])
                        .spawn()?;
                }
                CommandType::Lock => {
                    Command::new("rundll32.exe")
                        .args(["user32.dll,LockWorkStation"])
                        .spawn()?;
                }
                CommandType::SignOut => {
                    Command::new("shutdown")
                        .args(["/l"])
                        .spawn()?;
                }
                CommandType::EmptyRecycleBin => {
                    // 使用 PowerShell 清空回收站
                    Command::new("powershell")
                        .args(["-Command", "Clear-RecycleBin", "-Force"])
                        .spawn()?;
                }
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl Plugin for SystemCommandPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim().to_lowercase();
        
        if query.is_empty() {
            return Ok(vec![]);
        }
        
        let mut results = Vec::new();
        
        for cmd in &self.commands {
            // 检查关键词匹配
            let matches = cmd.keywords.iter().any(|kw| {
                let kw_lower = kw.to_lowercase();
                kw_lower.contains(&query) || query.contains(&kw_lower)
            });
            
            if matches {
                results.push(
                    QueryResult::new(cmd.name.clone())
                        .with_subtitle(cmd.description.clone())
                        .with_icon(WoxImage::emoji(&cmd.icon))
                        .with_score(800)
                        .with_action(Action::new("execute").default())
                );
            }
        }
        
        Ok(results)
    }
    
    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        if action_id != "execute" {
            return Err(anyhow::anyhow!("Unknown action"));
        }
        
        // 根据结果ID找到对应的命令
        if let Some(cmd) = self.commands.iter().find(|c| c.name == result_id) {
            tracing::info!("执行系统命令: {}", cmd.name);
            self.execute_system_command(&cmd.command_type)?;
        }
        
        Ok(())
    }
}
