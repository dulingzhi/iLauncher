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
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[cfg(target_os = "windows")]
use crate::mft_scanner::MftFileEntry;
// TODO: 重新实现 Scanner 集成
// use crate::mft_scanner::{ScannerLauncher, ScannerClient};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchConfig {
    #[serde(default = "default_use_mft")]
    pub use_mft: bool,
}

fn default_use_mft() -> bool {
    true  // 默认启用 MFT
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileItem {
    path: String,
    name: String,
    is_dir: bool,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    modified: i64,
}

#[cfg(target_os = "windows")]
impl From<MftFileEntry> for FileItem {
    fn from(mft: MftFileEntry) -> Self {
        Self {
            path: mft.path.clone(),
            name: mft.name(),
            is_dir: mft.is_dir(),
            size: mft.size(),
            modified: mft.modified(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileCache {
    version: u32,
    created_at: DateTime<Utc>,
    files: Vec<FileItem>,
    name_index: HashMap<char, Vec<usize>>,
}

pub struct FileSearchPlugin {
    metadata: PluginMetadata,
    files: Arc<RwLock<Vec<FileItem>>>,
    // 使用 HashMap 按文件名首字母索引，加速搜索
    name_index: Arc<RwLock<HashMap<char, Vec<usize>>>>,
    matcher: SkimMatcherV2,
    search_paths: Vec<PathBuf>,
    config: Arc<RwLock<FileSearchConfig>>,
}

impl FileSearchPlugin {
    pub fn new() -> Self {
        Self::new_with_config(true) // 默认启用 MFT
    }
    
    pub fn new_with_config(use_mft: bool) -> Self {
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
                settings: vec![
                    SettingDefinition {
                        r#type: "checkbox".to_string(),
                        key: Some("use_mft".to_string()),
                        label: Some("启用 MFT 快速扫描 (需要管理员权限)".to_string()),
                        value: Some(serde_json::json!(true)),
                    },
                ],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: PluginType::Native,
            },
            files: Arc::new(RwLock::new(Vec::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            matcher: SkimMatcherV2::default(),
            search_paths,
            config: Arc::new(RwLock::new(FileSearchConfig {
                use_mft,
            })),
        }
    }
    
    /// 初始化并后台扫描文件
    pub async fn init(&self) {
        tracing::info!("Starting file index initialization...");
        
        let files = self.files.clone();
        let name_index = self.name_index.clone();
        let paths = self.search_paths.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let use_mft = config.read().await.use_mft;
            
            // MFT模式：每次都重建索引（速度极快，9秒扫描450万文件）
            #[cfg(target_os = "windows")]
            if use_mft {
                tracing::info!("🚀 MFT mode enabled - rebuilding index from MFT (no cache)");
                Self::rebuild_index(files, name_index, paths, config).await;
                return;
            }
            
            // 标准BFS模式：使用缓存机制（扫描很慢，需要缓存）
            tracing::info!("📁 Standard mode - attempting to load from cache");
            
            // 尝试加载缓存
            if let Ok(cache_path) = Self::get_cache_path() {
                if cache_path.exists() {
                    tracing::info!("Loading file index from cache...");
                    let start = std::time::Instant::now();
                    
                    match Self::load_cache(&cache_path).await {
                        Ok(cache) => {
                            let file_count = cache.files.len();
                            
                            // 加载缓存数据
                            let mut files_guard = files.write().await;
                            *files_guard = cache.files;
                            
                            let mut index_guard = name_index.write().await;
                            *index_guard = cache.name_index;
                            
                            let elapsed = start.elapsed();
                            let age = Utc::now() - cache.created_at;
                            
                            tracing::info!(
                                "✓ Loaded {} files from cache in {:.3}s (cache age: {}h)",
                                file_count,
                                elapsed.as_secs_f32(),
                                age.num_hours()
                            );
                            
                            // 如果缓存超过24小时，后台重建索引
                            if age.num_hours() > 24 {
                                tracing::info!("Cache is old, rebuilding index in background...");
                                let files_clone = files.clone();
                                let name_index_clone = name_index.clone();
                                let paths_clone = paths.clone();
                                let config_clone = config.clone();
                                
                                tokio::spawn(async move {
                                    Self::rebuild_index(files_clone, name_index_clone, paths_clone, config_clone).await;
                                });
                            }
                            
                            return;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load cache: {}, will rebuild", e);
                        }
                    }
                }
            }
            
            // 缓存不存在或加载失败，重建索引
            Self::rebuild_index(files, name_index, paths, config).await;
        });
    }
    
    /// 重建文件索引
    async fn rebuild_index(
        files: Arc<RwLock<Vec<FileItem>>>,
        name_index: Arc<RwLock<HashMap<char, Vec<usize>>>>,
        paths: Vec<PathBuf>,
        config: Arc<RwLock<FileSearchConfig>>,
    ) {
        let start = std::time::Instant::now();
        
        let use_mft = config.read().await.use_mft;
        
        if let Ok(scanned_files) = Self::scan_files(&paths, use_mft).await {
            let file_count = scanned_files.len();
            
            // 构建索引
            let mut index: HashMap<char, Vec<usize>> = HashMap::new();
            for (idx, file) in scanned_files.iter().enumerate() {
                if let Some(first_char) = file.name.chars().next() {
                    let key = first_char.to_lowercase().next().unwrap_or(first_char);
                    index.entry(key).or_insert_with(Vec::new).push(idx);
                }
            }
            
            // 保存到内存
            let mut files_guard = files.write().await;
            *files_guard = scanned_files.clone();
            
            let mut index_guard = name_index.write().await;
            *index_guard = index.clone();
            
            let elapsed = start.elapsed();
            tracing::info!(
                "✓ Indexed {} files in {:.2}s ({:.0} files/sec)", 
                file_count,
                elapsed.as_secs_f32(),
                file_count as f32 / elapsed.as_secs_f32()
            );
            
            // 保存缓存策略：
            // - MFT模式：不保存缓存（每次重建很快，没必要缓存）
            // - BFS模式：保存缓存（扫描很慢，需要缓存）
            #[cfg(target_os = "windows")]
            if use_mft {
                tracing::info!("🚀 MFT mode - skipping cache save (will rebuild on next startup)");
                return;
            }
            
            // 异步保存缓存（仅BFS模式）
            tokio::spawn(async move {
                if let Ok(cache_path) = Self::get_cache_path() {
                    let cache = FileCache {
                        version: 1,
                        created_at: Utc::now(),
                        files: scanned_files,
                        name_index: index,
                    };
                    
                    if let Err(e) = Self::save_cache(&cache_path, &cache).await {
                        tracing::error!("Failed to save cache: {}", e);
                    } else {
                        tracing::info!("✓ Cache saved to {:?}", cache_path);
                    }
                }
            });
        } else {
            tracing::error!("File scan failed");
        }
    }
    
    /// 获取缓存文件路径
    fn get_cache_path() -> Result<PathBuf> {
        use crate::utils::paths;
        
        let cache_dir = paths::get_cache_dir()?;
        Ok(cache_dir.join("file_index.bin"))
    }
    
    /// 加载缓存
    async fn load_cache(path: &PathBuf) -> Result<FileCache> {
        let path = path.clone();
        
        tokio::task::spawn_blocking(move || {
            let data = std::fs::read(path)?;
            let cache: FileCache = bincode::deserialize(&data)?;
            
            // 验证版本
            if cache.version != 1 {
                anyhow::bail!("Unsupported cache version: {}", cache.version);
            }
            
            Ok(cache)
        })
        .await?
    }
    
    /// 保存缓存
    async fn save_cache(path: &PathBuf, cache: &FileCache) -> Result<()> {
        let path = path.clone();
        let data = bincode::serialize(cache)?;
        
        tokio::task::spawn_blocking(move || {
            std::fs::write(path, data)?;
            Ok(())
        })
        .await?
    }
    
    /// 扫描文件（超快速）
    async fn scan_files(paths: &[PathBuf], use_mft: bool) -> Result<Vec<FileItem>> {
        // Windows: 如果启用 MFT，直接查询数据库
        #[cfg(target_os = "windows")]
        {
            if use_mft {
                tracing::info!("🚀 MFT mode enabled - querying from database");
                return Self::load_from_mft_database().await;
            } else {
                tracing::info!("⚡ MFT disabled in settings, using standard scan mode");
            }
        }
        
        // 降级到标准 BFS 扫描
        Self::scan_with_bfs(paths).await
    }
    
    /// 从 MFT 数据库加载所有文件（可选：用于初始化）
    #[cfg(target_os = "windows")]
    async fn load_from_mft_database() -> Result<Vec<FileItem>> {
        use crate::mft_scanner::database;
        use crate::utils::paths;
        
        // 使用统一的数据目录
        let output_dir = paths::get_mft_database_dir()?
            .to_string_lossy()
            .to_string();
        
        // 从所有盘符的数据库加载（这里加载全量数据用于缓存）
        // 注意：实际搜索时应该使用 search_all_drives 进行按需查询
        tracing::info!("Loading files from MFT databases in {:?}", output_dir);
        
        // 暂时返回空，实际搜索时再查询
        // 这样可以避免启动时加载全部数据（450万文件太多）
        tracing::info!("MFT mode: will query database on demand during search");
        Ok(Vec::new())
    }
    
    /// 通过 IPC 与扫描器进程通信，获取 MFT 扫描结果
    #[cfg(target_os = "windows")]
    async fn scan_with_mft_ipc(_paths: &[PathBuf]) -> Result<Vec<FileItem>> {
        // 已废弃：MFT 现在使用数据库查询，不需要 IPC
        tracing::warn!("scan_with_mft_ipc is deprecated, use database queries instead");
        Ok(Vec::new())
    }
    
    /// BFS 扫描方式（所有平台）
    async fn scan_with_bfs(paths: &[PathBuf]) -> Result<Vec<FileItem>> {
        let paths = paths.to_vec();
        
        tokio::task::spawn_blocking(move || {
            let mut files = Vec::with_capacity(1000000); // 预分配 100 万容量
            let start = std::time::Instant::now();
            
            for base_path in &paths {
                if !base_path.exists() {
                    continue;
                }
                
                let drive_letter = base_path.to_string_lossy().chars().next().unwrap_or('C');
                tracing::info!("⚡ BFS scanning {}:\\ ...", drive_letter);
                
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
                        size: 0,  // BFS 模式不获取大小（性能优化）
                        modified: 0,
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
    
        /// 从 MFT 数据库查询文件
    #[cfg(target_os = "windows")]
    async fn query_from_mft_database(&self, search: &str, _ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query_start = std::time::Instant::now();
        use crate::mft_scanner::database;
        use crate::utils::paths;
        
        // 使用统一的数据目录
        let output_dir = paths::get_mft_database_dir()?
            .to_string_lossy()
            .to_string();
        
        tracing::debug!("🔍 MFT query: '{}' from {}", search, output_dir);
        
        // 检查数据库是否存在
        let db_dir = std::path::Path::new(&output_dir);
        if !db_dir.exists() {
            tracing::warn!("MFT database directory not found: {}", output_dir);
            return Ok(vec![QueryResult {
                id: "mft_scanning".to_string(),
                title: "⚡ MFT Scanner is indexing...".to_string(),
                subtitle: "Please wait for initial scan to complete".to_string(),
                icon: WoxImage::emoji("⏳"),
                preview: None,
                score: 100,
                context_data: serde_json::Value::Null,
                group: None,
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![],
            }]);
        }
        
        // 查询数据库（限制返回50个结果）
        let mft_entries = match database::search_all_drives(search, &output_dir, 50) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::error!("MFT database query failed: {:#}", e);
                return Ok(vec![QueryResult {
                    id: "mft_error".to_string(),
                    title: "❌ MFT Query Failed".to_string(),
                    subtitle: format!("Error: {}", e),
                    icon: WoxImage::emoji("⚠️"),
                    preview: None,
                    score: 100,
                    context_data: serde_json::Value::Null,
                    group: None,
                    plugin_id: self.metadata.id.clone(),
                    refreshable: false,
                    actions: vec![],
                }]);
            }
        };
        
        // 如果没有结果，返回提示
        if mft_entries.is_empty() {
            return Ok(vec![QueryResult {
                id: "no_results".to_string(),
                title: "No files found".to_string(),
                subtitle: format!("No matches for '{}'", search),
                icon: WoxImage::emoji("🔍"),
                preview: None,
                score: 0,
                context_data: serde_json::Value::Null,
                group: None,
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![],
            }]);
        }
        
        // 转换为 QueryResult
        let mut results = Vec::new();
        for entry in mft_entries {
            let is_dir = entry.is_dir();
            let name = entry.name();
            let icon = if is_dir {
                WoxImage::emoji("📁")
            } else {
                WoxImage::emoji("📄")
            };
            
            results.push(QueryResult {
                id: entry.path.clone(),
                title: name.clone(),
                subtitle: entry.path.clone(),
                icon,
                preview: Some(Preview::Text(format!(
                    "Path: {}\nType: {}\nSize: {} bytes",
                    entry.path,
                    if is_dir { "Directory" } else { "File" },
                    entry.size()
                ))),
                score: entry.priority.max(50),
                context_data: serde_json::json!({
                    "path": entry.path,
                    "is_dir": is_dir,
                }),
                group: None,
                plugin_id: self.metadata.id.clone(),
                refreshable: false,
                actions: vec![
                    Action {
                        id: "open".to_string(),
                        name: if is_dir {
                            "Open Folder".to_string()
                        } else {
                            "Open File".to_string()
                        },
                        icon: Some(WoxImage::emoji("📂")),
                        is_default: true,
                        prevent_hide: false,
                        hotkey: None,
                    },
                    Action {
                        id: "open_folder".to_string(),
                        name: "Open Containing Folder".to_string(),
                        icon: Some(WoxImage::emoji("📁")),
                        is_default: false,
                        prevent_hide: false,
                        hotkey: None,
                    },
                    Action {
                        id: "copy_path".to_string(),
                        name: "Copy Path".to_string(),
                        icon: Some(WoxImage::emoji("📋")),
                        is_default: false,
                        prevent_hide: false,
                        hotkey: None,
                    },
                ],
            });
        }
        
        let query_elapsed = query_start.elapsed();
        tracing::info!(
            "✅ MFT query completed: '{}' → {} results in {:.2}ms",
            search,
            results.len(),
            query_elapsed.as_secs_f64() * 1000.0
        );
        
        Ok(results)
    }
    
    /// 复制文本到剪贴板
    async fn copy_to_clipboard(text: &str) -> Result<()> {
        let text = text.to_string();
        
        tokio::task::spawn_blocking(move || {
            #[cfg(target_os = "windows")]
            {
                use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
                use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
                use windows::Win32::Foundation::HANDLE;
                
                unsafe {
                    if OpenClipboard(None).is_ok() {
                        EmptyClipboard().ok();
                        
                        // 转换为 UTF-16
                        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                        let len = wide.len() * 2;
                        
                        // 分配全局内存
                        if let Ok(hglb) = GlobalAlloc(GMEM_MOVEABLE, len) {
                            let lptstr = GlobalLock(hglb);
                            std::ptr::copy_nonoverlapping(
                                wide.as_ptr() as *const u8,
                                lptstr as *mut u8,
                                len,
                            );
                            GlobalUnlock(hglb).ok();
                            
                            SetClipboardData(13, HANDLE(hglb.0)).ok(); // CF_UNICODETEXT = 13
                        }
                        
                        CloseClipboard().ok();
                    }
                }
            }
            
            #[cfg(target_os = "macos")]
            {
                use std::process::Command;
                use std::io::Write;
                
                let mut child = Command::new("pbcopy")
                    .stdin(std::process::Stdio::piped())
                    .spawn()?;
                
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait()?;
            }
            
            #[cfg(target_os = "linux")]
            {
                use std::process::Command;
                use std::io::Write;
                
                let mut child = Command::new("xclip")
                    .args(["-selection", "clipboard"])
                    .stdin(std::process::Stdio::piped())
                    .spawn()?;
                
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait()?;
            }
            
            tracing::info!("Copied to clipboard: {}", text);
            Ok::<(), anyhow::Error>(())
        })
        .await?
    }
    
    /// 删除文件
    async fn delete_file(path: &str) -> Result<()> {
        let path = path.to_string();
        
        tokio::task::spawn_blocking(move || {
            let path_buf = PathBuf::from(&path);
            
            if path_buf.is_dir() {
                std::fs::remove_dir_all(&path_buf)?;
                tracing::info!("Deleted directory: {}", path);
            } else {
                std::fs::remove_file(&path_buf)?;
                tracing::info!("Deleted file: {}", path);
            }
            
            Ok::<(), anyhow::Error>(())
        })
        .await?
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
        
        // 检查是否启用 MFT，启用则直接查询数据库
        #[cfg(target_os = "windows")]
        {
            let use_mft = self.config.read().await.use_mft;
            if use_mft {
                return self.query_from_mft_database(search, ctx).await;
            }
        }
        
        // 标准 BFS 模式：使用内存索引
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
                                    name: if file.is_dir { "打开文件夹" } else { "打开文件" }.to_string(),
                                    icon: Some(WoxImage::emoji("📂")),
                                    is_default: true,
                                    prevent_hide: false,
                                    hotkey: None,
                                },
                                Action {
                                    id: "open_folder".to_string(),
                                    name: "打开所在位置".to_string(),
                                    icon: Some(WoxImage::emoji("📁")),
                                    is_default: false,
                                    prevent_hide: false,
                                    hotkey: Some("Ctrl+O".to_string()),
                                },
                                Action {
                                    id: "copy_path".to_string(),
                                    name: "复制路径".to_string(),
                                    icon: Some(WoxImage::emoji("📋")),
                                    is_default: false,
                                    prevent_hide: true,
                                    hotkey: Some("Ctrl+C".to_string()),
                                },
                                Action {
                                    id: "copy_name".to_string(),
                                    name: "复制文件名".to_string(),
                                    icon: Some(WoxImage::emoji("📝")),
                                    is_default: false,
                                    prevent_hide: true,
                                    hotkey: None,
                                },
                                Action {
                                    id: "delete".to_string(),
                                    name: "删除".to_string(),
                                    icon: Some(WoxImage::emoji("🗑️")),
                                    is_default: false,
                                    prevent_hide: false,
                                    hotkey: Some("Del".to_string()),
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
        tracing::info!("FileSearch::execute - result_id: {}, action_id: {}", result_id, action_id);
        
        match action_id {
            "open" => {
                tracing::info!("Executing 'open' action");
                Self::open_file(result_id).await?;
            }
            "open_folder" => {
                tracing::info!("Executing 'open_folder' action");
                Self::open_containing_folder(result_id).await?;
            }
            "copy_path" => {
                tracing::info!("Executing 'copy_path' action");
                Self::copy_to_clipboard(result_id).await?;
            }
            "copy_name" => {
                tracing::info!("Executing 'copy_name' action");
                let path_buf = PathBuf::from(result_id);
                if let Some(file_name) = path_buf.file_name() {
                    Self::copy_to_clipboard(&file_name.to_string_lossy()).await?;
                }
            }
            "delete" => {
                tracing::info!("Executing 'delete' action");
                Self::delete_file(result_id).await?;
            }
            _ => {
                tracing::warn!("Unknown action_id: {}", action_id);
            }
        }
        
        Ok(())
    }
}

