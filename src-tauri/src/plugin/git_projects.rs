// Git 项目快速访问插件

use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct GitProject {
    name: String,
    path: PathBuf,
}

pub struct GitProjectsPlugin {
    metadata: PluginMetadata,
}

impl GitProjectsPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "git".to_string(),
                name: "Git 项目".to_string(),
                description: "搜索和打开本地 Git 项目（动态查询 MFT 索引）".to_string(),
                icon: WoxImage::Emoji("📦".to_string()),
                version: "1.0.0".to_string(),
                author: "iLauncher".to_string(),
                trigger_keywords: vec!["git".to_string(), "project".to_string()],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "linux".to_string(), "macos".to_string()],
                plugin_type: PluginType::Native,
            },
        }
    }

    pub async fn init(&self) {
        tracing::info!("Git projects plugin initialized (dynamic query mode - no cache)");
    }

    /// 动态查询 Git 项目（从 file_search 插件的 MFT 索引）
    async fn query_git_projects_dynamic(&self) -> Result<Vec<GitProject>> {
        #[cfg(target_os = "windows")]
        {
            use crate::utils::paths;
            use crate::mft_scanner::index_builder::{IndexQuery, PathReader};
            
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
                
                tracing::debug!("🔍 Querying MFT index for .git folders on drive {}", drive_char);
                
                // 打开索引
                let query = IndexQuery::open(drive_char, &output_dir_str)?;
                let path_reader = PathReader::open(drive_char, &output_dir_str)?;
                
                // 搜索包含 ".git" 的路径（MFT 3-gram 会匹配路径中的任何片段）
                let file_ids = query.search(".git", 10000)?;
                
                tracing::debug!("Found {} potential .git entries", file_ids.len());
                
                let mut checked = 0;
                let mut is_dir = 0;
                let mut is_git_dir = 0;
                
                for file_id in file_ids {
                    if let Ok(path_str) = path_reader.get_path(file_id) {
                        checked += 1;
                        
                        // 示例日志（仅前3个）
                        if checked <= 3 {
                            tracing::debug!("  Sample path: '{}'", path_str);
                        }
                        
                        // 检查是否是目录（MFT 中目录路径以 \ 结尾）
                        if path_str.ends_with("\\") {
                            is_dir += 1;
                            
                            // 检查是否是 .git 目录（路径以 \.git\ 结尾）
                            if path_str.to_lowercase().ends_with("\\.git\\") {
                                is_git_dir += 1;
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
                                }
                            }
                        }
                    }
                }
                
                tracing::debug!("  Checked: {}, IsDir: {}, IsGitDir: {}, Projects: {}", 
                    checked, is_dir, is_git_dir, all_projects.len());
            }
            
            tracing::info!("✓ MFT dynamic query found {} Git projects", all_projects.len());
            
            // 去重
            all_projects.sort_by(|a, b| a.path.cmp(&b.path));
            all_projects.dedup_by(|a, b| a.path == b.path);
            
            Ok(all_projects)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // 非 Windows 平台暂不支持动态查询
            Err(anyhow::anyhow!("Dynamic query not supported on non-Windows platforms"))
        }
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

        tracing::debug!("Git projects plugin queried with search_term: '{}'", search_term);

        // 每次动态查询 MFT 索引
        let projects = match self.query_git_projects_dynamic().await {
            Ok(scanned) => {
                tracing::debug!("Found {} Git projects from MFT", scanned.len());
                scanned
            }
            Err(e) => {
                tracing::error!("Failed to query Git projects: {}", e);
                return Ok(Vec::new());
            }
        };

        if search_term.is_empty() {
            // 显示所有项目（按字母排序）
            let projects = projects;
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
        let mut results: Vec<(i64, GitProject)> = Vec::new();

        tracing::debug!("Git projects search: searching '{}' in {} projects", search_term, projects.len());

        for project in projects.iter() {
            let name_lower = project.name.to_lowercase();
            let search_lower = search_term.to_lowercase();
            
            // 优先使用子串匹配，其次使用模糊匹配
            let score = if name_lower.contains(&search_lower) {
                // 子串匹配：根据匹配位置给分
                let pos = name_lower.find(&search_lower).unwrap();
                let s = if pos == 0 {
                    100 // 开头匹配得分最高
                } else {
                    80 - (pos as i64) // 位置越靠前分数越高
                };
                tracing::debug!("  - '{}' substring match at pos {}, score: {}", project.name, pos, s);
                s
            } else {
                // 模糊匹配
                let name_score = matcher.fuzzy_match(&project.name, search_term).unwrap_or(0);
                let path_score = matcher.fuzzy_match(&project.path.display().to_string(), search_term).unwrap_or(0);
                let s = name_score.max(path_score);
                if s > 0 {
                    tracing::debug!("  - '{}' fuzzy match, score: {}", project.name, s);
                }
                s
            };

            if score > 0 {
                results.push((score, project.clone()));
            }
        }

        tracing::info!("Git projects search '{}': found {} matches", search_term, results.len());

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
