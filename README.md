# iLauncher

> 基于 Tauri 的跨平台应用启动器，灵感来自 Wox

## ✨ 特性

- 🚀 快速启动：Rust 核心提供极致性能
- 🎨 现代化 UI：React + Tailwind CSS
- 🔌 插件系统：支持 Python、Node.js、脚本插件
- ⚡ MFT 扫描：毫秒级文件搜索（450万+ 文件）
- 🤖 AI 集成：支持 OpenAI、Claude 等
- 🎯 全局热键：Alt/Cmd + Space 快速调用
- 💾 轻量级：内存占用 < 50MB
- 🌍 跨平台：Windows、macOS、Linux

## 🎯 当前进度

### ✅ 已完成

1. **项目结构**
   - ✅ 清理模板代码
   - ✅ 创建模块化目录结构
   - ✅ 配置 Tailwind CSS
   - ✅ 配置 TypeScript

2. **核心功能**
   - ✅ Rust 核心类型定义
   - ✅ TypeScript 类型定义
   - ✅ Tauri Commands 基础实现
   - ✅ 查询处理框架
   - ✅ 操作执行框架

3. **UI 组件**
   - ✅ SearchBox 搜索框组件
   - ✅ ResultList 结果列表
   - ✅ 键盘导航 (↑↓ Enter Esc)
   - ✅ 状态管理 (Zustand)
   - ✅ 防抖查询

4. **窗口管理**
   - ✅ 无边框透明窗口
   - ✅ 始终置顶
   - ✅ 显示/隐藏控制
   - ✅ 全局热键 (Alt/Cmd + Space)

### 🚧 进行中

- [ ] 应用搜索插件 (原生)
- [ ] 计算器插件
- [ ] 文件搜索插件

### 📋 待开发

详见 [TODO.md](./TODO.md)

## 🏗️ 技术栈

### 前端
- **框架**: React 18
- **语言**: TypeScript 5
- **样式**: Tailwind CSS
- **状态**: Zustand
- **UI库**: Radix UI
- **图标**: Lucide React

### 后端
- **核心**: Rust
- **框架**: Tauri 2
- **异步**: Tokio
- **热键**: global-hotkey
- **搜索**: fuzzy-matcher
- **日志**: tracing

## 🚀 快速开始

### 安装

```bash
# 克隆项目
git clone https://github.com/dulingzhi/iLauncher.git
cd iLauncher

# 安装依赖
bun install
cd src-tauri
cargo build --release

# 开发模式
bun tauri dev

# 构建发布版
bun tauri build
```

### 运行模式

iLauncher 是一个**单一可执行文件**，通过命令行参数切换不同模式：

#### 🖥️ GUI 模式（默认）
```powershell
# 双击启动或命令行
.\ilauncher.exe

# 如果启用了 MFT，会自动启动后台扫描进程
```

#### ⚡ MFT Service 模式（后台扫描）
```powershell
# 自动启动（GUI 模式下，如果配置启用）
.\ilauncher.exe --mft-service

# 手动管理
# 1. 全量扫描所有 NTFS 盘符
# 2. 实时监听文件变化
# 3. 数据写入 SQLite：%TEMP%\ilauncher_mft\*.db
```

**MFT 功能开关**：

编辑配置文件 `%APPDATA%\iLauncher\config\config.json`：

```json
{
  "advanced": {
    "enable_mft": true  // true=启用 MFT，false=使用 BFS
  }
}
```

或通过 Tauri 命令（前端调用）：

```typescript
import { invoke } from '@tauri-apps/api/tauri';

await invoke('toggle_mft', { enabled: true });  // 启用
await invoke('toggle_mft', { enabled: false }); // 禁用
```

### MFT 性能数据

| 指标 | MFT 模式 | BFS 模式 |
|------|----------|----------|
| 扫描 450 万文件 | 9 秒 | 5-10 分钟 |
| 搜索延迟 | <50ms | 100-500ms |
| 实时更新 | 是 | 否 |
| 权限要求 | 管理员 | 普通用户 |

详细文档：[MFT UI Integration Guide](./MFT_UI_INTEGRATION.md)
```

#### 📁 MFT Service 模式（文件索引）
```powershell
# 全量扫描 + 实时监控（需要管理员权限）
.\ilauncher.exe --mft-service

# 仅扫描一次
.\ilauncher.exe --mft-service --scan-only

# 指定驱动器和输出目录
.\ilauncher.exe --mft-service --drives C,D --output "D:/mft_db"
```

📖 **详细使用指南**: 查看 [ILAUNCHER_USAGE.md](./ILAUNCHER_USAGE.md)

🧪 **测试脚本**: 运行 `.\test_ilauncher.ps1` 验证所有功能

## 🏗️ 技术栈

- `Alt/Cmd + Space` - 显示/隐藏启动器
- `↑` / `↓` - 选择结果
- `Enter` - 执行默认操作
- `Esc` - 隐藏窗口

## 📁 项目结构

```
iLauncher/
├── src/                    # 前端代码
│   ├── components/         # React 组件
│   │   └── SearchBox.tsx   # 搜索框
│   ├── hooks/              # React Hooks
│   │   └── useQuery.ts     # 查询逻辑
│   ├── store/              # 状态管理
│   │   └── useAppStore.ts  # 全局状态
│   ├── types/              # TypeScript 类型
│   │   └── index.ts
│   ├── utils/              # 工具函数
│   │   └── cn.ts           # className 工具
│   ├── App.tsx             # 主应用
│   ├── main.tsx            # 入口文件
│   └── index.css           # 全局样式
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── commands/       # Tauri Commands
│   │   │   └── mod.rs
│   │   ├── core/           # 核心类型
│   │   │   ├── mod.rs
│   │   │   └── types.rs
│   │   ├── hotkey/         # 热键管理
│   │   │   └── mod.rs
│   │   ├── plugin/         # 插件系统 (TODO)
│   │   ├── lib.rs          # 库入口
│   │   └── main.rs         # 主程序
│   ├── Cargo.toml          # Rust 依赖
│   └── tauri.conf.json     # Tauri 配置
├── TODO.md                 # 详细开发计划
├── package.json
└── README.md
```

## 🤝 贡献

欢迎贡献！请查看 [TODO.md](./TODO.md) 了解开发计划。

## 📄 许可证

MIT License

## 🙏 致谢

- [Wox](https://github.com/Wox-launcher/Wox) - 灵感来源
- [Tauri](https://tauri.app/) - 跨平台框架
- [Raycast](https://www.raycast.com/) - 设计参考

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
