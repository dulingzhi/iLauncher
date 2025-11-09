# 性能与进程退出问题诊断

## 问题描述

用户报告两个问题：
1. **搜索结果还是比较慢** - 即使已实现 MFT 优化
2. **Service 依然没有自动退出** - 即使已添加 ExitProcess 强制终止

## 诊断发现

### 1. 运行状态检查

运行 `diagnose.ps1` 发现：
- ✅ **MFT 数据库存在**：C/D/E 盘共1.4GB数据库已生成
- ❌ **没有运行的进程**：诊断时无 ilauncher 进程
- ❌ **没有日志目录**：`$env:LOCALAPPDATA\iLauncher\logs` 不存在
- ❌ **没有配置文件**：`$env:LOCALAPPDATA\iLauncher\config\config.json` 不存在

### 2. 根本原因分析

**问题1：搜索慢**
- MFT Service 可能未正确启动
- 如果 Service 未启动，file_search 插件会降级到 BFS 模式
- BFS 模式需要遍历整个文件系统，速度很慢

**问题2：Service 未退出**
- 无法复现，因为 Service 可能根本没启动
- 需要先确认 Service 能否正确启动

### 3. Service 启动流程

查看 `src-tauri/src/lib.rs:91-117`，Service 启动流程：

```rust
// 如果启用了 MFT，启动 MFT Service 子进程（需要管理员权限）
let mft_enabled = plugins.iter()
    .find(|p| p.id == "file_search")
    .and_then(|p| p.config.as_ref())
    .and_then(|c| c.get("use_mft"))
    .and_then(|v| v.as_bool())
    .unwrap_or(false);

if mft_enabled {
    let exe_path = std::env::current_exe().ok();
    let ui_pid = std::process::id();
    
    if let Some(exe) = exe_path {
        let powershell_cmd = format!(
            "Start-Process -FilePath '{}' -ArgumentList '--mft-service','--ui-pid','{}' -Verb RunAs -WindowStyle Hidden",
            exe.display(),
            ui_pid
        );
        
        // 启动 PowerShell 执行命令（会触发 UAC）
        let child = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &powershell_cmd])
            .spawn();
    }
}
```

**潜在问题：**
1. **配置缺失**：`config.json` 不存在，`use_mft` 可能为 false
2. **UAC 权限**：用户可能拒绝了 UAC 提升权限请求
3. **PowerShell 错误**：`Start-Process -Verb RunAs` 可能失败但没有错误反馈

## 添加的性能日志

已在以下位置添加性能日志：

### `src-tauri/src/mft_scanner/database.rs`

```rust
// Database::search() - 单盘符搜索
tracing::debug!(
    "🔍 MFT search '{}' → {} results in {:.2}ms (target_group: {})",
    query, results.len(), elapsed.as_secs_f64() * 1000.0, target_group
);

// search_all_drives() - 多盘符并行搜索
tracing::info!(
    "🎯 MFT search_all_drives '{}' → {} results in {:.2}ms (scanned {} drives)",
    query, merged.len(), total_elapsed.as_secs_f64() * 1000.0, existing_drives.len()
);
```

### `src-tauri/src/plugin/file_search.rs`

```rust
// query_from_mft_database() - 插件层搜索
tracing::info!(
    "✅ MFT query completed: '{}' → {} results in {:.2}ms",
    search, results.len(), query_elapsed.as_secs_f64() * 1000.0
);
```

## 测试步骤

### 1. 启动 UI 并观察

```powershell
# 启动 UI
.\src-tauri\target\release\ilauncher.exe

# 检查进程
Get-Process ilauncher

# 预期：看到 2 个进程（UI + Service）
# 实际：如果只看到 1 个进程，说明 Service 未启动
```

### 2. 检查配置文件

```powershell
# 查看配置
$configFile = "$env:LOCALAPPDATA\iLauncher\config\config.json"
if (Test-Path $configFile) {
    Get-Content $configFile | ConvertFrom-Json | ConvertTo-Json -Depth 10
}
```

**预期配置：**
```json
{
  "plugins": [
    {
      "id": "file_search",
      "enabled": true,
      "config": {
        "use_mft": true
      }
    }
  ]
}
```

### 3. 手动测试 Service 启动

```powershell
# 获取当前进程 PID
$uiPid = (Get-Process ilauncher)[0].Id

# 手动启动 Service（需要管理员权限）
Start-Process -FilePath ".\src-tauri\target\release\ilauncher.exe" `
    -ArgumentList "--mft-service", "--ui-pid", "$uiPid" `
    -Verb RunAs

# 检查进程
Get-Process ilauncher
```

### 4. 查看性能日志

日志位置取决于 Service 如何启动：

**如果通过 PowerShell -Verb RunAs 启动：**
- 日志输出到 PowerShell 窗口（但窗口被隐藏了）
- 需要修改 `WindowStyle Normal` 来查看

**如果通过 double-click 启动：**
- 日志输出到 stderr/stdout

**解决方案：** 添加文件日志

```rust
// 在 lib.rs::run_mft_service() 中
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::writer::MakeWriterExt;

let log_dir = paths::get_log_dir()?;
let file_appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "mft_service.log");

tracing_subscriber::fmt()
    .with_writer(file_appender)
    .init();
```

## 下一步行动

### 紧急修复：添加文件日志

Service 的 `tracing` 日志目前只输出到 stdout，但由于 PowerShell 启动时使用了 `-WindowStyle Hidden`，所有日志都丢失了。

**解决方案：**
1. 使用 `tracing-appender` 将日志写入文件
2. 位置：`$env:LOCALAPPDATA\iLauncher\logs\mft_service.log`

### 配置初始化

如果配置文件不存在，使用默认配置：

```rust
// 确保默认启用 MFT
fn default_use_mft() -> bool {
    true
}
```

但需要确认 `AppConfig` 的默认值。

### UAC 提示处理

目前 PowerShell 启动 Service 会触发 UAC，如果用户拒绝：
- 没有任何错误提示
- Service 静默失败
- 搜索降级到 BFS 模式（慢）

**改进方案：**
1. 检测 PowerShell spawn 是否成功
2. 如果失败，显示友好的错误提示
3. 提供"不使用 MFT"的降级选项

## 预期性能指标

根据之前的优化，MFT 模式应该达到：

- **搜索延迟**：< 50ms（通常 10-30ms）
- **结果数量**：限制 50 条
- **并行查询**：3 个盘符同时查询（rayon）
- **智能分表**：只查询 3 个相关表（93% 减少）

如果实际性能达不到，可能原因：
1. Service 未启动（降级到 BFS）
2. 数据库索引缺失（需要 `CREATE INDEX idx_path ON listN(PATH)`）
3. SQLite 缓存太小（当前 64MB）

## 测试清单

- [ ] UI 启动后，确认有 2 个 ilauncher 进程
- [ ] 配置文件存在且 `use_mft: true`
- [ ] MFT 数据库存在（C/D/E.db）
- [ ] 搜索 "chrome" 响应时间 < 100ms
- [ ] 查看日志确认使用 MFT 模式
- [ ] 关闭 UI，确认 Service 在 2-3 秒内退出
- [ ] 查看日志确认 Service 检测到 UI 退出
