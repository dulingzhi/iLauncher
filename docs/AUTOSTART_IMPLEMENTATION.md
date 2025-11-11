# Auto Start (开机自启) 功能开发完成

## 📋 功能概述

成功为 iLauncher 实现了跨平台开机自启动功能，用户可以在设置中一键启用或禁用。

## ✅ 完成的工作

### 1. 后端实现 (Rust)

#### 添加依赖
```toml
# Cargo.toml
auto-launch = "0.5"  # 跨平台开机自启库
```

#### 核心模块 (`src-tauri/src/utils/autostart.rs`)
- ✅ `enable()` - 启用开机自启
- ✅ `disable()` - 禁用开机自启  
- ✅ `is_enabled()` - 检查开机自启状态
- ✅ `sync_with_config()` - 根据配置同步自启状态

#### Tauri 命令 (`src-tauri/src/commands/mod.rs`)
- ✅ `enable_autostart()` - 前端调用启用
- ✅ `disable_autostart()` - 前端调用禁用
- ✅ `is_autostart_enabled()` - 前端查询状态
- ✅ `set_autostart(enabled: bool)` - 统一设置接口

#### 应用启动时同步 (`src-tauri/src/lib.rs`)
```rust
// 应用启动时根据配置同步开机自启状态
if let Err(e) = utils::autostart::sync_with_config(config.advanced.start_on_boot) {
    tracing::warn!("Failed to sync autostart with config: {}", e);
} else {
    tracing::info!("✓ Autostart synced: {}", config.advanced.start_on_boot);
}
```

### 2. 前端实现 (TypeScript/React)

#### 设置界面 (`src/components/Settings.tsx`)
- ✅ 在 Advanced 标签页添加 "Start on Boot" 开关
- ✅ 保存时调用 `set_autostart` API
- ✅ 错误处理和用户提示

#### 保存逻辑
```typescript
// 保存设置时处理开机自启
try {
  await invoke('set_autostart', { enabled: config.advanced.start_on_boot });
} catch (error) {
  console.error('Failed to set autostart:', error);
  showToast('Settings saved, but autostart setup failed', 'error');
}
```

### 3. 文档

- ✅ 创建测试指南 `docs/AUTOSTART_TEST.md`
- ✅ 包含详细的测试步骤
- ✅ 故障排查指南
- ✅ API 使用示例

## 🎯 功能特性

### 跨平台支持

| 平台 | 实现方式 | 状态 |
|------|---------|------|
| Windows | 注册表 `HKCU\...\Run` | ✅ 完成 |
| macOS | LaunchAgents | ✅ 支持 |
| Linux | autostart .desktop | ✅ 支持 |

### 用户体验

1. **简单易用**: 设置界面一键开关
2. **自动同步**: 应用启动时自动同步状态
3. **错误提示**: 设置失败时有明确提示
4. **配置持久化**: 设置保存到配置文件

### 开发体验

1. **类型安全**: 完整的 TypeScript 类型定义
2. **错误处理**: 完善的 Result 错误处理
3. **日志记录**: 详细的操作日志
4. **单元测试**: 包含基础测试用例

## 📖 使用方法

### 用户操作

1. 打开 iLauncher
2. 输入 `settings` 打开设置
3. 切换到 "Advanced" 标签
4. 勾选 "Start on Boot"
5. 点击 Save 保存

### 验证设置 (Windows)

```powershell
# 查看注册表
Get-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" | 
  Select-Object -Property *iLauncher*

# 查看日志
Get-Content "$env:LOCALAPPDATA\iLauncher\logs\ilauncher.log" -Tail 20 | 
  Select-String "autostart"
```

## 🔍 技术细节

### 实现原理

#### Windows
```rust
// 添加到注册表
// HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run
// 键名: iLauncher
// 值: "C:\path\to\iLauncher.exe"
```

#### macOS
```bash
# 创建 plist 文件
~/Library/LaunchAgents/com.ilauncher.plist
```

#### Linux
```bash
# 创建 desktop entry
~/.config/autostart/iLauncher.desktop
```

### 依赖库

使用 [auto-launch](https://crates.io/crates/auto-launch) crate:
- 跨平台实现
- 简单 API
- 可靠性高
- 活跃维护

## 🧪 测试

### 测试清单

- [x] Windows 注册表项创建
- [x] 启用/禁用功能
- [x] 应用启动时同步
- [x] 配置持久化
- [x] 错误处理
- [x] 日志记录
- [ ] macOS 测试 (待实际 Mac 环境测试)
- [ ] Linux 测试 (待实际 Linux 环境测试)

### 已知问题

暂无

## 📝 代码统计

| 文件 | 新增行数 | 说明 |
|------|---------|------|
| `utils/autostart.rs` | ~90 | 核心自启动逻辑 |
| `commands/mod.rs` | ~35 | Tauri 命令 |
| `lib.rs` | ~6 | 启动时同步 |
| `Settings.tsx` | ~10 | 前端保存逻辑 |
| **总计** | **~141** | 代码行数 |

## 🚀 下一步优化

### 可选功能

1. **延迟启动**: 支持设置启动延迟时间
2. **最小化启动**: 启动时自动最小化到托盘
3. **静默启动**: 启动时不显示窗口
4. **启动参数**: 支持自定义启动参数

### 示例实现

```rust
// 延迟启动示例
pub fn enable_with_delay(delay_seconds: u32) -> Result<()> {
    let auto_launch = get_auto_launch()?;
    let args = &[format!("--delay={}", delay_seconds)];
    auto_launch.enable_with_args(args)
        .context("Failed to enable auto-start with delay")
}
```

## 📚 参考资料

- [auto-launch crate](https://crates.io/crates/auto-launch)
- [Windows Run Registry Key](https://docs.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)
- [macOS LaunchAgents](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html)
- [Linux autostart](https://specifications.freedesktop.org/autostart-spec/autostart-spec-latest.html)

## 🎉 总结

开机自启功能已完全实现，包括：
- ✅ 跨平台支持 (Windows/macOS/Linux)
- ✅ 用户友好的设置界面
- ✅ 自动同步机制
- ✅ 完善的错误处理
- ✅ 详细的文档和测试指南

预计开发时间：**1天** ✅ 按计划完成！
