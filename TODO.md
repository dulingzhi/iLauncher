# iLauncher - Tauri 重构计划

> 基于 Wox 功能分析的完整重构方案
> 
> 日期: 2025年11月3日

---

## 📋 目录

- [Wox 功能分析](#wox-功能分析)
- [架构设计](#架构设计)
- [技术栈](#技术栈)
- [模块设计](#模块设计)
- [实施路线图](#实施路线图)
- [开发任务](#开发任务)

---

## 🔍 Wox 功能分析

### 核心架构

Wox 采用微服务架构，包含以下组件：

```
┌─────────────────────────────────────────────────────┐
│                  Wox 原始架构                        │
├─────────────────────────────────────────────────────┤
│  wox.ui.flutter (Flutter) - UI 层                   │
│  ↕ WebSocket + HTTP                                 │
│  wox.core (Go) - 核心后端                           │
│  ↕ WebSocket                                        │
│  Plugin Hosts (Python/Node.js) - 插件宿主           │
│  ↕                                                  │
│  Plugins - 各类插件                                  │
└─────────────────────────────────────────────────────┘
```

### 核心功能模块

#### 1. 系统插件
- **App**: 应用搜索和启动
- **Calculator**: 数学计算器（支持复杂函数）
- **File**: 文件搜索和管理
- **Clipboard**: 剪贴板历史
- **MediaPlayer**: 媒体播放器控制
- **WebSearch**: Web 搜索
- **Converter**: 单位转换
- **Browser**: 浏览器书签搜索
- **AI Command**: AI 命令执行
- **Chat**: AI 聊天（支持 MCP 协议）

#### 2. 插件系统特性

**脚本插件**
- 单文件实现
- 按需执行（无常驻进程）
- 支持 Python、JavaScript、Bash
- 通过注释定义元数据
- JSON-RPC 标准输入输出通信
- 10秒执行超时限制

**完整功能插件**
- 独立进程运行
- WebSocket 通信
- 持久化状态管理
- 支持 Python (`wox-plugin`) 和 Node.js (`@wox-launcher/wox-plugin`)
- 丰富的 API 支持（AI、设置、预览）
- 完整生命周期管理

#### 3. 查询系统

**输入查询**
```
格式: [触发关键词] [命令] [搜索内容]
示例:
  - "wpm install emoji" - 触发词:wpm, 命令:install, 搜索:emoji
  - "emoji smile" - 触发词:emoji, 搜索:smile
  - "mail" - 全局搜索:mail
```

**选择查询**
- 选中文件/文本作为查询
- 支持拖拽操作

#### 4. 核心 API

```go
type API interface {
    ChangeQuery(ctx, query)          // 改变查询
    HideApp(ctx)                     // 隐藏应用
    ShowApp(ctx)                     // 显示应用
    Notify(ctx, description)         // 通知
    Log(ctx, level, msg)             // 日志
    GetTranslation(ctx, key)         // 翻译
    GetSetting(ctx, key)             // 获取设置
    SaveSetting(ctx, key, value)     // 保存设置
    OnSettingChanged(ctx, callback)  // 设置变更监听
    RegisterQueryCommands(ctx, cmds) // 注册命令
    AIChatStream(ctx, ...)           // AI 流式聊天
}
```

#### 5. 插件配置 (plugin.json)

```json
{
  "Id": "plugin-uuid",
  "Name": "Plugin Name",
  "Author": "Author Name",
  "Version": "1.0.0",
  "MinWoxVersion": "2.0.0",
  "Runtime": "Python|Nodejs|Dotnet",
  "Icon": "icon.png",
  "EntryFile": "main.py",
  "SupportedOS": ["Windows", "Linux", "Macos"],
  "TriggerKeywords": ["keyword1", "keyword2"],
  "Commands": [
    {"Command": "install", "Description": "Install plugin"}
  ],
  "Settings": [
    {
      "Type": "textbox|checkbox|select|head|newline",
      "Value": { "Key": "setting_key", "Label": "Label" }
    }
  ]
}
```

#### 6. 主题系统

- JSON 格式主题配置
- 支持自定义颜色、字体、间距
- 系统内置主题
- 用户自定义主题
- AI 生成主题

#### 7. 其他特性

- **热键管理**: 主热键、选择热键、查询热键
- **MRU (Most Recently Used)**: 最近使用记录
- **自动备份**: 设置和数据备份
- **自动更新**: 检查和安装更新
- **多语言**: i18n 支持
- **托盘图标**: 系统托盘集成
- **深度链接**: `wox://` 协议支持
- **Action Panel**: `Alt+J` 显示更多操作

---

## 🏗️ 架构设计

### iLauncher Tauri 架构

```
┌─────────────────────────────────────────────────────┐
│                Tauri Application                     │
├─────────────────────────────────────────────────────┤
│                                                      │
│  ┌────────────────────────────────────────────┐    │
│  │     React UI Layer (TypeScript)            │    │
│  │  ┌──────────────────────────────────────┐  │    │
│  │  │ • SearchBox (主搜索框)                │  │    │
│  │  │ • ResultList (结果列表)               │  │    │
│  │  │ • ActionPanel (操作面板)              │  │    │
│  │  │ • SettingsView (设置界面)             │  │    │
│  │  │ • PluginStore (插件商店)              │  │    │
│  │  │ • ThemeEditor (主题编辑器)            │  │    │
│  │  └──────────────────────────────────────┘  │    │
│  └────────────────────────────────────────────┘    │
│                       ↕                             │
│              Tauri IPC Commands                     │
│                       ↕                             │
│  ┌────────────────────────────────────────────┐    │
│  │      Rust Backend Core (Tauri)             │    │
│  │  ┌──────────────────────────────────────┐  │    │
│  │  │ LauncherEngine                       │  │    │
│  │  │  ├─ PluginManager                   │  │    │
│  │  │  ├─ QueryProcessor                  │  │    │
│  │  │  ├─ IndexService                    │  │    │
│  │  │  │   ├─ AppIndex                    │  │    │
│  │  │  │   └─ FileIndex                   │  │    │
│  │  │  ├─ HotkeyManager                   │  │    │
│  │  │  ├─ SettingsManager                 │  │    │
│  │  │  ├─ ThemeEngine                     │  │    │
│  │  │  └─ AIService                       │  │    │
│  │  └──────────────────────────────────────┘  │    │
│  └────────────────────────────────────────────┘    │
│                       ↕                             │
│  ┌────────────────────────────────────────────┐    │
│  │         Plugin Hosts & Runners              │    │
│  │  ┌────────┐ ┌────────┐ ┌────────┐         │    │
│  │  │ Python │ │ Node.js│ │ Script │         │    │
│  │  │  Host  │ │  Host  │ │ Runner │         │    │
│  │  └────────┘ └────────┘ └────────┘         │    │
│  └────────────────────────────────────────────┘    │
│                       ↕                             │
│  ┌────────────────────────────────────────────┐    │
│  │              User Plugins                   │    │
│  └────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

### 数据流

```
用户输入 → SearchBox
    ↓
Tauri Command: query(input)
    ↓
QueryProcessor.process(input)
    ↓ (解析: trigger_keyword, command, search)
PluginManager.dispatch(query)
    ↓ (分发到匹配的插件)
[Plugin1, Plugin2, ...].query(context)
    ↓ (并发执行)
聚合结果 → 排序 → 评分
    ↓
返回 UI → ResultList 渲染
    ↓ (用户选择)
执行 Action → execute_action(result_id, action_id)
```

---

## 🛠️ 技术栈

### 前端技术栈

```json
{
  "framework": "React 18+",
  "language": "TypeScript 5+",
  "stateManagement": "Zustand / Jotai",
  "styling": "Tailwind CSS",
  "components": "Radix UI / Headless UI",
  "animation": "Framer Motion",
  "virtualization": "React Virtual",
  "icons": "Lucide React",
  "build": "Vite"
}
```

### 后端技术栈 (Rust)

```toml
[dependencies]
# Tauri Core
tauri = { version = "2", features = ["macos-private-api"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 异步运行时
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# 数据库
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio", "migrate"] }

# 搜索引擎
tantivy = "0.21"          # 全文搜索
fuzzy-matcher = "0.3"     # 模糊匹配
nucleo = "0.2"            # 快速模糊搜索（类似 fzf）

# 系统集成
global-hotkey = "0.5"     # 全局热键
notify = "6"              # 文件系统监听
directories = "5"         # 系统目录
sysinfo = "0.30"          # 系统信息

# HTTP & Network
reqwest = { version = "0.11", features = ["json"] }
tokio-tungstenite = "0.21" # WebSocket

# 插件系统
pyo3 = { version = "0.20", features = ["auto-initialize"] } # Python
# 或使用独立进程通信
serde_json = "1"

# AI 集成
async-openai = "0.20"
anthropic-sdk = "0.1"     # Claude
ollama-rs = "0.1"         # Ollama 本地模型

# 工具库
regex = "1"
chrono = "0.4"
uuid = { version = "1", features = ["v4", "serde"] }
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"

# 平台特定
[target.'cfg(windows)'.dependencies]
windows = { version = "0.52", features = ["Win32_UI_Shell", "Win32_System_Registry"] }

[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.25"
objc = "0.2"

[target.'cfg(target_os = "linux")'.dependencies]
freedesktop-entry-parser = "1"
```

---

## 📦 模块设计

### 1. 核心引擎 (Core Engine)

#### 文件结构
```
src-tauri/src/
├── core/
│   ├── mod.rs              # 模块导出
│   ├── engine.rs           # LauncherEngine 主引擎
│   ├── query.rs            # QueryProcessor 查询处理
│   └── types.rs            # 核心类型定义
```

#### 核心类型
```rust
// src-tauri/src/core/types.rs

/// 查询上下文
pub struct QueryContext {
    pub query_type: QueryType,
    pub trigger_keyword: String,
    pub command: Option<String>,
    pub search: String,
    pub raw_query: String,
}

pub enum QueryType {
    Input,      // 普通输入
    Selection,  // 选择查询
}

/// 查询结果
#[derive(Serialize, Deserialize, Clone)]
pub struct QueryResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: WoxImage,
    pub score: i32,
    pub plugin_id: String,
    pub context_data: serde_json::Value,
    pub actions: Vec<Action>,
    pub preview: Option<Preview>,
    pub refreshable: bool,
    pub group: Option<String>,
}

/// 操作
#[derive(Serialize, Deserialize, Clone)]
pub struct Action {
    pub id: String,
    pub name: String,
    pub icon: Option<WoxImage>,
    pub is_default: bool,
    pub hotkey: Option<String>,
    pub prevent_hide: bool,
}

/// 图标
#[derive(Serialize, Deserialize, Clone)]
pub enum WoxImage {
    Svg(String),           // SVG 内容
    File(String),          // 文件路径
    Url(String),           // URL
    Base64(String),        // Base64 编码
    Emoji(String),         // Emoji
    SystemIcon(String),    // 系统图标
}

/// 预览
#[derive(Serialize, Deserialize, Clone)]
pub enum Preview {
    Text(String),
    Markdown(String),
    Image(String),
    Html(String),
    File(String),
}
```

#### 主引擎
```rust
// src-tauri/src/core/engine.rs

pub struct LauncherEngine {
    plugin_manager: Arc<PluginManager>,
    index_service: Arc<IndexService>,
    settings: Arc<RwLock<Settings>>,
    query_processor: Arc<QueryProcessor>,
    hotkey_manager: Arc<HotkeyManager>,
    theme_engine: Arc<ThemeEngine>,
    ai_service: Arc<AIService>,
}

impl LauncherEngine {
    pub async fn new() -> Result<Self> {
        // 初始化数据库
        let db = Database::init().await?;
        
        // 初始化各组件
        let settings = Arc::new(RwLock::new(Settings::load().await?));
        let plugin_manager = Arc::new(PluginManager::new(db.clone()).await?);
        let index_service = Arc::new(IndexService::new(db.clone()).await?);
        let query_processor = Arc::new(QueryProcessor::new());
        let hotkey_manager = Arc::new(HotkeyManager::new()?);
        let theme_engine = Arc::new(ThemeEngine::new().await?);
        let ai_service = Arc::new(AIService::new(settings.clone()).await?);
        
        Ok(Self {
            plugin_manager,
            index_service,
            settings,
            query_processor,
            hotkey_manager,
            theme_engine,
            ai_service,
        })
    }
    
    /// 主查询入口
    pub async fn query(&self, input: &str) -> Result<Vec<QueryResult>> {
        // 1. 解析查询
        let ctx = self.query_processor.parse(input)?;
        
        // 2. 获取匹配的插件
        let plugins = self.plugin_manager.get_matching_plugins(&ctx).await;
        
        // 3. 并发执行查询
        let mut tasks = Vec::new();
        for plugin in plugins {
            let ctx = ctx.clone();
            tasks.push(plugin.query(ctx));
        }
        
        let results_vec = futures::future::join_all(tasks).await;
        
        // 4. 聚合结果
        let mut all_results = Vec::new();
        for results in results_vec {
            if let Ok(mut results) = results {
                all_results.append(&mut results);
            }
        }
        
        // 5. 排序和评分
        self.query_processor.rank_results(&mut all_results, &ctx);
        
        Ok(all_results)
    }
    
    /// 执行操作
    pub async fn execute_action(
        &self,
        result_id: &str,
        action_id: &str
    ) -> Result<()> {
        self.plugin_manager.execute_action(result_id, action_id).await
    }
}
```

### 2. 插件系统 (Plugin System)

#### 文件结构
```
src-tauri/src/
├── plugin/
│   ├── mod.rs              # 插件系统核心
│   ├── manager.rs          # PluginManager
│   ├── trait.rs            # Plugin trait
│   ├── api.rs              # PluginAPI
│   ├── metadata.rs         # 插件元数据
│   ├── host/
│   │   ├── mod.rs
│   │   ├── python.rs       # Python 插件 Host
│   │   ├── nodejs.rs       # Node.js 插件 Host
│   │   └── script.rs       # 脚本插件 Runner
│   └── native/
│       ├── mod.rs
│       ├── calculator.rs   # 计算器插件
│       ├── app.rs          # 应用搜索插件
│       ├── file.rs         # 文件搜索插件
│       └── clipboard.rs    # 剪贴板插件
```

#### Plugin Trait
```rust
// src-tauri/src/plugin/trait.rs

#[async_trait]
pub trait Plugin: Send + Sync {
    /// 获取插件元数据
    fn metadata(&self) -> &PluginMetadata;
    
    /// 初始化插件
    async fn init(&mut self, api: PluginAPI) -> Result<()>;
    
    /// 查询
    async fn query(&self, ctx: QueryContext) -> Result<Vec<QueryResult>>;
    
    /// 执行操作
    async fn execute_action(&self, action: &Action, context_data: serde_json::Value) -> Result<()>;
    
    /// 卸载清理（可选）
    async fn on_unload(&self) -> Result<()> {
        Ok(())
    }
    
    /// 设置变更（可选）
    async fn on_setting_changed(&self, key: &str, value: &serde_json::Value) -> Result<()> {
        Ok(())
    }
}

/// 插件元数据
#[derive(Serialize, Deserialize, Clone)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub min_launcher_version: String,
    pub description: String,
    pub icon: WoxImage,
    pub trigger_keywords: Vec<String>,
    pub commands: Vec<Command>,
    pub settings: Vec<SettingDefinition>,
    pub supported_os: Vec<String>,
    pub plugin_type: PluginType,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum PluginType {
    Native,     // Rust 原生插件
    Python,     // Python 插件
    NodeJS,     // Node.js 插件
    Script,     // 脚本插件
}
```

#### Plugin Manager
```rust
// src-tauri/src/plugin/manager.rs

pub struct PluginManager {
    plugins: RwLock<HashMap<String, Box<dyn Plugin>>>,
    python_host: Arc<PythonHost>,
    nodejs_host: Arc<NodeJsHost>,
    script_runner: Arc<ScriptRunner>,
    db: Database,
}

impl PluginManager {
    /// 加载所有插件
    pub async fn load_all_plugins(&self) -> Result<()> {
        // 1. 加载原生插件
        self.load_native_plugins().await?;
        
        // 2. 扫描并加载用户插件
        self.scan_and_load_user_plugins().await?;
        
        Ok(())
    }
    
    /// 安装插件
    pub async fn install_plugin(&self, url_or_path: &str) -> Result<()> {
        // 1. 下载/复制插件
        // 2. 验证 plugin.json
        // 3. 解压到插件目录
        // 4. 加载插件
        // 5. 保存到数据库
    }
    
    /// 获取匹配的插件
    pub async fn get_matching_plugins(
        &self,
        ctx: &QueryContext
    ) -> Vec<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().await;
        
        plugins.values()
            .filter(|p| {
                // 检查触发关键词
                if ctx.trigger_keyword == "*" {
                    p.metadata().trigger_keywords.contains(&"*".to_string())
                } else {
                    p.metadata().trigger_keywords.contains(&ctx.trigger_keyword)
                }
            })
            .map(|p| Arc::clone(p))
            .collect()
    }
}
```

### 3. 索引服务 (Index Service)

#### 文件结构
```
src-tauri/src/
├── index/
│   ├── mod.rs
│   ├── app_index.rs        # 应用索引
│   ├── file_index.rs       # 文件索引
│   ├── platform/
│   │   ├── mod.rs
│   │   ├── windows.rs      # Windows 特定
│   │   ├── macos.rs        # macOS 特定
│   │   └── linux.rs        # Linux 特定
```

#### App Index
```rust
// src-tauri/src/index/app_index.rs

use tantivy::*;

pub struct AppIndex {
    index: Index,
    reader: IndexReader,
    writer: Arc<RwLock<IndexWriter>>,
    apps: Arc<RwLock<Vec<AppInfo>>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub icon: WoxImage,
    pub app_type: AppType,
    pub description: Option<String>,
    pub keywords: Vec<String>,
}

impl AppIndex {
    pub async fn new() -> Result<Self> {
        // 创建 Tantivy 索引
        let schema = Self::build_schema();
        let index = Index::create_in_ram(schema);
        let reader = index.reader()?;
        let writer = Arc::new(RwLock::new(index.writer(50_000_000)?));
        
        Ok(Self {
            index,
            reader,
            writer,
            apps: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    fn build_schema() -> Schema {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("name", TEXT | STORED);
        schema_builder.add_text_field("path", STORED);
        schema_builder.add_text_field("description", TEXT);
        schema_builder.add_text_field("keywords", TEXT);
        schema_builder.build()
    }
    
    /// 重建索引
    pub async fn rebuild(&self) -> Result<()> {
        let apps = self.scan_system_apps().await?;
        
        let mut writer = self.writer.write().await;
        writer.delete_all_documents()?;
        
        for app in &apps {
            writer.add_document(self.app_to_document(app)?)?;
        }
        
        writer.commit()?;
        *self.apps.write().await = apps;
        
        Ok(())
    }
    
    /// 搜索应用
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<AppInfo>> {
        // 1. Tantivy 全文搜索
        let searcher = self.reader.searcher();
        // 实现搜索逻辑...
        
        // 2. 模糊匹配（用于短查询）
        let apps = self.apps.read().await;
        let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
        let mut scored: Vec<_> = apps.iter()
            .filter_map(|app| {
                matcher.fuzzy_match(&app.name, query)
                    .map(|score| (app.clone(), score))
            })
            .collect();
        
        scored.sort_by_key(|(_, score)| -score);
        Ok(scored.into_iter().take(limit).map(|(app, _)| app).collect())
    }
    
    #[cfg(target_os = "windows")]
    async fn scan_system_apps(&self) -> Result<Vec<AppInfo>> {
        // 扫描 Windows 应用
        // - Start Menu
        // - UWP Apps
        // - Registry entries
    }
    
    #[cfg(target_os = "macos")]
    async fn scan_system_apps(&self) -> Result<Vec<AppInfo>> {
        // 扫描 macOS 应用
        // - /Applications
        // - ~/Applications
        // - Spotlight metadata
    }
    
    #[cfg(target_os = "linux")]
    async fn scan_system_apps(&self) -> Result<Vec<AppInfo>> {
        // 扫描 Linux 应用
        // - .desktop files
        // - /usr/share/applications
        // - ~/.local/share/applications
    }
}
```

### 4. 热键管理 (Hotkey Manager)

```rust
// src-tauri/src/hotkey/mod.rs

use global_hotkey::*;

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    receiver: GlobalHotKeyEventReceiver,
    registered: RwLock<HashMap<HotKey, HotkeyCallback>>,
}

type HotkeyCallback = Box<dyn Fn() + Send + Sync>;

impl HotkeyManager {
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        let receiver = GlobalHotKeyEvent::receiver();
        
        Ok(Self {
            manager,
            receiver,
            registered: RwLock::new(HashMap::new()),
        })
    }
    
    /// 注册主热键
    pub fn register_main_hotkey<F>(&self, keys: &str, callback: F) -> Result<()>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let hotkey = HotKey::from_str(keys)?;
        self.manager.register(hotkey)?;
        self.registered.write().await.insert(hotkey, Box::new(callback));
        Ok(())
    }
    
    /// 监听热键事件
    pub async fn listen(&self) {
        while let Ok(event) = self.receiver.try_recv() {
            if event.state == HotKeyState::Pressed {
                let registered = self.registered.read().await;
                if let Some(callback) = registered.get(&event.id) {
                    callback();
                }
            }
        }
    }
}
```

### 5. Tauri Commands

```rust
// src-tauri/src/commands/mod.rs

use tauri::State;

#[tauri::command]
pub async fn query(
    input: String,
    engine: State<'_, Arc<LauncherEngine>>
) -> Result<Vec<QueryResult>, String> {
    engine.query(&input).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_action(
    result_id: String,
    action_id: String,
    engine: State<'_, Arc<LauncherEngine>>
) -> Result<(), String> {
    engine.execute_action(&result_id, &action_id).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(
    engine: State<'_, Arc<LauncherEngine>>
) -> Result<Settings, String> {
    Ok(engine.settings.read().await.clone())
}

#[tauri::command]
pub async fn update_setting(
    key: String,
    value: serde_json::Value,
    engine: State<'_, Arc<LauncherEngine>>
) -> Result<(), String> {
    engine.settings.write().await.set(&key, value);
    Ok(())
}

#[tauri::command]
pub async fn get_plugins(
    engine: State<'_, Arc<LauncherEngine>>
) -> Result<Vec<PluginMetadata>, String> {
    engine.plugin_manager.get_all_metadata().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_plugin(
    url_or_path: String,
    engine: State<'_, Arc<LauncherEngine>>
) -> Result<(), String> {
    engine.plugin_manager.install_plugin(&url_or_path).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn uninstall_plugin(
    plugin_id: String,
    engine: State<'_, Arc<LauncherEngine>>
) -> Result<(), String> {
    engine.plugin_manager.uninstall_plugin(&plugin_id).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rebuild_index(
    engine: State<'_, Arc<LauncherEngine>>
) -> Result<(), String> {
    engine.index_service.rebuild_all().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_chat(
    messages: Vec<ChatMessage>,
    engine: State<'_, Arc<LauncherEngine>>
) -> Result<String, String> {
    engine.ai_service.chat(messages).await
        .map_err(|e| e.to_string())
}
```

### 6. 前端实现

#### 主搜索组件
```typescript
// src/components/SearchBox.tsx

import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState, useCallback } from 'react';
import { debounce } from 'lodash-es';

interface QueryResult {
  id: string;
  title: string;
  subtitle: string;
  icon: WoxImage;
  score: number;
  actions: Action[];
  preview?: Preview;
}

export function SearchBox() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<QueryResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [loading, setLoading] = useState(false);
  const [showActionPanel, setShowActionPanel] = useState(false);
  
  // 防抖查询
  const debouncedQuery = useCallback(
    debounce(async (input: string) => {
      if (!input.trim()) {
        setResults([]);
        return;
      }
      
      setLoading(true);
      try {
        const results = await invoke<QueryResult[]>('query', { input });
        setResults(results);
        setSelectedIndex(0);
      } catch (error) {
        console.error('Query failed:', error);
      } finally {
        setLoading(false);
      }
    }, 100),
    []
  );
  
  useEffect(() => {
    debouncedQuery(query);
  }, [query, debouncedQuery]);
  
  const handleKeyDown = async (e: React.KeyboardEvent) => {
    switch (e.key) {
      case 'Enter':
        e.preventDefault();
        await executeDefaultAction();
        break;
        
      case 'ArrowDown':
        e.preventDefault();
        setSelectedIndex(i => Math.min(i + 1, results.length - 1));
        break;
        
      case 'ArrowUp':
        e.preventDefault();
        setSelectedIndex(i => Math.max(i - 1, 0));
        break;
        
      case 'Escape':
        e.preventDefault();
        if (showActionPanel) {
          setShowActionPanel(false);
        } else {
          await invoke('hide_app');
        }
        break;
        
      case 'j':
        if (e.altKey || e.metaKey) {
          e.preventDefault();
          setShowActionPanel(!showActionPanel);
        }
        break;
    }
  };
  
  const executeDefaultAction = async () => {
    const result = results[selectedIndex];
    if (!result) return;
    
    const defaultAction = result.actions.find(a => a.is_default) || result.actions[0];
    if (!defaultAction) return;
    
    try {
      await invoke('execute_action', {
        resultId: result.id,
        actionId: defaultAction.id
      });
      
      if (!defaultAction.prevent_hide) {
        await invoke('hide_app');
      }
    } catch (error) {
      console.error('Action execution failed:', error);
    }
  };
  
  return (
    <div className="search-container">
      <input
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Type to search..."
        className="search-input"
        autoFocus
      />
      
      {loading && <LoadingIndicator />}
      
      <ResultList
        results={results}
        selectedIndex={selectedIndex}
        onSelect={setSelectedIndex}
      />
      
      {showActionPanel && (
        <ActionPanel
          result={results[selectedIndex]}
          onClose={() => setShowActionPanel(false)}
        />
      )}
    </div>
  );
}
```

#### 结果列表
```typescript
// src/components/ResultList.tsx

import { useVirtualizer } from '@tanstack/react-virtual';

export function ResultList({ results, selectedIndex, onSelect }) {
  const parentRef = useRef<HTMLDivElement>(null);
  
  const virtualizer = useVirtualizer({
    count: results.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 60,
    overscan: 5,
  });
  
  return (
    <div ref={parentRef} className="result-list">
      <div
        style={{
          height: `${virtualizer.getTotalSize()}px`,
          position: 'relative',
        }}
      >
        {virtualizer.getVirtualItems().map((virtualItem) => {
          const result = results[virtualItem.index];
          const isSelected = virtualItem.index === selectedIndex;
          
          return (
            <ResultItem
              key={result.id}
              result={result}
              isSelected={isSelected}
              onClick={() => onSelect(virtualItem.index)}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: `${virtualItem.size}px`,
                transform: `translateY(${virtualItem.start}px)`,
              }}
            />
          );
        })}
      </div>
    </div>
  );
}
```

---

## 🗓️ 实施路线图

### Phase 1: 基础框架 (2-3 周)

**目标**: 搭建核心架构，实现基本功能

#### Week 1: 项目初始化
- [ ] 清理现有模板代码
- [ ] 配置 Cargo.toml 依赖
- [ ] 配置 Tauri 窗口（无边框、透明、置顶）
- [ ] 设置 TypeScript 项目结构
- [ ] 配置 Tailwind CSS
- [ ] 创建基础 UI 组件库

#### Week 2: 核心引擎
- [ ] 实现 `LauncherEngine` 骨架
- [ ] 实现 `QueryProcessor` 查询解析
- [ ] 实现基础 Tauri Commands
- [ ] 实现数据库层 (SQLite + sqlx)
- [ ] 实现设置管理系统

#### Week 3: UI 与交互
- [ ] 实现 SearchBox 组件
- [ ] 实现 ResultList 虚拟滚动
- [ ] 实现键盘导航
- [ ] 实现全局热键监听
- [ ] 窗口显示/隐藏动画

**里程碑**: 能够显示窗口，接受输入，响应热键

---

### Phase 2: 核心插件 (3-4 周)

**目标**: 实现最重要的原生插件

#### Week 4: 插件系统框架
- [ ] 定义 `Plugin` trait
- [ ] 实现 `PluginManager`
- [ ] 实现 `PluginAPI`
- [ ] 创建插件注册机制
- [ ] 实现插件生命周期管理

#### Week 5: 应用搜索插件
- [ ] 实现 `AppIndex` 索引服务
- [ ] Windows: 扫描开始菜单、UWP 应用
- [ ] macOS: 扫描 /Applications
- [ ] Linux: 解析 .desktop 文件
- [ ] 实现应用启动功能
- [ ] 图标提取和缓存

#### Week 6: 计算器 & 文件插件
- [ ] 计算器插件（表达式解析、函数支持）
- [ ] 文件搜索插件基础
- [ ] 文件索引服务
- [ ] 文件操作（打开、复制路径等）

#### Week 7: 设置界面
- [ ] 设置界面 UI
- [ ] 通用设置（热键、主题、语言）
- [ ] 插件设置
- [ ] 设置持久化
- [ ] 导入/导出设置

**里程碑**: 可以搜索和启动应用，进行计算，管理设置

---

### Phase 3: 插件生态 (3-4 周)

**目标**: 支持外部插件，构建插件系统

#### Week 8-9: Python 插件支持
- [ ] 研究 PyO3 / 进程通信方案
- [ ] 实现 Python 插件 Host
- [ ] 定义 Python Plugin API
- [ ] 创建 Python SDK 包 (`ilauncher-plugin`)
- [ ] 编写示例插件
- [ ] 测试插件加载和通信

#### Week 10: Node.js & 脚本插件
- [ ] 实现 Node.js 插件 Host（进程通信）
- [ ] 创建 Node.js SDK (`@ilauncher/plugin`)
- [ ] 实现脚本插件 Runner（JSON-RPC）
- [ ] 支持 .py, .js, .sh 脚本插件

#### Week 11: 插件商店
- [ ] 插件商店 UI
- [ ] 插件搜索和浏览
- [ ] 插件安装/卸载
- [ ] 插件更新检查
- [ ] 插件评分和评论（本地）

**里程碑**: 支持加载外部插件，用户可以安装插件

---

### Phase 4: 高级功能 (2-3 周)

**目标**: 实现差异化特性

#### Week 12: AI 集成
- [ ] AI Service 架构
- [ ] OpenAI API 集成
- [ ] Anthropic Claude 集成
- [ ] Ollama 本地模型支持
- [ ] AI 聊天界面
- [ ] AI 辅助查询

#### Week 13: 增强功能
- [ ] 剪贴板历史插件
- [ ] Action Panel UI
- [ ] 预览面板（文本、图片、Markdown）
- [ ] 主题编辑器
- [ ] 多语言支持 (i18n)

#### Week 14: 性能优化
- [ ] 查询性能分析
- [ ] 索引优化
- [ ] 启动速度优化
- [ ] 内存占用优化
- [ ] 并发查询优化

**里程碑**: 功能完整，性能优秀

---

### Phase 5: 打磨发布 (2 周)

**目标**: 准备公开发布

#### Week 15: 跨平台测试
- [ ] Windows 10/11 测试
- [ ] macOS (Intel + Apple Silicon) 测试
- [ ] Linux (Ubuntu, Fedora, Arch) 测试
- [ ] 修复平台特定 Bug
- [ ] 性能基准测试

#### Week 16: 文档和发布
- [ ] 用户文档
- [ ] 插件开发文档
- [ ] API 文档
- [ ] 贡献指南
- [ ] 自动更新机制
- [ ] GitHub Actions CI/CD
- [ ] 打包发布（.exe, .dmg, .AppImage）

**里程碑**: v1.0.0 发布

---

## ✅ 开发任务清单

### 立即开始 (本周)

#### 1. 项目结构重组
```bash
# 清理模板文件
- [ ] 删除 src/App.tsx 模板代码
- [ ] 删除 src/App.css 模板样式
- [ ] 清理 src-tauri/src/lib.rs 示例代码

# 创建新结构
- [ ] 创建 src/components/ 目录
- [ ] 创建 src/hooks/ 目录
- [ ] 创建 src/store/ 目录
- [ ] 创建 src/types/ 目录
- [ ] 创建 src/utils/ 目录

- [ ] 创建 src-tauri/src/core/ 目录
- [ ] 创建 src-tauri/src/plugin/ 目录
- [ ] 创建 src-tauri/src/index/ 目录
- [ ] 创建 src-tauri/src/hotkey/ 目录
- [ ] 创建 src-tauri/src/commands/ 目录
- [ ] 创建 src-tauri/src/database/ 目录
```

#### 2. 依赖安装

**前端依赖**
```bash
npm install -D tailwindcss postcss autoprefixer
npm install zustand
npm install @radix-ui/react-dialog @radix-ui/react-popover
npm install framer-motion
npm install @tanstack/react-virtual
npm install lucide-react
npm install clsx tailwind-merge
```

**后端依赖** (更新 Cargo.toml)
```toml
[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio"] }
tantivy = "0.21"
fuzzy-matcher = "0.3"
global-hotkey = "0.5"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

#### 3. 基础文件创建

- [ ] `src-tauri/src/core/mod.rs` - 核心模块
- [ ] `src-tauri/src/core/types.rs` - 类型定义
- [ ] `src-tauri/src/commands/mod.rs` - Tauri commands
- [ ] `src/types/index.ts` - TypeScript 类型
- [ ] `src/components/SearchBox.tsx` - 搜索框组件
- [ ] `src/store/useAppStore.ts` - 全局状态
- [ ] `tailwind.config.js` - Tailwind 配置

#### 4. Tauri 配置

```json
// src-tauri/tauri.conf.json
{
  "tauri": {
    "windows": [{
      "title": "iLauncher",
      "width": 800,
      "height": 600,
      "decorations": false,
      "transparent": true,
      "alwaysOnTop": true,
      "skipTaskbar": true,
      "center": true,
      "visible": false,
      "resizable": false
    }]
  }
}
```

### 下一步 (本月)

#### 核心功能实现优先级

**P0 - 必须有**
1. [ ] 查询处理引擎
2. [ ] 应用搜索插件
3. [ ] 全局热键
4. [ ] 基础 UI 组件
5. [ ] 设置管理

**P1 - 应该有**
6. [ ] 计算器插件
7. [ ] 文件搜索插件
8. [ ] 插件管理器
9. [ ] 主题系统
10. [ ] 键盘快捷键

**P2 - 可以有**
11. [ ] AI 集成
12. [ ] 剪贴板历史
13. [ ] 预览功能
14. [ ] 插件商店
15. [ ] 自动更新

---

## 📊 技术决策

### 为什么选择 Tauri？

**相比 Wox (Go + Flutter):**

| 对比项 | Wox (Go + Flutter) | iLauncher (Tauri + Rust) |
|--------|-------------------|--------------------------|
| 性能 | 好 | **更好** (Rust 更快) |
| 内存占用 | ~100MB | **~30MB** |
| 包体积 | ~80MB | **~15MB** |
| 启动速度 | 快 | **更快** |
| 系统集成 | 中等 | **优秀** |
| 前端生态 | Flutter | **React/Vue (更丰富)** |
| 开发体验 | 好 | **很好** |
| 跨平台 | 支持 | **原生支持** |

### 架构优势

1. **单一技术栈**: Rust + TypeScript，统一的开发体验
2. **原生性能**: Rust 核心提供极致性能
3. **现代 UI**: React/Tailwind 提供灵活的 UI 开发
4. **安全性**: Rust 的内存安全保证
5. **可扩展性**: 清晰的插件架构

---

## 🎯 成功指标

### 性能目标
- [ ] 启动时间 < 500ms
- [ ] 查询响应 < 50ms (本地)
- [ ] 查询响应 < 200ms (网络)
- [ ] 内存占用 < 50MB (空闲)
- [ ] 索引重建 < 5s (1000 应用)

### 功能目标
- [ ] 支持 10+ 原生插件
- [ ] 支持 Python/Node.js 插件
- [ ] 支持 3 种 AI 提供商
- [ ] 支持 10+ 主题
- [ ] 支持 5+ 语言

### 质量目标
- [ ] 单元测试覆盖率 > 80%
- [ ] 集成测试覆盖核心流程
- [ ] 零 Clippy 警告
- [ ] 完整的文档

---

## 📚 参考资源

### Wox 相关
- [Wox GitHub](https://github.com/Wox-launcher/Wox)
- [Wox 文档](https://wox-launcher.github.io/Wox/#/)
- [Wox 插件开发](https://wox-launcher.github.io/Wox/#/full_featured_plugin_guide)

### Tauri 相关
- [Tauri 官方文档](https://tauri.app/)
- [Tauri Examples](https://github.com/tauri-apps/tauri/tree/dev/examples)
- [Awesome Tauri](https://github.com/tauri-apps/awesome-tauri)

### 技术文档
- [Tantivy 文档](https://docs.rs/tantivy/)
- [SQLx 文档](https://docs.rs/sqlx/)
- [Global Hotkey](https://docs.rs/global-hotkey/)
- [PyO3 文档](https://pyo3.rs/)

---

## 🤝 贡献指南

### 开发流程
1. Fork 项目
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

### 代码规范
- Rust: `cargo fmt` + `cargo clippy`
- TypeScript: ESLint + Prettier
- 提交信息: Conventional Commits

---

## 📝 更新日志

### [Unreleased]
- 项目初始化
- 完成架构设计
- 编写实施计划

---

**最后更新**: 2025-11-03
**版本**: 0.1.0-alpha
**状态**: 规划阶段
