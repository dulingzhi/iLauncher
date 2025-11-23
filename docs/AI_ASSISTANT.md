# AI 助手集成 (Phase 2)

## 功能概述

完整的 AI 对话系统，支持 ChatGPT 和 Claude，提供多轮对话、对话管理和配置界面。

## 核心特性

### 1. 多 AI 提供商支持
- **OpenAI**: GPT-3.5-turbo, GPT-4 等模型
- **Anthropic**: Claude 系列模型
- 可配置自定义 API Base URL

### 2. 对话管理
- **多对话支持**: 创建、切换、删除多个独立对话
- **对话历史**: 自动保存对话记录（内存存储）
- **对话标题**: 自动使用第一条用户消息作为标题

### 3. 消息功能
- **发送消息**: 支持文本输入
- **流式响应**: API 返回完整响应后一次性显示（未来可改为 SSE 流式）
- **上下文保持**: 自动包含对话历史作为上下文

### 4. 配置管理
- **API Key 管理**: 安全存储 API 密钥（前端输入，后端存储）
- **模型选择**: 自定义使用的模型
- **参数调整**: temperature, max_tokens 等（预留接口）

## 技术实现

### 后端 (Rust)

#### `ai_assistant.rs` - AI 助手插件
```rust
pub struct AIAssistantPlugin {
    metadata: PluginMetadata,
    config: Arc<RwLock<AIConfig>>,           // AI 配置
    conversations: Arc<RwLock<Vec<Conversation>>>, // 对话列表
    current_conversation: Arc<RwLock<Option<String>>>, // 当前对话 ID
    client: Client,                           // HTTP 客户端
}

// 核心方法
impl AIAssistantPlugin {
    pub async fn send_message(&self, message: String) -> Result<String>;
    pub async fn create_conversation(&self, title: String) -> String;
    pub async fn get_conversations(&self) -> Vec<Conversation>;
    async fn call_openai_api(&self, config, conv_id) -> Result<String>;
    async fn call_anthropic_api(&self, config, conv_id) -> Result<String>;
}
```

#### API 调用流程

**OpenAI API**:
```
POST https://api.openai.com/v1/chat/completions
Headers:
  Authorization: Bearer {api_key}
  Content-Type: application/json
Body:
  {
    "model": "gpt-3.5-turbo",
    "messages": [
      {"role": "user", "content": "Hello"}
    ],
    "temperature": 0.7,
    "max_tokens": 2000
  }
Response:
  {
    "choices": [{
      "message": {
        "role": "assistant",
        "content": "AI response"
      }
    }]
  }
```

**Anthropic API** (Claude):
```
POST https://api.anthropic.com/v1/messages
Headers:
  x-api-key: {api_key}
  anthropic-version: 2023-06-01
  Content-Type: application/json
Body:
  {
    "model": "claude-3-sonnet-20240229",
    "messages": [
      {"role": "user", "content": "Hello"}
    ],
    "max_tokens": 2000
  }
Response:
  {
    "content": [{
      "text": "AI response"
    }]
  }
```

#### Tauri Commands
```rust
#[tauri::command]
pub async fn get_ai_config() -> AIConfig;

#[tauri::command]
pub async fn save_ai_config(config: AIConfig);

#[tauri::command]
pub async fn send_ai_message(message: String) -> String;

#[tauri::command]
pub async fn create_ai_conversation(title: String) -> String;

#[tauri::command]
pub async fn get_ai_conversations() -> Vec<Conversation>;

#[tauri::command]
pub async fn switch_ai_conversation(conv_id: String);

#[tauri::command]
pub async fn delete_ai_conversation(conv_id: String);
```

### 前端 (React + TypeScript)

#### `AIChat.tsx` - 主聊天界面

**布局**:
```
┌─────────────┬───────────────────────────┐
│  Sidebar    │  Chat Area                │
│             │                           │
│  + New Chat │  ┌─────────────────────┐ │
│             │  │  Message            │ │
│  Conv 1     │  │  from User          │ │
│  Conv 2     │  └─────────────────────┘ │
│  Conv 3     │                           │
│             │  ┌─────────────────────┐ │
│  [Settings] │  │  AI Response        │ │
│             │  └─────────────────────┘ │
│             │                           │
│             │  [Input Box] [Send]      │
└─────────────┴───────────────────────────┘
```

**状态管理**:
```typescript
const [conversations, setConversations] = useState<Conversation[]>([]);
const [currentConvId, setCurrentConvId] = useState<string | null>(null);
const [input, setInput] = useState('');
const [loading, setLoading] = useState(false);
const [showSettings, setShowSettings] = useState(false);
const [config, setConfig] = useState<AIConfig | null>(null);
```

**核心功能**:
- `loadConfig()`: 加载 AI 配置
- `loadConversations()`: 加载对话列表
- `createNewConversation()`: 创建新对话
- `sendMessage()`: 发送消息到 AI
- `deleteConversation()`: 删除对话
- `saveConfig()`: 保存配置

### 插件集成

#### 搜索结果
当用户搜索 "ai"、"gpt"、"chat"、"ask" 时：
```
┌────────────────────────────────────────┐
│ 🤖 Ask AI: how to learn rust         │
│    Send to openai gpt-3.5-turbo       │
│                                        │
│ 💭 Previous Conversation 1             │
│    user: Hello...                      │
│                                        │
│ 💭 Previous Conversation 2             │
│    user: Help me...                    │
└────────────────────────────────────────┘
```

#### 操作 (Actions)
- **Send Message**: 发送到 AI 并打开聊天界面
- **Open Conversation**: 打开历史对话
- **Delete**: 删除对话

#### 沙盒权限
```rust
SandboxConfig {
    plugin_id: "ai_assistant",
    security_level: SecurityLevel::Restricted,
    custom_permissions: vec![
        PluginPermission::NetworkAccess(Domain("api.openai.com")),
        PluginPermission::NetworkAccess(Domain("api.anthropic.com")),
        PluginPermission::ClipboardAccess,
        PluginPermission::SystemInfoRead,
    ],
    timeout_ms: Some(60000), // 60秒超时（AI 响应可能较慢）
    max_memory_mb: Some(200),
}
```

## 使用指南

### 配置 API Key

1. 搜索 "ai" 打开 AI 助手
2. 点击左下角 "Settings"
3. 选择 Provider (OpenAI / Anthropic)
4. 输入 API Key
5. 输入 Model 名称 (如 `gpt-3.5-turbo` 或 `claude-3-sonnet-20240229`)
6. 点击 "Save"

### 开始对话

1. 搜索 "ai your question" 或点击 "+ New Chat"
2. 输入消息并按回车或点击发送按钮
3. 等待 AI 响应（显示加载动画）
4. 继续对话以保持上下文

### 管理对话

- **切换对话**: 点击侧边栏的对话项
- **删除对话**: 悬停在对话上，点击垃圾桶图标
- **创建新对话**: 点击 "+ New Chat" 按钮

## API Key 获取

### OpenAI
1. 访问 https://platform.openai.com/api-keys
2. 登录账号
3. 点击 "Create new secret key"
4. 复制 key（格式: `sk-...`）

### Anthropic (Claude)
1. 访问 https://console.anthropic.com/settings/keys
2. 登录账号
3. 点击 "Create Key"
4. 复制 key

## 支持的模型

### OpenAI Models
- `gpt-3.5-turbo` (推荐, 性价比高)
- `gpt-3.5-turbo-16k` (长上下文)
- `gpt-4` (最强能力)
- `gpt-4-turbo-preview` (更快的 GPT-4)
- `gpt-4o` (多模态, 需要额外权限)

### Anthropic Models (Claude)
- `claude-3-haiku-20240307` (最快, 最便宜)
- `claude-3-sonnet-20240229` (推荐, 平衡)
- `claude-3-opus-20240229` (最强)
- `claude-3-5-sonnet-20241022` (最新, 最强)

## 代码统计

- **新增文件**: 
  - `ai_assistant.rs` (570 行)
  - `commands/ai.rs` (93 行)
  - `AIChat.tsx` (440 行)
  - `AI_ASSISTANT.md` (本文档)
- **修改文件**: 
  - `plugin/mod.rs` (+25 行，注册插件 + 沙盒配置)
  - `commands/mod.rs` (+1 行，导入 ai 模块)
  - `lib.rs` (+7 行，注册 commands)
  - `App.tsx` (+5 行，添加 AI Chat 视图)
- **总计**: ~1,140 行新增代码

## 性能指标

- **API 调用延迟**: 2-10秒（取决于模型和网络）
- **超时设置**: 60秒
- **内存占用**: ~50MB（对话历史）
- **并发限制**: 单个请求（串行处理）

## 安全考虑

### API Key 存储
- ⚠️ **当前**: 明文存储在内存中（`AIConfig`）
- 🔒 **建议**: 使用系统密钥链加密存储（未来优化）
  - Windows: Windows Credential Manager
  - macOS: Keychain
  - Linux: Secret Service API

### 网络隔离
- 沙盒限制只能访问 `api.openai.com` 和 `api.anthropic.com`
- 其他网络请求会被拒绝

### 数据隐私
- 对话历史仅存储在内存中，应用重启后清空
- API 调用数据由 OpenAI/Anthropic 处理，遵守其隐私政策

## 已知限制

1. **无流式响应**: 当前等待完整响应后一次性显示（计划支持 SSE）
2. **对话未持久化**: 重启应用后对话历史丢失（计划添加数据库）
3. **无多模态支持**: 仅支持文本，不支持图片输入/输出
4. **无语音输入**: 不支持语音转文字
5. **无联网搜索**: AI 无法访问实时信息（可集成搜索插件）

## 未来优化方向

### 短期 (Phase 3)
1. **对话持久化**: 存储到 SQLite 数据库
2. **流式响应**: 支持 SSE 流式输出
3. **Markdown 渲染**: 渲染 AI 响应中的代码块、列表等
4. **复制消息**: 一键复制 AI 响应
5. **导出对话**: 导出为 Markdown/PDF

### 中期
1. **多模态支持**: 支持 GPT-4V、Claude 图片输入
2. **语音输入**: 集成 Whisper API
3. **插件联动**: AI 可调用其他插件（如搜索、计算器）
4. **系统提示词**: 自定义 AI 角色和行为
5. **快捷提示**: 预设常用提示词模板

### 长期
1. **本地模型**: 集成 Ollama/LLaMA 本地运行
2. **知识库**: RAG 文档检索增强
3. **多智能体**: 多个 AI 协作完成任务
4. **工作流**: 构建自动化 AI 工作流
5. **插件市场**: 社区贡献 AI 插件

## 故障排查

### API Key 无效
- **错误**: "API key not configured" 或 401 Unauthorized
- **解决**: 检查 API Key 是否正确，是否有余额

### 网络超时
- **错误**: "Request timeout" 或连接失败
- **解决**: 检查网络连接，尝试使用代理或 Base URL

### 模型不存在
- **错误**: "Model not found" 或 404
- **解决**: 确认模型名称正确，检查账号是否有权限

### 响应为空
- **错误**: AI 返回空消息
- **解决**: 检查 max_tokens 设置，可能太小导致截断

## 示例对话

```
User: 用 Rust 实现快速排序
AI: 这是一个 Rust 快速排序实现：

​```rust
fn quicksort<T: Ord>(arr: &mut [T]) {
    if arr.len() <= 1 {
        return;
    }
    let pivot = partition(arr);
    quicksort(&mut arr[0..pivot]);
    quicksort(&mut arr[pivot + 1..]);
}

fn partition<T: Ord>(arr: &mut [T]) -> usize {
    let len = arr.len();
    let pivot = len / 2;
    arr.swap(pivot, len - 1);
    let mut i = 0;
    for j in 0..len - 1 {
        if arr[j] < arr[len - 1] {
            arr.swap(i, j);
            i += 1;
        }
    }
    arr.swap(i, len - 1);
    i
}
​```

这个实现使用了原地分区算法...
```

---

**开发时间**: Phase 2 完成  
**下一步**: Phase 3 - 插件市场
