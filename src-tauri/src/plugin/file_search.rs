// 文件搜索插件 - 超快速全盘扫描（类似 Everything）

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct FileItem {
    path: String,
    name: String,
    is_dir: bool,
}

pub struct FileSearchPlugin {
    metadata: PluginMetadata,
    files: Arc<RwLock<Vec<FileItem>>>,
    // 使用 HashMap 按文件名首字母索引，加速搜索
    name_index: Arc<RwLock<HashMap<char, Vec<usize>>>>,
    matcher: SkimMatcherV2,
    search_paths: Vec<PathBuf>,
}

impl FileSearchPlugin {
    pub fn new() -> Self {
        // 全盘搜索路径
        let mut search_paths = Vec::new();
        
        #[cfg(target_os = "windows")]
        {
            // Windows: 扫描所有盘符
            for drive in b'A'..=b'Z' {
                let path = PathBuf::from(format!("{}:\\", drive as char));
                if path.exists() {
                    search_paths.push(path);
                }
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            // macOS: 从根目录开始，但跳过系统目录
            search_paths.push(PathBuf::from("/Users"));
            search_paths.push(PathBuf::from("/Applications"));
        }
        
        #[cfg(target_os = "linux")]
        {
            // Linux: 从 home 开始
            if let Some(home) = directories::UserDirs::new() {
                search_paths.push(home.home_dir().to_path_buf());
            }
            search_paths.push(PathBuf::from("/usr"));
            search_paths.push(PathBuf::from("/opt"));
        }
        
        Self {
            metadata: PluginMetadata {
                id: "file_search".to_string(),
                name: "File Search".to_string(),
                description: "Search files and folders (Ultra-fast full disk scan)".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("📁"),
                trigger_keywords: vec![],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: PluginType::Native,
            },
            files: Arc::new(RwLock::new(Vec::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            matcher: SkimMatcherV2::default(),
            search_paths,
        }
    }
    
    /// 初始化并后台扫描文件
    pub async fn init(&self) {
        tracing::info!("Starting ultra-fast file scan...");
        
        // 立即返回，在后台扫描
        let files = self.files.clone();
        let name_index = self.name_index.clone();
        let paths = self.search_paths.clone();
        
        tokio::spawn(async move {
            let start = std::time::Instant::now();
            
            if let Ok(scanned_files) = Self::scan_files(&paths).await {
                let file_count = scanned_files.len();
                
                // 构建索引
                let mut index: HashMap<char, Vec<usize>> = HashMap::new();
                for (idx, file) in scanned_files.iter().enumerate() {
                    if let Some(first_char) = file.name.chars().next() {
                        let key = first_char.to_lowercase().next().unwrap_or(first_char);
                        index.entry(key).or_insert_with(Vec::new).push(idx);
                    }
                }
                
                // 保存数据
                let mut files_guard = files.write().await;
                *files_guard = scanned_files;
                
                let mut index_guard = name_index.write().await;
                *index_guard = index;
                
                let elapsed = start.elapsed();
                tracing::info!(
                    "✓ Indexed {} files in {:.2}s ({:.0} files/sec)", 
                    file_count,
                    elapsed.as_secs_f32(),
                    file_count as f32 / elapsed.as_secs_f32()
                );
            } else {
                tracing::error!("File scan failed");
            }
        });
    }
    
    /// 扫描文件（超快速）
    async fn scan_files(paths: &[PathBuf]) -> Result<Vec<FileItem>> {
        let paths = paths.to_vec();
        
        tokio::task::spawn_blocking(move || {
            let mut files = Vec::with_capacity(1000000); // 预分配 100 万容量
            let start = std::time::Instant::now();
            
            for base_path in &paths {
                if !base_path.exists() {
                    continue;
                }
                
                let drive_letter = base_path.to_string_lossy().chars().next().unwrap_or('C');
                tracing::info!("⚡ Scanning {}:\\ ...", drive_letter);
                
                let count_before = files.len();
                Self::ultra_fast_walk(base_path, &mut files);
                let count_after = files.len();
                
                let elapsed = start.elapsed().as_secs_f32();
                tracing::info!(
                    "  {}:\\ → {} files ({:.1}s, {:.0}/s)", 
                    drive_letter,
                    count_after - count_before,
                    elapsed,
                    (count_after - count_before) as f32 / elapsed
                );
            }
            
            Ok(files)
        })
        .await?
    }
    
    /// 超快速遍历（优化版本）
    fn ultra_fast_walk(base_path: &PathBuf, files: &mut Vec<FileItem>) {
        // 使用 VecDeque 作为 BFS 队列，比递归更快
        let mut queue = std::collections::VecDeque::with_capacity(1000);
        queue.push_back(base_path.clone());
        
        // 跳过列表（最小化）
        let skip_names = [
            "$Recycle.Bin",
            "System Volume Information", 
            "Config.Msi",
            "Recovery",
            "$RECYCLE.BIN",
        ];
        
        while let Some(current_dir) = queue.pop_front() {
            // 快速读取目录，忽略错误
            if let Ok(entries) = std::fs::read_dir(&current_dir) {
                for entry in entries.flatten() {
                    // 快速获取路径和名称
                    let Ok(file_name) = entry.file_name().into_string() else {
                        continue;
                    };
                    
                    // 快速跳过检查
                    if skip_names.contains(&file_name.as_str()) {
                        continue;
                    }
                    
                    let path = entry.path();
                    let path_str = path.to_string_lossy().into_owned();
                    
                    // 快速判断是否是目录（避免元数据查询）
                    let is_dir = if let Ok(file_type) = entry.file_type() {
                        file_type.is_dir()
                    } else {
                        false
                    };
                    
                    // 直接添加，不做其他检查
                    files.push(FileItem {
                        path: path_str,
                        name: file_name,
                        is_dir,
                    });
                    
                    // 如果是目录，加入队列
                    if is_dir {
                        queue.push_back(path);
                    }
                }
            }
        }
    }
    
    /// 打开文件或文件夹
    async fn open_file(path: &str) -> Result<()> {
        let path = path.to_string();
        
        tokio::task::spawn_blocking(move || {
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("cmd")
                    .args(["/C", "start", "", &path])
                    .spawn()?;
            }
            
            #[cfg(target_os = "macos")]
            {
                std::process::Command::new("open")
                    .arg(&path)
                    .spawn()?;
            }
            
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("xdg-open")
                    .arg(&path)
                    .spawn()?;
            }
            
            Ok(())
        })
        .await?
    }
    
    /// 打开文件所在文件夹
    async fn open_containing_folder(path: &str) -> Result<()> {
        let path_buf = PathBuf::from(path);
        let folder = path_buf.parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        
        Self::open_file(&folder).await
    }
}

#[async_trait]
impl Plugin for FileSearchPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let search = ctx.search.trim();
        
        // 至少输入2个字符才开始搜索
        if search.len() < 2 {
            return Ok(Vec::new());
        }
        
        let files = self.files.read().await;
        
        // 如果还没扫描完成
        if files.is_empty() {
            return Ok(vec![QueryResult {
                id: "scanning".to_string(),
                title: "⚡ Indexing files...".to_string(),
                subtitle: "Ultra-fast scan in progress".to_string(),
                icon: WoxImage::emoji("⚡"),
                preview: None,
                score: 100,
                context_data: serde_json::Value::Null,
                group: None,
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![],
            }]);
        }
        
        let name_index = self.name_index.read().await;
        let mut results = Vec::new();
        let search_lower = search.to_lowercase();
        let first_char = search_lower.chars().next().unwrap_or(' ');
        
        // 使用索引加速搜索
        let indices_to_search = if let Some(indices) = name_index.get(&first_char) {
            indices.as_slice()
        } else {
            // 如果索引中没有，搜索全部（兜底）
            &[]
        };
        
        // 如果索引为空，说明没有匹配首字母的，快速返回
        if !indices_to_search.is_empty() {
            for &idx in indices_to_search {
                if let Some(file) = files.get(idx) {
                    if let Some(score) = self.matcher.fuzzy_match(&file.name, search) {
                        let icon = if file.is_dir {
                            WoxImage::emoji("📁")
                        } else {
                            // 根据扩展名显示不同图标
                            let icon_str = if let Some(ext_pos) = file.name.rfind('.') {
                                match &file.name[ext_pos + 1..].to_lowercase().as_str() {
                                    &"txt" | &"md" | &"log" => "📄",
                                    &"pdf" => "📕",
                                    &"doc" | &"docx" => "📘",
                                    &"xls" | &"xlsx" => "📊",
                                    &"ppt" | &"pptx" => "📊",
                                    &"zip" | &"rar" | &"7z" => "📦",
                                    &"jpg" | &"jpeg" | &"png" | &"gif" | &"bmp" => "🖼️",
                                    &"mp3" | &"wav" | &"flac" => "🎵",
                                    &"mp4" | &"avi" | &"mkv" => "🎬",
                                    &"exe" | &"msi" => "⚙️",
                                    &"js" | &"ts" | &"py" | &"rs" | &"go" | &"java" => "💻",
                                    _ => "📄",
                                }
                            } else {
                                "📄"
                            };
                            WoxImage::emoji(icon_str)
                        };
                        
                        results.push(QueryResult {
                            id: file.path.clone(),
                            title: file.name.clone(),
                            subtitle: file.path.clone(),
                            icon,
                            preview: None,
                            score: score as i32,
                            context_data: serde_json::Value::Null,
                            group: None,
                            plugin_id: self.metadata.id.clone(),
                            refreshable: false,
                            actions: vec![
                                Action {
                                    id: "open".to_string(),
                                    name: if file.is_dir { "Open Folder" } else { "Open File" }.to_string(),
                                    icon: None,
                                    is_default: true,
                                    prevent_hide: false,
                                    hotkey: None,
                                },
                                Action {
                                    id: "open_folder".to_string(),
                                    name: "Open Containing Folder".to_string(),
                                    icon: None,
                                    is_default: false,
                                    prevent_hide: false,
                                    hotkey: Some("Ctrl+Enter".to_string()),
                                },
                            ],
                        });
                        
                        // 限制返回结果数量，避免 UI 卡顿
                        if results.len() >= 50 {
                            break;
                        }
                    }
                }
            }
        }
        
        // 按分数排序
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
    }
    
    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        match action_id {
            "open" => {
                Self::open_file(result_id).await?;
            }
            "open_folder" => {
                Self::open_containing_folder(result_id).await?;
            }
            _ => {}
        }
        
        Ok(())
    }
}
