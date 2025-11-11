<div align="center">

# iLauncher

<p>
  <strong>快速、轻量、优雅的应用启动器</strong>
</p>

<p>
  <a href="https://github.com/dulingzhi/iLauncher/releases">
    <img src="https://img.shields.io/github/v/release/dulingzhi/iLauncher?style=flat-square" alt="Release">
  </a>
  <a href="https://github.com/dulingzhi/iLauncher/blob/master/LICENSE">
    <img src="https://img.shields.io/github/license/dulingzhi/iLauncher?style=flat-square" alt="License">
  </a>
  <a href="https://github.com/dulingzhi/iLauncher/releases">
    <img src="https://img.shields.io/github/downloads/dulingzhi/iLauncher/total?style=flat-square" alt="Downloads">
  </a>
</p>

<p>
  <strong>中文</strong> | <a href="README_EN.md">English</a>
</p>

</div>

---

## 📸 预览

### 搜索界面
![搜索界面](docs/search.png)

### 设置界面
![设置界面](docs/setting.png)

---

## ✨ 特性

- 🚀 **极速启动** - Rust 核心，毫秒级响应
- 🎯 **全局快捷键** - `Alt + Space` 随时唤起
- 🔍 **智能搜索** - 支持拼音、模糊匹配
- ⚡ **MFT 文件搜索** - 450 万+文件，9 秒扫描完成
- 🎨 **精美主题** - 多款内置主题，支持自定义
- 📋 **剪贴板历史** - 永不丢失重要内容
- 🧮 **计算器** - 快速计算数学表达式
- 🔄 **自动更新** - 一键更新到最新版本
- 🔐 **开机自启** - 可选开机自动启动
- 💾 **轻量级** - 内存占用 < 50MB
- 🌍 **跨平台** - Windows、macOS、Linux

---

## 📦 下载安装

### Windows

访问 [Releases](https://github.com/dulingzhi/iLauncher/releases) 页面下载最新版本：

- **推荐**: `iLauncher_x.x.x_x64-setup.exe` （安装程序）
- 或: `iLauncher_x.x.x_x64.msi` （MSI 安装包）

### macOS

- **Intel Mac**: `iLauncher_x.x.x_x64.dmg`
- **Apple Silicon (M1/M2/M3)**: `iLauncher_x.x.x_aarch64.dmg`

### Linux

- **AppImage**: `iLauncher_x.x.x_amd64.AppImage` （免安装，直接运行）
- **Debian/Ubuntu**: `iLauncher_x.x.x_amd64.deb`

---

## 🚀 快速开始

### 1. 启动应用

安装完成后，应用会自动运行。你会在系统托盘看到 iLauncher 图标。

### 2. 使用快捷键

- **显示/隐藏**: `Alt + Space` (Windows/Linux) 或 `Cmd + Space` (macOS)
- **向上选择**: `↑` 或 `Ctrl + P`
- **向下选择**: `↓` 或 `Ctrl + N`
- **执行操作**: `Enter`
- **隐藏窗口**: `Esc`

### 3. 开始搜索

输入任何内容开始搜索：

- **应用程序**: 输入应用名称快速启动
- **文件**: 输入文件名查找文件
- **计算**: 输入数学表达式（如 `2+2`）
- **剪贴板**: 输入 `clipboard` 查看历史记录
- **设置**: 输入 `settings` 打开设置

---

## 🎯 核心功能

### 📱 应用搜索

快速查找并启动已安装的应用程序，支持拼音首字母搜索。

**示例**:
- 输入 `chrome` → 启动 Chrome 浏览器
- 输入 `wjb` → 找到"微信"（拼音首字母）

### 📂 文件搜索

使用 MFT（Master File Table）技术，实现毫秒级文件搜索。

**性能**:
- 450 万+文件扫描时间：约 9 秒
- 搜索响应时间：< 50ms
- 支持实时索引更新

### 🧮 计算器

直接在搜索框输入数学表达式即可计算。

**示例**:
- `2+2` → `4`
- `sin(90)` → `1`
- `sqrt(16)` → `4`

### 📋 剪贴板历史

自动记录复制的文本，永不丢失重要内容。

**功能**:
- 自动保存历史记录
- 支持收藏常用内容
- 快速搜索和粘贴

### 🎨 主题系统

多款精美内置主题，支持自定义主题编辑器。

**内置主题**:
- VS Code Dark
- GitHub Light
- Dracula
- Nord
- Monokai
- 更多...

---

## ⚙️ 设置

### 常规设置

- **全局热键**: 自定义唤起快捷键
- **搜索延迟**: 调整搜索防抖时间
- **最大结果数**: 控制显示的搜索结果数量
- **界面语言**: 中文/English

### 外观设置

- **主题**: 选择或自定义主题
- **窗口大小**: 调整窗口宽度和高度
- **字体大小**: 调整界面字体
- **透明度**: 设置窗口透明度
- **预览面板**: 开启/关闭文件预览

### 高级设置

- **开机自启**: 系统启动时自动运行
- **托盘图标**: 显示/隐藏系统托盘图标
- **缓存管理**: 清除应用缓存
- **自动更新**: 检查并安装更新

---

## 🔄 自动更新

iLauncher 支持自动更新：

1. **自动检查**: 启动后 5 秒自动检查更新
2. **手动检查**: 设置 → 高级 → 检查更新
3. **一键更新**: 发现新版本时一键下载安装
4. **自动重启**: 更新完成后自动重启应用

---

## ❓ 常见问题

<details>
<summary><strong>Q: 如何修改全局快捷键？</strong></summary>

1. 打开设置（输入 `settings`）
2. 进入"常规"选项卡
3. 点击"全局热键"输入框
4. 按下你想要的快捷键组合
5. 点击"保存"

</details>

<details>
<summary><strong>Q: MFT 文件搜索需要管理员权限吗？</strong></summary>

是的，MFT 扫描需要管理员权限才能访问主文件表。首次启用时会弹出 UAC 提示。

如果不想使用管理员权限，应用会自动降级使用传统文件遍历方式（速度较慢）。

</details>

<details>
<summary><strong>Q: 如何卸载？</strong></summary>

**Windows**:
- 控制面板 → 程序和功能 → 找到 iLauncher → 卸载
- 或使用安装包自带的卸载程序

**macOS**:
- 将 iLauncher.app 拖到废纸篓

**Linux**:
- 删除 AppImage 文件
- 或使用包管理器卸载（如果通过 .deb 安装）

</details>

<details>
<summary><strong>Q: 数据存储在哪里？</strong></summary>

**Windows**: `%LOCALAPPDATA%\iLauncher\`
**macOS**: `~/Library/Application Support/iLauncher/`
**Linux**: `~/.local/share/iLauncher/`

包含：
- `config/` - 配置文件
- `cache/` - 缓存数据
- `logs/` - 日志文件
- `clipboard.db` - 剪贴板历史

</details>

---

## 🛠️ 开发者

如果你想为 iLauncher 贡献代码或进行二次开发：

### 技术栈

- **前端**: React 19 + TypeScript + Tailwind CSS
- **后端**: Rust + Tauri 2
- **构建**: Bun + Vite

### 本地开发

```bash
# 克隆项目
git clone https://github.com/dulingzhi/iLauncher.git
cd iLauncher

# 安装依赖
bun install

# 开发模式
bun tauri dev

# 构建发布版
bun tauri build
```

### 文档

- [功能对比](FEATURE_COMPARISON.md) - 与 Wox 的功能对比
- [发布流程](.github/workflows/README.md) - CI/CD 配置说明

---

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

### 如何贡献

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 提交 Pull Request

---

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情

---

## 🙏 致谢

- [Wox](https://github.com/Wox-launcher/Wox) - 灵感来源
- [Tauri](https://tauri.app/) - 跨平台框架
- [Raycast](https://www.raycast.com/) - UI 设计参考

---

<div align="center">

**如果觉得有用，请给个 ⭐ Star！**

Made with ❤️ by [dulingzhi](https://github.com/dulingzhi)

</div>
