# iLauncher 启动参数指南

## 概述
`ilauncher.exe` 是一个多功能的单一可执行文件，通过命令行参数可以切换不同的运行模式。

## 运行模式

### 1. 🖥️ GUI 模式（默认）
双击运行或不带参数启动，进入图形界面模式。

```powershell
.\ilauncher.exe
```

### 2. 📁 MFT Service 模式（扫描 + 监控）
对 NTFS 驱动器进行全量扫描后自动启动实时监控。

```powershell
# 基本用法（使用配置文件中的设置）
.\ilauncher.exe --mft-service

# 指定输出目录
.\ilauncher.exe --mft-service --output "D:/mft_db"

# 指定驱动器（逗号分隔）
.\ilauncher.exe --mft-service --drives C,D,E

# 仅扫描一次，不启动监控
.\ilauncher.exe --mft-service --scan-only

# 完整示例
.\ilauncher.exe --mft-service --output "D:/mft_db" --drives C,D --scan-only
```

### 3. 🔧 旧版 MFT Scanner 模式（已废弃）
仅为兼容性保留，建议使用 `--mft-service` 代替。

```powershell
.\ilauncher.exe --mft-scanner
```

## MFT Service 参数详解

| 参数 | 简写 | 说明 | 示例 | 默认值 |
|------|------|------|------|--------|
| `--mft-service` | - | 启用 MFT Service 模式 | `--mft-service` | - |
| `--output` | `-o` | 数据库输出目录 | `--output "D:/mft_db"` | 配置文件中的 `output_dir` |
| `--drives` | `-d` | 要处理的驱动器（逗号分隔） | `--drives C,D,E` | 配置文件中的 `drives` |
| `--scan-only` | - | 仅执行扫描，不启动监控 | `--scan-only` | `false` |

## 使用场景

### 场景 1: 日常使用（GUI 模式）
```powershell
# 双击启动或命令行启动
.\ilauncher.exe
```
- ✅ 正常的启动器界面
- ✅ 插件系统
- ✅ 搜索功能

### 场景 2: 初次建立索引（MFT 扫描）
```powershell
# 以管理员身份运行 PowerShell
.\ilauncher.exe --mft-service --output "D:/mft_db" --scan-only
```
- ✅ 对所有驱动器进行全量扫描
- ✅ 构建 FRN 映射表，重建完整路径
- ✅ 保存到 SQLite 数据库
- ✅ 扫描完成后自动退出

### 场景 3: 后台常驻监控（扫描 + 监控）
```powershell
# 以管理员身份运行 PowerShell
.\ilauncher.exe --mft-service --output "D:/mft_db" --drives C,D
```
- ✅ 先进行全量扫描
- ✅ 扫描完成后自动启动实时监控
- ✅ 监听文件创建、删除、重命名事件
- ✅ 按 `Ctrl+C` 优雅退出

### 场景 4: 定时任务（仅更新索引）
```powershell
# 创建 Windows 计划任务，每天凌晨 3 点执行
schtasks /create /tn "iLauncher MFT Scan" /tr "D:\Apps\ilauncher.exe --mft-service --scan-only" /sc daily /st 03:00 /ru SYSTEM
```

## 运行流程示例

### GUI 模式
```
PS> .\ilauncher.exe

🚀 Starting iLauncher...
[GUI 窗口打开]
```

### MFT Service 模式（完整流程）
```
PS> .\ilauncher.exe --mft-service --drives C

🚀 MFT Service starting...
📅 2025-11-08 22:45:30
✓ Config loaded
✓ Output directory: D:/mft_db
✓ Drives to process: ['C']

╔═══════════════════════════════════════════╗
║    Phase 1: Full Disk Scan                ║
╚═══════════════════════════════════════════╝

📀 Starting scan for drive C:
🔍 Building FRN map (Phase 1)...
✓ FRN map built: 1,234,567 entries
💾 Rebuilding paths and saving to database (Phase 2)...
   Progress: 100000 files saved
   Progress: 200000 files saved
   ...
✅ Drive C scan completed: 1,234,567 files saved to database

╔═══════════════════════════════════════════╗
║    Scan Phase Complete                    ║
╚═══════════════════════════════════════════╝
⏱️  Total scan time: 45.23s
✓ Successfully scanned drives: ['C']

╔═══════════════════════════════════════════╗
║    Phase 2: Real-time Monitoring          ║
╚═══════════════════════════════════════════╝

👀 Starting monitor for drive C:
✓ All monitors started
💡 Press Ctrl+C to stop monitoring and exit

   ➕ Created: C:\Users\Documents\new_file.txt
   ✏️  Renamed: C:\Downloads\renamed.pdf
   🗑️  Deleted: C:\Temp\old_file.tmp

^C
🛑 Received shutdown signal, stopping monitors...
🛑 Stop signal received, exiting monitor loop for drive C
✓ Monitor for drive C stopped gracefully
🎉 MFT Service stopped successfully
```

## 配置文件

默认配置文件：`scan_config.json`（与 exe 同目录）

```json
{
  "drives": ["C", "D"],
  "output_dir": "D:/mft_db",
  "ignore_paths": [
    "$Recycle.Bin",
    "System Volume Information",
    "Windows\\WinSxS",
    "Windows\\Temp",
    "AppData\\Local\\Temp"
  ]
}
```

## 性能参考

| 操作 | 性能指标 |
|------|----------|
| 扫描速度 | ~100 万文件/秒（SSD） |
| 监控延迟 | <100ms（实时响应） |
| 内存占用（扫描） | ~50MB（100 万文件） |
| 内存占用（监控） | ~30MB（常驻） |
| 数据库大小 | ~100MB（100 万文件） |

## 系统要求

⚠️ **MFT Service 模式要求**：
- Windows 操作系统
- NTFS 文件系统
- **管理员权限**（必需）
- USN Journal 功能（自动启用）

⚠️ **GUI 模式要求**：
- 无需管理员权限
- 支持 Windows 7+

## 故障排查

### 问题：MFT Service 启动失败
```
❌ Administrator privileges required
```
**解决**：右键 → 以管理员身份运行 PowerShell

### 问题：找不到配置文件
```
Failed to load config
```
**解决**：在 exe 同目录创建 `scan_config.json`

### 问题：路径不完整
```
只显示文件名而非完整路径
```
**解决**：确保使用了 `--mft-service`（已实现 FRN 映射）

## 与旧版本对比

| 特性 | 旧版本（3 个 exe） | 新版本（单一 exe） |
|------|-------------------|-------------------|
| UI 程序 | `ilauncher.exe` | `ilauncher.exe` |
| 扫描器 | `scanner.exe` | `ilauncher.exe --mft-service --scan-only` |
| 监控器 | `monitor.exe` | `ilauncher.exe --mft-service` |
| 管理难度 | 🔴 高（3 个文件） | 🟢 低（1 个文件） |
| 启动方式 | 手动分别启动 | 参数切换模式 |
| 路径重建 | ❌ 部分缺失 | ✅ 完整实现 |

## 高级用法

### 1. 创建桌面快捷方式（GUI 模式）
```powershell
$WshShell = New-Object -comObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut("$Home\Desktop\iLauncher.lnk")
$Shortcut.TargetPath = "D:\Apps\ilauncher.exe"
$Shortcut.Save()
```

### 2. 创建桌面快捷方式（MFT Service）
```powershell
$WshShell = New-Object -comObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut("$Home\Desktop\iLauncher MFT Service.lnk")
$Shortcut.TargetPath = "D:\Apps\ilauncher.exe"
$Shortcut.Arguments = "--mft-service"
$Shortcut.Save()
```

### 3. 计划任务（每日扫描）
```powershell
$Action = New-ScheduledTaskAction -Execute "D:\Apps\ilauncher.exe" -Argument "--mft-service --scan-only"
$Trigger = New-ScheduledTaskTrigger -Daily -At 3am
Register-ScheduledTask -Action $Action -Trigger $Trigger -TaskName "iLauncher Daily Scan" -Description "Daily MFT scan" -RunLevel Highest
```

## 日志输出

MFT Service 模式的日志会输出到控制台，可以重定向到文件：

```powershell
.\ilauncher.exe --mft-service 2>&1 | Tee-Object -FilePath "mft_service.log"
```
