// 进程管理器插件

use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use sysinfo::System;

pub struct ProcessPlugin {
    metadata: PluginMetadata,
}

impl ProcessPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "process".to_string(),
                name: "进程管理器".to_string(),
                description: "搜索和管理系统进程".to_string(),
                icon: WoxImage::Emoji("⚙️".to_string()),
                version: "1.0.0".to_string(),
                author: "iLauncher".to_string(),
                trigger_keywords: vec!["ps".to_string(), "kill".to_string()],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string()],
                plugin_type: PluginType::Native,
            },
        }
    }
}

#[async_trait]
impl crate::plugin::Plugin for ProcessPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim();
        
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // 检查触发词
        let (search_term, is_kill_mode) = if query.starts_with("kill ") {
            (&query[5..], true)
        } else if query.starts_with("ps ") {
            (&query[3..], false)
        } else {
            (query, false)
        };

        if search_term.is_empty() {
            return Ok(Vec::new());
        }

        // 创建系统信息对象并刷新进程
        let mut sys = System::new_all();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All);

        let matcher = SkimMatcherV2::default();
        let mut results = Vec::new();

        for (pid, process) in sys.processes() {
            let process_name = process.name().to_string_lossy().to_string();
            let exe_path = process.exe().map(|p| p.display().to_string()).unwrap_or_default();
            
            // 匹配进程名或路径
            let name_score = matcher.fuzzy_match(&process_name, search_term).unwrap_or(0);
            let path_score = matcher.fuzzy_match(&exe_path, search_term).unwrap_or(0);
            let score = name_score.max(path_score);

            if score > 20 {
                let memory_mb = process.memory() / 1024 / 1024;
                let cpu_usage = process.cpu_usage();
                
                let subtitle = if exe_path.is_empty() {
                    format!("PID: {} | 内存: {} MB | CPU: {:.1}%", pid, memory_mb, cpu_usage)
                } else {
                    format!("PID: {} | 内存: {} MB | CPU: {:.1}% | {}", pid, memory_mb, cpu_usage, exe_path)
                };

                let mut actions = vec![
                    Action {
                        id: "kill".to_string(),
                        name: "结束进程".to_string(),
                        icon: None,
                        is_default: is_kill_mode,
                        hotkey: None,
                        prevent_hide: false,
                    },
                    Action {
                        id: "open_location".to_string(),
                        name: "打开文件位置".to_string(),
                        icon: None,
                        is_default: false,
                        hotkey: None,
                        prevent_hide: false,
                    },
                ];

                if !is_kill_mode {
                    actions.insert(0, Action {
                        id: "info".to_string(),
                        name: "详细信息".to_string(),
                        icon: None,
                        is_default: true,
                        hotkey: None,
                        prevent_hide: true,
                    });
                }

                results.push(QueryResult {
                    id: pid.to_string(),
                    plugin_id: self.metadata.id.clone(),
                    title: process_name.clone(),
                    subtitle,
                    icon: WoxImage::emoji("⚙️".to_string()),
                    score: score as i32,
                    context_data: serde_json::json!({
                        "pid": pid.as_u32(),
                        "name": process_name,
                        "path": exe_path,
                        "memory": memory_mb,
                        "cpu": cpu_usage,
                    }),
                    actions,
                    preview: None,
                    refreshable: false,
                    group: None,
                });
            }
        }

        // 按分数排序，限制结果数量
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(20);

        Ok(results)
    }

    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        let pid: u32 = result_id.parse()?;
        
        match action_id {
            "kill" => {
                #[cfg(target_os = "windows")]
                {
                    use std::process::Command;
                    let output = Command::new("taskkill")
                        .args(&["/F", "/PID", &pid.to_string()])
                        .output()?;
                    
                    if output.status.success() {
                        tracing::info!("Successfully killed process {}", pid);
                        Ok(())
                    } else {
                        let error = String::from_utf8_lossy(&output.stderr);
                        Err(anyhow::anyhow!("Failed to kill process: {}", error))
                    }
                }
                
                #[cfg(not(target_os = "windows"))]
                {
                    Err(anyhow::anyhow!("Process killing not supported on this OS"))
                }
            }
            "open_location" => {
                // 获取进程路径并打开所在目录
                let mut sys = System::new();
                sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
                
                if let Some(process) = sys.process(sysinfo::Pid::from_u32(pid)) {
                    if let Some(exe) = process.exe() {
                        if let Some(parent) = exe.parent() {
                            #[cfg(target_os = "windows")]
                            {
                                std::process::Command::new("explorer")
                                    .arg(parent)
                                    .spawn()?;
                            }
                            
                            return Ok(());
                        }
                    }
                }
                
                Err(anyhow::anyhow!("Cannot find process executable path"))
            }
            "info" => {
                // 详细信息会在界面上展示，这里不需要执行操作
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unknown action: {}", action_id)),
        }
    }
}
