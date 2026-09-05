# iLauncher Plugin Package Format (.ilp)

## 概述

`.ilp` (iLauncher Plugin) 是 iLauncher 的标准插件包格式，使用 ZIP 压缩打包。

## 包结构

```
my-plugin.ilp (ZIP archive)
├── manifest.json          # 插件元数据 (必需)
├── plugin.wasm           # WASM 插件代码 (可选)
├── plugin.js             # JavaScript 插件代码 (可选)
├── icon.png              # 插件图标 (必需, 128x128)
├── README.md             # 说明文档 (推荐)
├── LICENSE               # 许可证 (推荐)
├── signature.sig         # RSA 签名 (生产环境必需)
└── resources/            # 资源文件 (可选)
    ├── icons/
    ├── data/
    └── templates/
```

## manifest.json 规范

```json
{
  "id": "com.example.my-plugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "description": "A sample plugin",
  "author": {
    "name": "John Doe",
    "email": "john@example.com",
    "url": "https://example.com"
  },
  "homepage": "https://github.com/example/my-plugin",
  "repository": {
    "type": "git",
    "url": "https://github.com/example/my-plugin"
  },
  "license": "MIT",
  "keywords": ["search", "utility", "productivity"],
  "icon": "icon.png",
  
  "engine": {
    "type": "wasm",
    "entry": "plugin.wasm",
    "runtime_version": ">=0.1.0"
  },
  
  "triggers": ["mp", "myplugin"],
  
  "permissions": [
    "network:api.example.com",
    "filesystem:read:~/Documents",
    "clipboard:read",
    "clipboard:write",
    "system:info"
  ],
  
  "sandbox": {
    "level": "restricted",
    "timeout_ms": 5000,
    "max_memory_mb": 50
  },
  
  "settings": [
    {
      "key": "api_key",
      "type": "string",
      "label": "API Key",
      "description": "Your API key",
      "required": true,
      "secret": true
    },
    {
      "key": "max_results",
      "type": "number",
      "label": "Max Results",
      "default": 10,
      "min": 1,
      "max": 100
    }
  ],
  
  "dependencies": [
    {
      "id": "com.ilauncher.core-utils",
      "version": "^1.0.0"
    }
  ],
  
  "changelog": {
    "1.0.0": ["Initial release"],
    "0.9.0": ["Beta version"]
  }
}
```

## 字段说明

### 基础信息

- **id** (string, 必需): 插件唯一标识符，推荐格式 `com.author.plugin-name`
- **name** (string, 必需): 插件显示名称
- **version** (string, 必需): 语义化版本号 (semver)
- **description** (string, 必需): 简短描述
- **author** (object, 必需): 作者信息
  - `name`: 姓名
  - `email`: 邮箱
  - `url`: 个人主页
- **homepage** (string, 可选): 插件主页
- **repository** (object, 可选): 源代码仓库
- **license** (string, 必需): 许可证 (如 MIT, Apache-2.0)
- **keywords** (array, 可选): 搜索关键词
- **icon** (string, 必需): 图标文件路径

### 引擎配置

- **engine.type** (enum): 插件类型
  - `wasm`: WebAssembly 插件
  - `javascript`: JavaScript 插件
  - `native`: Rust 原生插件（仅官方）
- **engine.entry** (string): 入口文件路径
- **engine.runtime_version** (string): 最低运行时版本

### 触发器

- **triggers** (array): 触发词列表，用于搜索匹配

### 权限

- **permissions** (array): 权限列表
  - `network:<domain>`: 网络访问
  - `filesystem:read:<path>`: 文件系统读取
  - `filesystem:write:<path>`: 文件系统写入
  - `clipboard:read`: 剪贴板读取
  - `clipboard:write`: 剪贴板写入
  - `system:info`: 系统信息读取
  - `system:execute`: 执行外部命令
  - `database:read`: 数据库读取
  - `database:write`: 数据库写入

### 沙盒配置

- **sandbox.level** (enum): 安全级别
  - `none`: 无限制（不推荐）
  - `basic`: 基础隔离
  - `restricted`: 限制访问
  - `strict`: 严格隔离
- **sandbox.timeout_ms** (number): 超时时间（毫秒）
- **sandbox.max_memory_mb** (number): 最大内存（MB）

### 设置

- **settings** (array): 插件配置项
  - `key`: 配置键名
  - `type`: 类型 (string, number, boolean, enum)
  - `label`: 显示标签
  - `description`: 说明
  - `required`: 是否必需
  - `secret`: 是否为密钥（隐藏显示）
  - `default`: 默认值
  - `min`/`max`: 数值范围

### 依赖

- **dependencies** (array): 依赖的其他插件
  - `id`: 插件 ID
  - `version`: 版本要求（支持 semver 语法）

## 签名验证

### 签名生成

```bash
# 1. 生成 RSA 密钥对
openssl genrsa -out private.pem 2048
openssl rsa -in private.pem -pubout -out public.pem

# 2. 对插件包签名（排除 signature.sig）
zip -r my-plugin-unsigned.zip manifest.json plugin.wasm icon.png
openssl dgst -sha256 -sign private.pem -out signature.sig my-plugin-unsigned.zip

# 3. 将签名添加到包中
zip my-plugin.ilp -r manifest.json plugin.wasm icon.png signature.sig
```

### 签名验证流程

```rust
fn verify_plugin_signature(ilp_path: &Path, public_key: &RsaPublicKey) -> Result<bool> {
    // 1. 提取 ZIP 内容
    let mut archive = ZipArchive::new(File::open(ilp_path)?)?;
    
    // 2. 读取签名文件
    let mut signature_file = archive.by_name("signature.sig")?;
    let mut signature = Vec::new();
    signature_file.read_to_end(&mut signature)?;
    
    // 3. 重新打包除签名外的所有文件
    let temp_zip = create_temp_zip_without_signature(&archive)?;
    
    // 4. 计算哈希并验证
    let hash = sha256_file(&temp_zip)?;
    public_key.verify(Pkcs1v15Sign::new::<Sha256>(), &hash, &signature).is_ok()
}
```

## 插件 API

### Plugin Trait (Rust)

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> &PluginMetadata;
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>>;
    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()>;
    async fn on_install(&self) -> Result<()> { Ok(()) }
    async fn on_uninstall(&self) -> Result<()> { Ok(()) }
}
```

### WASM Interface

```rust
// WASM 插件导出函数
#[no_mangle]
pub extern "C" fn plugin_init() -> *const PluginMetadata;

#[no_mangle]
pub extern "C" fn plugin_query(query: *const u8, len: usize) -> *const QueryResults;

#[no_mangle]
pub extern "C" fn plugin_execute(result_id: *const u8, action_id: *const u8) -> i32;
```

### JavaScript Interface

```javascript
// JavaScript 插件导出对象
export default {
  metadata: {
    id: "com.example.my-plugin",
    name: "My Plugin",
    triggers: ["mp"],
  },
  
  async query(context) {
    return [
      {
        id: "result-1",
        title: "Result 1",
        subtitle: "Description",
        icon: "icon.png",
        actions: [
          { id: "open", label: "Open" },
        ],
      },
    ];
  },
  
  async execute(resultId, actionId) {
    if (actionId === "open") {
      // Do something
    }
  },
};
```

## 发布流程

### 1. 开发插件

```bash
# 创建项目
cargo new --lib my-plugin
cd my-plugin

# 添加依赖
cargo add ilauncher-plugin-api
```

### 2. 构建 WASM

```bash
# 安装 wasm-pack
cargo install wasm-pack

# 构建 WASM
wasm-pack build --target web
```

### 3. 打包插件

```bash
# 创建 manifest.json
cat > manifest.json <<EOF
{
  "id": "com.example.my-plugin",
  "name": "My Plugin",
  ...
}
EOF

# 打包
zip -r my-plugin.ilp manifest.json plugin.wasm icon.png README.md LICENSE
```

### 4. 签名

```bash
# 签名（排除 signature.sig）
openssl dgst -sha256 -sign private.pem -out signature.sig my-plugin.ilp

# 添加签名
zip my-plugin.ilp signature.sig
```

### 5. 发布到市场

```bash
# 上传到 GitHub Release
gh release create v1.0.0 my-plugin.ilp

# 或提交到官方市场
curl -X POST https://plugins.ilauncher.com/api/submit \
  -F "plugin=@my-plugin.ilp" \
  -F "public_key=@public.pem"
```

## 插件市场 API

### 获取插件列表

```
GET https://plugins.ilauncher.com/api/plugins
Query Parameters:
  - q: 搜索关键词
  - category: 分类
  - sort: 排序 (downloads, rating, date)
  - page: 页码
  - per_page: 每页数量

Response:
{
  "total": 100,
  "page": 1,
  "per_page": 20,
  "plugins": [
    {
      "id": "com.example.my-plugin",
      "name": "My Plugin",
      "version": "1.0.0",
      "description": "...",
      "author": "John Doe",
      "downloads": 1000,
      "rating": 4.5,
      "icon_url": "https://...",
      "download_url": "https://..."
    }
  ]
}
```

### 获取插件详情

```
GET https://plugins.ilauncher.com/api/plugins/{id}

Response:
{
  "id": "com.example.my-plugin",
  "manifest": { ... },
  "readme": "...",
  "versions": ["1.0.0", "0.9.0"],
  "statistics": {
    "downloads": 1000,
    "rating": 4.5,
    "reviews": 10
  }
}
```

### 下载插件

```
GET https://plugins.ilauncher.com/api/plugins/{id}/download?version=1.0.0

Response: (Binary .ilp file)
```

## 安全审查

所有提交到官方市场的插件需通过以下审查：

1. **代码审查**: 检查恶意代码、后门
2. **权限审查**: 确认权限使用合理
3. **性能测试**: 资源占用、响应时间
4. **安全扫描**: 漏洞扫描、依赖检查
5. **签名验证**: 验证开发者身份

## 最佳实践

1. **最小权限原则**: 只申请必需的权限
2. **错误处理**: 优雅处理所有错误情况
3. **性能优化**: 避免阻塞操作，使用异步
4. **用户体验**: 提供清晰的图标、描述和设置
5. **更新日志**: 记录每个版本的变更
6. **文档完善**: 提供详细的 README 和示例

## 参考

- [插件开发指南](PLUGIN_DEVELOPMENT.md)
- [沙盒权限详解](SANDBOX_PERMISSIONS.md)
- [WASM 插件开发](WASM_PLUGIN_GUIDE.md)
- [官方插件市场](https://plugins.ilauncher.com)
