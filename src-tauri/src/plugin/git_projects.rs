// Git 项目快速访问插件

use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(target_os = "windows")]
use crate::mft_scanner::index_builder::{IndexQuery, PathReader};

#[derive(Debug, Clone)]
struct GitProject {
    name: String,
    path: PathBuf,
}

pub struct GitProjectsPlugin {
    metadata: PluginMetadata,
    projects: Arc<RwLock<Vec<GitProject>>>,
}

impl GitProjectsPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "git".to_string(),
                name: "Git 项目".to_string(),
                description: "搜索和打开本地 Git 项目".to_string(),
                icon: WoxImage::Emoji("📦".to_string()),
                version: "1.0.0".to_string(),
                author: "iLauncher".to_string(),
                trigger_keywords: vec!["git".to_string(), "project".to_string()],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "linux".to_string(), "macos".to_string()],
                plugin_type: PluginType::Native,
            },
            projects: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn init(&self) {
        tracing::info!("Initializing Git projects plugin...");
        
        if let Err(e) = self.scan_projects().await {
            tracing::warn!("Failed to scan Git projects: {}", e);
        }
        
        let count = self.projects.read().await.len();
        tracing::info!("Found {} Git projects", count);
    }

    async fn scan_projects(&self) -> Result<()> {
        let projects;
        
        // 尝试使用 MFT 索引进行快速扫描
        #[cfg(target_os = "windows")]
        {
            match self.scan_projects_with_mft().await {
                Ok(mft_projects) => {
                    tracing::info!("✓ MFT scan found {} Git projects", mft_projects.len());
                    projects = mft_projects;
                }
                Err(e) => {
                    tracing::warn!("MFT scan failed ({}), falling back to walkdir", e);
                    projects = self.scan_projects_fallback().await?;
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            projects = self.scan_projects_fallback().await?;
        }

        // 去重（可能有符号链接导致重复）
        let mut projects = projects;
        projects.sort_by(|a, b| a.path.cmp(&b.path));
        projects.dedup_by(|a, b| a.path == b.path);

        *self.projects.write().await = projects;
        Ok(())
    }

    /// 使用 MFT 索引快速扫描 Git 项目（Windows）
    #[cfg(target_os = "windows")]
    async fn scan_projects_with_mft(&self) -> Result<Vec<GitProject>> {
        use crate::utils::paths;
        
        let output_dir = paths::get_mft_database_dir()?;
        let output_dir_str = output_dir.to_string_lossy().to_string();
        
        let mut all_projects = Vec::new();
        
        // 扫描所有驱动器
        for drive in b'C'..=b'Z' {
            let drive_char = drive as char;
            let drive_path = format!("{}:\\", drive_char);
            
            if !std::path::Path::new(&drive_path).exists() {
                continue;
            }
            
            // 检查索引文件是否存在
            let fst_file = format!("{}\\{}_index.fst", output_dir_str, drive_char);
            if !std::path::Path::new(&fst_file).exists() {
                tracing::debug!("No MFT index for drive {}, skipping", drive_char);
                continue;
            }
            
            tracing::info!("🔍 Scanning drive {} with MFT index...", drive_char);
            
            // 打开索引
            let query = IndexQuery::open(drive_char, &output_dir_str)?;
            let path_reader = PathReader::open(drive_char, &output_dir_str)?;
            
            // 搜索 ".git" 目录
            let file_ids = query.search(".git", 10000)?; // 限制最多 10000 个结果
            
            for file_id in file_ids {
                if let Ok(path_str) = path_reader.get_path(file_id) {
                    // 检查是否是目录（MFT 中目录路径以 \ 结尾）
                    if !path_str.ends_with("\\") {
                        continue;
                    }
                    
                    // 检查是否是 .git 目录（路径以 \.git\ 结尾）
                    if path_str.to_lowercase().ends_with("\\.git\\") {
                        // 获取项目路径（.git 的父目录）
                        let git_path = PathBuf::from(&path_str);
                        if let Some(project_path) = git_path.parent() {
                            let project_name = project_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown")
                                .to_string();
                            
                            all_projects.push(GitProject {
                                name: project_name,
                                path: project_path.to_path_buf(),
                            });
                            
                            tracing::debug!("Found Git project: {}", project_path.display());
                        }
                    }
                }
            }
            
            tracing::info!("✓ Drive {} scan complete", drive_char);
        }
        
        Ok(all_projects)
    }

    /// 回退方案：使用 walkdir 递归扫描（跨平台）
    async fn scan_projects_fallback(&self) -> Result<Vec<GitProject>> {
        use walkdir::WalkDir;
        
        let mut projects = Vec::new();
        
        // 扫描常用目录
        let scan_dirs = self.get_scan_directories();
        
        for base_dir in scan_dirs {
            if !base_dir.exists() {
                continue;
            }

            tracing::info!("Scanning for Git projects in: {:?}", base_dir);
            
            // 使用 WalkDir 递归扫描，但限制深度避免扫描太深
            for entry in WalkDir::new(&base_dir)
                .max_depth(4)  // 限制深度
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                
                // 检查是否是 .git 目录
                if path.is_dir() && path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                    if let Some(project_path) = path.parent() {
                        let project_name = project_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        
                        projects.push(GitProject {
                            name: project_name,
                            path: project_path.to_path_buf(),
                        });
                        
                        tracing::debug!("Found Git project: {}", project_path.display());
                    }
                }
            }
        }

        Ok(projects)
    }

    fn get_scan_directories(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // 获取用户目录
        if let Some(home) = dirs::home_dir() {
            // Documents/Projects
            dirs.push(home.join("Documents"));
            dirs.push(home.join("Projects"));
            dirs.push(home.join("Code"));
            dirs.push(home.join("dev"));
            dirs.push(home.join("workspace"));
            
            // OneDrive 同步目录
            dirs.push(home.join("OneDrive").join("Projects"));
            dirs.push(home.join("OneDrive").join("Code"));
        }

        // Windows 常用开发目录
        #[cfg(target_os = "windows")]
        {
            dirs.push(PathBuf::from("D:\\Projects"));
            dirs.push(PathBuf::from("E:\\Projects"));
            dirs.push(PathBuf::from("D:\\Code"));
            dirs.push(PathBuf::from("E:\\Code"));
            dirs.push(PathBuf::from("C:\\Projects"));
        }

        dirs
    }

    fn find_vscode_path(&self) -> Option<PathBuf> {
        // 尝试找到 VSCode 可执行文件
        #[cfg(target_os = "windows")]
        {
            let possible_paths = vec![
                PathBuf::from("C:\\Program Files\\Microsoft VS Code\\Code.exe"),
                PathBuf::from("C:\\Program Files (x86)\\Microsoft VS Code\\Code.exe"),
                dirs::home_dir()
                    .map(|h| h.join("AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe"))
                    .unwrap_or_default(),
            ];

            for path in possible_paths {
                if path.exists() {
                    return Some(path);
                }
            }
        }

        None
    }
}

#[async_trait]
impl crate::plugin::Plugin for GitProjectsPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim();
        
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // 检查触发词
        let search_term = if query.starts_with("git ") {
            &query[4..]
        } else if query.starts_with("project ") {
            &query[8..]
        } else {
            return Ok(Vec::new());
        };

        if search_term.is_empty() {
            // 显示所有项目（按字母排序）
            let projects = self.projects.read().await;
            let mut results: Vec<QueryResult> = projects
                .iter()
                .take(20)
                .map(|project| {
                    QueryResult {
                        id: project.path.display().to_string(),
                        plugin_id: self.metadata.id.clone(),
                        title: project.name.clone(),
                        subtitle: project.path.display().to_string(),
                        icon: WoxImage::emoji("📁".to_string()),
                        score: 50,
                        context_data: serde_json::json!({
                            "name": project.name,
                            "path": project.path.display().to_string(),
                        }),
                        actions: vec![
                            Action {
                                id: "open_vscode".to_string(),
                                name: "在 VSCode 中打开".to_string(),
                                icon: None,
                                is_default: true,
                                hotkey: None,
                                prevent_hide: false,
                            },
                            Action {
                                id: "open_explorer".to_string(),
                                name: "在文件管理器中打开".to_string(),
                                icon: None,
                                is_default: false,
                                hotkey: None,
                                prevent_hide: false,
                            },
                            Action {
                                id: "open_terminal".to_string(),
                                name: "在终端中打开".to_string(),
                                icon: None,
                                is_default: false,
                                hotkey: None,
                                prevent_hide: false,
                            },
                        ],
                        preview: None,
                        refreshable: false,
                        group: None,
                    }
                })
                .collect();

            results.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
            return Ok(results);
        }

        // 模糊搜索
        let matcher = SkimMatcherV2::default();
        let projects = self.projects.read().await;
        let mut results: Vec<(i64, GitProject)> = Vec::new();

        for project in projects.iter() {
            let name_score = matcher.fuzzy_match(&project.name, search_term).unwrap_or(0);
            let path_score = matcher.fuzzy_match(&project.path.display().to_string(), search_term).unwrap_or(0);
            let score = name_score.max(path_score);

            if score > 20 {
                results.push((score, project.clone()));
            }
        }

        // 按分数排序
        results.sort_by(|a, b| b.0.cmp(&a.0));
        results.truncate(20);

        let query_results: Vec<QueryResult> = results
            .into_iter()
            .map(|(score, project)| {
                QueryResult {
                    id: project.path.display().to_string(),
                    plugin_id: self.metadata.id.clone(),
                    title: project.name.clone(),
                    subtitle: project.path.display().to_string(),
                    icon: WoxImage::emoji("📦".to_string()),
                    score: score as i32,
                    context_data: serde_json::json!({
                        "name": project.name,
                        "path": project.path.display().to_string(),
                    }),
                    actions: vec![
                        Action {
                            id: "open_vscode".to_string(),
                            name: "在 VSCode 中打开".to_string(),
                            icon: None,
                            is_default: true,
                            hotkey: None,
                            prevent_hide: false,
                        },
                        Action {
                            id: "open_explorer".to_string(),
                            name: "在文件管理器中打开".to_string(),
                            icon: None,
                            is_default: false,
                            hotkey: None,
                            prevent_hide: false,
                        },
                        Action {
                            id: "open_terminal".to_string(),
                            name: "在终端中打开".to_string(),
                            icon: None,
                            is_default: false,
                            hotkey: None,
                            prevent_hide: false,
                        },
                    ],
                    preview: None,
                    refreshable: false,
                    group: None,
                }
            })
            .collect();

        Ok(query_results)
    }

    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        let project_path = PathBuf::from(result_id);

        match action_id {
            "open_vscode" => {
                // 尝试在 VSCode 中打开
                if let Some(vscode) = self.find_vscode_path() {
                    std::process::Command::new(vscode)
                        .arg(&project_path)
                        .spawn()?;
                } else {
                    // 如果找不到 VSCode，尝试使用 code 命令
                    #[cfg(target_os = "windows")]
                    {
                        std::process::Command::new("cmd")
                            .args(&["/C", "code", result_id])
                            .spawn()?;
                    }
                    
                    #[cfg(not(target_os = "windows"))]
                    {
                        std::process::Command::new("code")
                            .arg(&project_path)
                            .spawn()?;
                    }
                }
                
                tracing::info!("Opened project in VSCode: {}", result_id);
                Ok(())
            }
            "open_explorer" => {
                // 在文件管理器中打开
                #[cfg(target_os = "windows")]
                {
                    std::process::Command::new("explorer")
                        .arg(&project_path)
                        .spawn()?;
                }
                
                #[cfg(target_os = "macos")]
                {
                    std::process::Command::new("open")
                        .arg(&project_path)
                        .spawn()?;
                }
                
                #[cfg(target_os = "linux")]
                {
                    std::process::Command::new("xdg-open")
                        .arg(&project_path)
                        .spawn()?;
                }
                
                tracing::info!("Opened project in file manager: {}", result_id);
                Ok(())
            }
            "open_terminal" => {
                // 在终端中打开
                #[cfg(target_os = "windows")]
                {
                    std::process::Command::new("cmd")
                        .args(&["/C", "start", "cmd", "/K", "cd", "/D", result_id])
                        .spawn()?;
                }
                
                #[cfg(target_os = "macos")]
                {
                    std::process::Command::new("open")
                        .args(&["-a", "Terminal", result_id])
                        .spawn()?;
                }
                
                #[cfg(target_os = "linux")]
                {
                    std::process::Command::new("gnome-terminal")
                        .args(&["--working-directory", result_id])
                        .spawn()?;
                }
                
                tracing::info!("Opened project in terminal: {}", result_id);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unknown action: {}", action_id)),
        }
    }
}
