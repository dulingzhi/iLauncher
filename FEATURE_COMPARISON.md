# iLauncher vs Wox 功能对比清单

> 对比基于 Wox 2.0.0-beta.6 版本

## ✅ 已实现功能

### 核心功能
- ✅ 全局热键触发 (Alt/Cmd + Space)
- ✅ 模糊搜索引擎
- ✅ 插件系统架构
- ✅ 键盘导航 (↑↓ Enter Esc)
- ✅ 设置界面
- ✅ 插件管理器
- ✅ 主题系统（内置多款主题）
- ✅ 自定义主题编辑器
- ✅ 预览面板（文件预览）
- ✅ 国际化支持 (i18n)
- ✅ 系统托盘图标

### 系统插件
- ✅ 应用搜索 (App Search)
- ✅ 文件搜索 (File Search with MFT)
- ✅ 计算器 (Calculator)
- ✅ 单位转换 (Unit Converter)
- ✅ 网页搜索 (Web Search)
- ✅ 剪贴板历史 (Clipboard History)
- ✅ 设置快捷打开 (Settings Plugin)
- ✅ 插件管理快捷打开 (Plugin Manager Plugin)

### UI/UX
- ✅ 无边框透明窗口
- ✅ 始终置顶
- ✅ 自动隐藏（失焦）
- ✅ 防抖查询
- ✅ 主题实时预览
- ✅ 响应式布局
- ✅ Toast 通知

## 🚧 部分实现功能

### 文件搜索
- ✅ MFT 扫描 (450万+ 文件，9秒扫描)
- ✅ 实时索引更新 (USN Journal)
- ⚠️ 需要管理员权限（Wox 也需要）
- ❌ Everything 集成（Windows）

### 剪贴板
- ✅ 文本历史记录
- ⚠️ 基础功能实现
- ❌ 图片历史记录
- ❌ 文件历史记录
- ❌ 收藏功能
- ❌ 固定条目

### 主题系统
- ✅ 多款内置主题
- ✅ 自定义主题编辑器
- ✅ 主题实时预览
- ⚠️ 主题导入/导出（UI存在但未完全实现）
- ❌ AI 生成主题

## ❌ 缺失功能

### 查询功能
- ❌ **Selection Query** (选中文本/文件后查询)
  - 快捷键：Ctrl+Alt+Space (Win) / Opt+Cmd+Space (Mac)
  - 用途：对选中内容执行操作
- ❌ **Clip Query** (PopClip/SnipDo 集成)
  - 鼠标点击图标触发
- ❌ **Query Shortcuts** (查询快捷方式)
  - 例如：`sh` → `llm shell`
- ❌ **Query Hotkeys** (自定义查询热键)
  - 例如：Ctrl+Shift+S → 执行特定查询
  - 支持变量：{wox:selected_text}, {wox:active_browser_url}, {wox:file_explorer_path}
- ❌ **Silent Mode** (静默模式)
  - 单一结果直接执行，无UI显示

### AI 功能
- ❌ **AI Chat** (AI 聊天集成)
  - 支持 MCP (Model Context Protocol)
  - 多工具同时调用
  - 自定义 AI Agents
- ❌ **AI Command** (AI 命令处理)
  - 智能命令解析
  - 上下文理解
- ❌ **AI Theme Generator** (AI 主题生成)
  - 基于描述生成主题

### 插件系统
- ❌ **Script Plugin** 支持
  - 单文件轻量插件
  - Python/JavaScript/Bash 脚本
  - JSON-RPC 通信
  - 元数据注释定义
  - 即时生效，无需重启
- ❌ **Plugin Host** 架构
  - Python 插件宿主
  - Node.js 插件宿主
  - WebSocket 通信
- ❌ **Plugin Store** (插件商店)
  - 在线浏览插件
  - 一键安装/更新
  - 插件评分/评论
- ❌ **Plugin API** 高级功能
  - 自定义设置界面
  - 插件间通信
  - 状态持久化

### 系统插件缺失
- ❌ **Browser Bookmark** (浏览器书签)
  - Chrome/Firefox/Edge/Safari/Arc 支持
  - 跨平台同步
- ❌ **Media Player** (媒体播放器控制)
  - Spotify/Music 集成
- ❌ **Shell Command** (Shell 命令执行)
  - 历史记录管理
  - 输出捕获
- ❌ **Converter** (高级转换器)
  - 加密/解密模块
  - 货币转换（实时汇率）
  - 时间转换（时区）
  - 数学表达式
- ❌ **Query History** (查询历史)
  - MRU (Most Recently Used)
  - 历史搜索
- ❌ **Backup & Restore** (备份恢复)
  - 设置备份
  - 插件配置备份
- ❌ **Doctor** (诊断工具)
  - 系统检查
  - 插件诊断
- ❌ **Indicator** (指示器)
  - 系统状态显示
- ❌ **Theme Manager** (主题管理器)
  - 主题商店
  - 在线主题

### 高级功能
- ❌ **Deep Link** 支持
  - wox://query?q=xxx
  - wox://plugin/xxx
  - URL Scheme 集成
- ❌ **Auto Start** (开机自启)
  - 跨平台实现
  - 注册表/启动项管理
- ❌ **Update Manager** (自动更新)
  - 版本检测
  - 增量更新
  - 回滚支持
- ❌ **Action Filtering** (操作过滤)
  - 高级结果排序
  - 智能评分
- ❌ **Result Grouping** (结果分组)
  - 按类型分组
  - 自定义分组
- ❌ **Quick Select** (快速选择)
  - 数字/字母快捷键选择结果
- ❌ **Last Display Position** (记住窗口位置)
  - 恢复上次位置
- ❌ **MRU Mode** (最近使用模式)
  - 打开时显示 MRU 结果
- ❌ **Last Query Mode** (上次查询模式)
  - 保留上次查询 vs 清空

### UI/UX 增强
- ❌ **Preview Panel 扩展**
  - 图片预览
  - HTML 预览
  - 自定义预览
- ❌ **Context Menu** (上下文菜单)
  - 右键菜单
  - 额外操作
- ❌ **Double Modifier Hotkey** (双击修饰键)
  - 例如：双击 Ctrl
- ❌ **Transparent Display Optimization** (透明度优化)
  - Windows 特别优化
- ❌ **Focus Management** (焦点管理)
  - 更好的焦点控制
  - 失焦后恢复

### 插件商店
- ❌ Obsidian 插件
- ❌ Emoji 搜索插件
- ❌ Sum Selection Numbers 插件
- ❌ DeepL 翻译插件
- ❌ Arc 浏览器标签页插件
- ❌ macOS 音量控制插件
- ❌ Moodist 环境音插件
- ❌ Spotify 集成插件
- ❌ RSS 阅读器插件
- ❌ Custom Commands 插件
- ❌ 每日60秒新闻插件

## 📊 功能覆盖率

### 总体覆盖率
- **核心功能**: ~60% (基础功能完善，高级功能缺失)
- **系统插件**: ~40% (基础插件有，高级插件缺)
- **AI 功能**: ~0% (完全未实现)
- **插件生态**: ~10% (架构有，但无商店和脚本支持)
- **UI/UX**: ~70% (基础完善，缺少高级交互)

### 优势功能
1. ✨ **MFT 文件搜索**: 性能极佳（9秒扫描450万文件）
2. ✨ **主题编辑器**: 可视化编辑，实时预览
3. ✨ **预览面板**: 文件内容预览（代码高亮、Markdown）
4. ✨ **现代化 UI**: React + Tailwind，响应式设计

### 需要优先开发的功能

#### 🔴 高优先级
1. **Selection Query** (选中查询)
   - 影响: 核心交互方式缺失
   - 难度: 中等
   - 时间: 2-3天

2. **Script Plugin 支持**
   - 影响: 插件生态扩展性
   - 难度: 中等
   - 时间: 3-5天

3. **Plugin Store** (插件商店)
   - 影响: 用户可扩展性
   - 难度: 高
   - 时间: 1-2周

4. **Browser Bookmark** 插件
   - 影响: 常用功能
   - 难度: 中等
   - 时间: 2-3天

5. **Auto Start** (开机自启)
   - 影响: 用户体验
   - 难度: 低
   - 时间: 1天

#### 🟡 中优先级
6. **AI Chat** 集成
   - 影响: 现代化功能
   - 难度: 高
   - 时间: 1-2周

7. **Query Shortcuts** (查询快捷方式)
   - 影响: 效率提升
   - 难度: 低
   - 时间: 1-2天

8. **Quick Select** (快速选择)
   - 影响: 键盘效率
   - 难度: 低
   - 时间: 1天

9. **Converter 插件扩展**
   - 影响: 实用性
   - 难度: 中等
   - 时间: 3-5天

10. **Update Manager** (自动更新)
    - 影响: 维护性
    - 难度: 中等
    - 时间: 3-5天

#### 🟢 低优先级
11. **AI Theme Generator**
12. **Deep Link 完善**
13. **Media Player 控制**
14. **Result Grouping**
15. **更多插件商店插件**

## 📝 开发路线图建议

### Phase 1: 核心功能完善 (2-3周)
- [ ] Selection Query
- [ ] Auto Start
- [ ] Query Shortcuts
- [ ] Quick Select
- [ ] 剪贴板功能完善（图片、文件、收藏）

### Phase 2: 插件生态 (3-4周)
- [ ] Script Plugin 支持
- [ ] Plugin Store
- [ ] Browser Bookmark 插件
- [ ] Converter 插件扩展
- [ ] Shell Command 插件

### Phase 3: AI 集成 (2-3周)
- [ ] AI Chat 基础
- [ ] MCP 支持
- [ ] AI Command
- [ ] (可选) AI Theme Generator

### Phase 4: 高级功能 (2-3周)
- [ ] Update Manager
- [ ] Deep Link 完善
- [ ] Query Hotkeys with Variables
- [ ] Media Player 控制
- [ ] Result Grouping

### Phase 5: 生态扩展 (持续)
- [ ] 插件商店插件移植
- [ ] 社区贡献指南
- [ ] 插件开发文档
- [ ] 示例插件

## 🎯 竞争优势

iLauncher 可以在以下方面超越 Wox：

1. **性能**: Rust 核心 + MFT 优化
2. **现代化**: React 18 + Tailwind CSS
3. **跨平台**: 统一的 Tauri 架构
4. **内存占用**: < 50MB (Wox ~100MB+)
5. **启动速度**: 更快的冷启动
6. **主题系统**: 更强大的可视化编辑器

## 📚 参考资源

- [Wox Documentation](https://wox-launcher.github.io/Wox/#/)
- [Wox GitHub](https://github.com/Wox-launcher/Wox)
- [Wox Plugin Store](https://wox-launcher.github.io/Wox/#/plugin_store)
- [Wox AI Theme](https://wox-launcher.github.io/Wox/#/ai_theme)
