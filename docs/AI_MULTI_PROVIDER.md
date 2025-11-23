# AI助手增强 - 多模型支持

## 🎉 新增功能

AI助手现已支持 **7种主流AI模型**，包括GitHub Copilot集成！

### 支持的AI Provider

| Provider | 模型示例 | API密钥获取 | 特点 |
|---------|---------|-----------|-----|
| **OpenAI** | `gpt-3.5-turbo`, `gpt-4`, `gpt-4-turbo` | [platform.openai.com](https://platform.openai.com) | 最流行，响应快 |
| **Anthropic** | `claude-3-opus-20240229`, `claude-3-sonnet-20240229` | [console.anthropic.com](https://console.anthropic.com) | 长上下文，逻辑强 |
| **GitHub Copilot** | `gpt-4`, `copilot-chat` | [github.com/settings/tokens](https://github.com/settings/tokens) | GitHub订阅用户免费 |
| **DeepSeek** | `deepseek-chat`, `deepseek-coder` | [platform.deepseek.com](https://platform.deepseek.com) | 编程专用，便宜 |
| **Google Gemini** | `gemini-pro`, `gemini-ultra` | [makersuite.google.com](https://makersuite.google.com) | 多模态，免费额度 |
| **Ollama** | `llama2`, `mistral`, `codellama` | 无需API密钥 | 本地运行，隐私优先 |
| **Custom** | 自定义 | 自定义 | OpenAI兼容接口 |

---

## 🔧 配置步骤

### 1. OpenAI配置

```
Provider: OpenAI
API Key: sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
Model: gpt-3.5-turbo (或 gpt-4)
Base URL: (留空，使用默认)
```

**获取API密钥**:
1. 访问 https://platform.openai.com/api-keys
2. 登录账户
3. 点击 "Create new secret key"
4. 复制密钥并保存（只显示一次）

---

### 2. GitHub Copilot配置 ⭐

**前置条件**:
- GitHub账户已订阅 GitHub Copilot ($10/月)
- 或者学生/开源维护者免费使用

**配置步骤**:

```
Provider: GitHub Copilot
API Key: ghu_xxxxxxxxxxxxxxxxxxxx (GitHub Token)
Model: gpt-4 (或 copilot-chat)
Base URL: https://api.githubcopilot.com
```

**获取Token**:
1. 访问 https://github.com/settings/tokens
2. 点击 "Generate new token (classic)"
3. 勾选权限：
   - ✅ `copilot` (必选)
   - ✅ `read:user` (可选)
4. 点击 "Generate token"
5. 复制生成的 `ghu_` 开头的token

**使用限制**:
- 需要有效的 GitHub Copilot 订阅
- 每分钟请求限制：20次
- 每月上下文限制：根据订阅计划

---

### 3. Anthropic (Claude)配置

```
Provider: Anthropic (Claude)
API Key: sk-ant-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
Model: claude-3-opus-20240229
Base URL: (留空)
```

**推荐模型**:
- `claude-3-opus-20240229` - 最强大，最贵
- `claude-3-sonnet-20240229` - 性价比高
- `claude-3-haiku-20240307` - 最快，最便宜

---

### 4. DeepSeek配置

```
Provider: DeepSeek
API Key: sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
Model: deepseek-chat (或 deepseek-coder)
Base URL: https://api.deepseek.com
```

**特点**:
- 价格极低（比OpenAI便宜10倍）
- `deepseek-coder` 专为编程优化
- 支持中文理解

---

### 5. Google Gemini配置

```
Provider: Google Gemini
API Key: AIzaSyxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
Model: gemini-pro
Base URL: (留空)
```

**免费额度**:
- 每分钟60次请求
- 每天1,500次请求

---

### 6. Ollama本地配置

**安装Ollama**:
```bash
# Windows (PowerShell)
winget install Ollama.Ollama

# macOS
brew install ollama

# Linux
curl https://ollama.ai/install.sh | sh
```

**下载模型**:
```bash
ollama pull llama2        # Meta LLaMA 2 (7B)
ollama pull mistral       # Mistral 7B
ollama pull codellama     # Code LLaMA
ollama pull llama3:70b    # LLaMA 3 70B (需要大内存)
```

**配置iLauncher**:
```
Provider: Ollama (Local)
API Key: (留空，本地无需密钥)
Model: llama2 (或其他已下载的模型)
Base URL: http://localhost:11434
```

**优点**:
- 完全离线运行
- 无使用限制
- 隐私保护
- 免费

---

## 🎨 高级配置

### Temperature设置

控制生成文本的随机性：

- **0.0** - 完全确定性（适合代码生成）
- **0.3-0.5** - 偏保守（适合技术文档）
- **0.7** - 平衡（默认，适合对话）
- **1.0-2.0** - 创意性高（适合创作）

### Max Tokens设置

控制响应长度：

- **100-500** - 简短回答
- **1000-2000** - 标准对话（默认）
- **4000-8000** - 长文生成

**注意**: tokens数量影响成本和速度

---

## 💡 使用技巧

### 1. GitHub Copilot最佳实践

```
# 代码解释
ai 解释这段代码: function fibonacci(n) { ... }

# 代码优化
ai 优化以下代码的性能: [粘贴代码]

# Bug修复
ai 这段代码为什么报错: [粘贴错误信息]

# 代码生成
ai 写一个React组件用于显示用户列表
```

### 2. 切换模型策略

- **快速查询** → `gpt-3.5-turbo` 或 `llama2` (本地)
- **复杂任务** → `gpt-4` 或 `claude-3-opus`
- **代码相关** → `deepseek-coder` 或 `codellama`
- **长文本** → `claude-3` (200K上下文)

### 3. 本地优先策略

对于日常查询，优先使用Ollama本地模型：
1. 节省成本
2. 响应更快（无网络延迟）
3. 隐私保护

复杂任务时切换到云端模型。

---

## 🔒 安全建议

1. **不要在代码中硬编码API密钥**
2. **定期轮换API密钥**
3. **监控API使用量和成本**
4. **GitHub Token权限最小化**（只勾选必要的scope）
5. **使用Ollama处理敏感信息**（避免发送到云端）

---

## 📊 成本对比

| Provider | 价格 (每1M tokens) | 适合场景 |
|---------|------------------|---------|
| OpenAI GPT-3.5 | $0.50 (输入) $1.50 (输出) | 日常对话 |
| OpenAI GPT-4 | $30 (输入) $60 (输出) | 复杂任务 |
| Claude 3 Opus | $15 (输入) $75 (输出) | 长文本分析 |
| Claude 3 Sonnet | $3 (输入) $15 (输出) | 性价比首选 |
| DeepSeek Chat | $0.14 (输入) $0.28 (输出) | 预算有限 |
| GitHub Copilot | $10/月无限 | 已有订阅用户 |
| Ollama | 免费 | 本地硬件足够 |

---

## 🐛 故障排查

### GitHub Copilot认证失败

**错误**: `401 Unauthorized`

**解决方案**:
1. 检查Token是否勾选 `copilot` scope
2. 确认GitHub Copilot订阅有效
3. Token可能已过期，重新生成

### Ollama连接失败

**错误**: `Failed to connect to localhost:11434`

**解决方案**:
```bash
# 检查Ollama是否运行
ollama list

# 启动Ollama服务
ollama serve

# Windows: 检查防火墙设置
```

### API速率限制

**错误**: `429 Too Many Requests`

**解决方案**:
1. 降低请求频率
2. 升级API计划
3. 切换到本地Ollama

---

## 📝 示例对话

### 编程助手
```
User: ai 用Rust实现一个LRU缓存
AI: [完整代码实现]

User: 继续 解释一下这个算法的时间复杂度
AI: [详细分析]
```

### 文档生成
```
User: ai 为以下函数生成JSDoc注释: [粘贴代码]
AI: [生成的注释]
```

### 代码审查
```
User: ai review 这段代码有什么问题: [粘贴代码]
AI: [指出潜在问题和改进建议]
```

---

## 🎯 下一步计划

- [ ] 流式响应支持（实时显示生成过程）
- [ ] 代码片段识别和高亮
- [ ] 多轮对话上下文优化
- [ ] 预设Prompt模板（代码审查、重构、解释等）
- [ ] 支持上传图片（多模态）
- [ ] 对话导出（Markdown/PDF）

---

**更新日期**: 2025-01-24  
**版本**: v0.1.0+ai-multi-provider
