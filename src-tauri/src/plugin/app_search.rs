// 应用搜索插件

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    pub name: String,
    pub path: PathBuf,
    pub icon_path: Option<PathBuf>,
}

pub struct AppSearchPlugin {
    metadata: PluginMetadata,
    apps: Arc<RwLock<Vec<App>>>,
    matcher: SkimMatcherV2,
}

impl AppSearchPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "app_search".to_string(),
                name: "Application Search".to_string(),
                description: "Search and launch applications".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("🚀"),
                trigger_keywords: vec![],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: PluginType::Native,
            },
            apps: Arc::new(RwLock::new(Vec::new())),
            matcher: SkimMatcherV2::default(),
        }
    }
    
    /// 初始化并加载应用
    pub async fn init(&self) {
        if let Ok(apps) = Self::scan_applications().await {
            let mut apps_guard = self.apps.write().await;
            *apps_guard = apps;
            tracing::info!("Loaded {} applications", apps_guard.len());
        }
    }
    
    /// 扫描系统应用
    async fn scan_applications() -> Result<Vec<App>> {
        #[cfg(target_os = "windows")]
        {
            Self::scan_windows_apps().await
        }
        
        #[cfg(target_os = "macos")]
        {
            Self::scan_macos_apps().await
        }
        
        #[cfg(target_os = "linux")]
        {
            Self::scan_linux_apps().await
        }
    }
    
    #[cfg(target_os = "windows")]
    async fn scan_windows_apps() -> Result<Vec<App>> {
        let mut apps = Vec::new();
        
        // 扫描开始菜单
        let common_start_menu = PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs");
        let user_start_menu = directories::BaseDirs::new()
            .map(|dirs| dirs.data_dir().join(r"Microsoft\Windows\Start Menu\Programs"))
            .unwrap_or_default();
        
        for start_menu in [common_start_menu, user_start_menu] {
            if let Ok(entries) = Self::scan_directory(&start_menu, ".lnk").await {
                apps.extend(entries);
            }
        }
        
        // 扫描桌面
        if let Some(dirs) = directories::UserDirs::new() {
            if let Some(desktop) = dirs.desktop_dir() {
                if let Ok(entries) = Self::scan_directory(&desktop.to_path_buf(), ".lnk").await {
                    apps.extend(entries);
                }
            }
        }
        
        Ok(apps)
    }
    
    #[cfg(target_os = "macos")]
    async fn scan_macos_apps() -> Result<Vec<App>> {
        let mut apps = Vec::new();
        let app_dirs = vec![
            PathBuf::from("/Applications"),
            PathBuf::from("/System/Applications"),
        ];
        
        for dir in app_dirs {
            if let Ok(entries) = Self::scan_directory(&dir, ".app").await {
                apps.extend(entries);
            }
        }
        
        Ok(apps)
    }
    
    #[cfg(target_os = "linux")]
    async fn scan_linux_apps() -> Result<Vec<App>> {
        let mut apps = Vec::new();
        let mut app_dirs = vec![
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
        ];
        
        if let Some(dirs) = directories::BaseDirs::new() {
            app_dirs.push(dirs.data_dir().join("applications"));
        }
        
        for dir in app_dirs {
            if let Ok(entries) = Self::scan_directory(&dir, ".desktop").await {
                apps.extend(entries);
            }
        }
        
        Ok(apps)
    }
    
    async fn scan_directory(dir: &PathBuf, extension: &str) -> Result<Vec<App>> {
        use std::fs;
        
        let mut apps = Vec::new();
        
        if !dir.exists() {
            return Ok(apps);
        }
        
        fn scan_recursive(dir: &PathBuf, extension: &str, apps: &mut Vec<App>) -> Result<()> {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    
                    if path.is_dir() {
                        let _ = scan_recursive(&path, extension, apps);
                    } else if let Some(ext) = path.extension() {
                        if ext.to_string_lossy().to_lowercase() == extension.trim_start_matches('.') {
                            if let Some(name) = path.file_stem() {
                                apps.push(App {
                                    name: name.to_string_lossy().to_string(),
                                    path: path.clone(),
                                    icon_path: None,
                                });
                            }
                        }
                    }
                }
            }
            Ok(())
        }
        
        scan_recursive(dir, extension, &mut apps)?;
        Ok(apps)
    }
}

#[async_trait]
impl Plugin for AppSearchPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim();
        
        if query.is_empty() {
            return Ok(vec![]);
        }
        
        let apps = self.apps.read().await;
        let mut results = Vec::new();
        
        for app in apps.iter() {
            if let Some(score) = self.matcher.fuzzy_match(&app.name, query) {
                results.push(QueryResult {
                    id: app.path.to_string_lossy().to_string(),
                    title: app.name.clone(),
                    subtitle: app.path.to_string_lossy().to_string(),
                    icon: WoxImage::emoji("📦"),
                    preview: None,
                    score: score as i32,
                    context_data: serde_json::Value::Null,
                    group: None,
                    plugin_id: self.metadata.id.clone(),
                    refreshable: false,
                    actions: vec![
                        Action {
                            id: "open".to_string(),
                            name: "Open".to_string(),
                            icon: None,
                            is_default: true,
                            prevent_hide: false,
                            hotkey: None,
                        }
                    ],
                });
            }
        }
        
        // 按分数排序
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(10); // 只返回前 10 个结果
        
        Ok(results)
    }
    
    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        if action_id == "open" {
            #[cfg(target_os = "windows")]
            {
                // 🔥 使用 CREATE_NO_WINDOW 标志隐藏控制台窗口
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                
                std::process::Command::new("cmd")
                    .args(["/C", "start", "", result_id])
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()?;
            }
            
            #[cfg(target_os = "macos")]
            {
                std::process::Command::new("open")
                    .arg(result_id)
                    .spawn()?;
            }
            
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("xdg-open")
                    .arg(result_id)
                    .spawn()?;
            }
            
            tracing::info!("Opened application: {}", result_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Unknown action"))
        }
    }
}
