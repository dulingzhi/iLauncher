# 开机自启功能测试指南

## 功能说明

iLauncher 现在支持开机自启功能，可以在系统启动时自动启动应用程序。

## 实现方式

### Windows
- 使用注册表 `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`
- 添加应用程序路径

### macOS  
- 使用 LaunchAgents
- 创建 plist 文件在 `~/Library/LaunchAgents/`

### Linux
- 使用 autostart desktop entry
- 创建 .desktop 文件在 `~/.config/autostart/`

## 测试步骤

### 1. 启动应用

```bash
cd d:\Projects\iLauncher
bun tauri dev
```

### 2. 打开设置

1. 按 `Alt + Space` 打开 iLauncher
2. 输入 `settings` 或 `设置`
3. 选择 Settings 并回车

### 3. 启用开机自启

1. 切换到 **Advanced** (高级) 标签
2. 找到 "Start on Boot" (开机启动) 选项
3. 勾选复选框
4. 点击 "Save" 保存设置

### 4. 验证注册表（Windows）

打开注册表编辑器：

```powershell
# 查看注册表项
Get-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" | Select-Object -Property *iLauncher*
```

应该看到类似：
```
iLauncher    REG_SZ    "D:\Projects\iLauncher\src-tauri\target\debug\iLauncher.exe"
```

### 5. 验证功能

方法 1: 注销重新登录
```powershell
# 注销
shutdown /l
```

方法 2: 重启系统
```powershell
# 重启
shutdown /r /t 0
```

重新登录后，iLauncher 应该自动启动。

### 6. 禁用开机自启

1. 再次打开设置
2. 取消勾选 "Start on Boot"
3. 保存设置
4. 验证注册表项已删除

## API 使用

### 前端调用

```typescript
import { invoke } from '@tauri-apps/api/core';

// 启用开机自启
await invoke('enable_autostart');

// 禁用开机自启
await invoke('disable_autostart');

// 检查状态
const enabled = await invoke<boolean>('is_autostart_enabled');

// 设置（推荐）
await invoke('set_autostart', { enabled: true });
```

### Rust 使用

```rust
use crate::utils::autostart;

// 启用
autostart::enable()?;

// 禁用
autostart::disable()?;

// 检查状态
let is_enabled = autostart::is_enabled()?;

// 同步配置
autostart::sync_with_config(should_enable)?;
```

## 故障排查

### 问题 1: 保存设置失败

**错误信息**: "Settings saved, but autostart setup failed"

**解决方法**:
1. 检查应用程序路径是否正确
2. 确认有权限修改注册表（Windows）
3. 查看日志文件了解详细错误

### 问题 2: 重启后没有自动启动

**检查清单**:
- [ ] 注册表项是否存在
- [ ] 应用程序路径是否正确
- [ ] 是否有其他安全软件阻止
- [ ] 检查系统启动日志

### 问题 3: 编译错误

如果遇到编译错误，确保已安装依赖：

```bash
cd src-tauri
cargo update
cargo build
```

## 日志查看

Windows 日志位置：
```
%LOCALAPPDATA%\iLauncher\logs\ilauncher.log
```

查看日志：
```powershell
Get-Content "$env:LOCALAPPDATA\iLauncher\logs\ilauncher.log" -Tail 50
```

搜索自启动相关日志：
```powershell
Select-String -Path "$env:LOCALAPPDATA\iLauncher\logs\ilauncher.log" -Pattern "autostart|Auto-start"
```

## 注意事项

1. **开发模式**: 在 `tauri dev` 模式下，注册的是 debug 版本的路径
2. **生产模式**: 发布后，路径会自动更新为安装路径
3. **权限**: 不需要管理员权限即可设置开机自启
4. **卸载**: 卸载应用时应自动清理注册表项

## 测试结果记录

测试日期: ___________
测试人: ___________

| 测试项 | 预期结果 | 实际结果 | 通过/失败 |
|--------|---------|---------|----------|
| 启用开机自启 | 注册表项创建成功 | | □ |
| 重启后自动启动 | 应用自动启动 | | □ |
| 禁用开机自启 | 注册表项删除成功 | | □ |
| 重启后不自动启动 | 应用不启动 | | □ |
| 配置持久化 | 设置保存到配置文件 | | □ |
| 应用启动时同步 | 启动时状态正确 | | □ |

## 下一步

- [ ] 测试 macOS 支持
- [ ] 测试 Linux 支持
- [ ] 添加更多错误处理
- [ ] 添加用户通知
- [ ] 支持延迟启动
