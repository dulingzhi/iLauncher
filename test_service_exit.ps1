# 测试 MFT Service 自动退出功能
# 验证 UI 进程退出后，Service 是否正确退出

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  MFT Service 退出测试" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 检查管理员权限
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "⚠️  警告: 需要管理员权限运行此测试" -ForegroundColor Yellow
    Write-Host "请右键选择 '以管理员身份运行' PowerShell" -ForegroundColor Yellow
    exit 1
}

$ExePath = ".\src-tauri\target\release\ilauncher.exe"

if (-not (Test-Path $ExePath)) {
    Write-Host "❌ 错误: 找不到 ilauncher.exe" -ForegroundColor Red
    Write-Host "请先编译: cargo build --release" -ForegroundColor Yellow
    exit 1
}

Write-Host "✓ 找到可执行文件: $ExePath" -ForegroundColor Green
Write-Host ""

# ==========================================
# 测试 1: 启动 UI，检查 Service 是否也启动
# ==========================================
Write-Host "测试 1: UI 启动后 Service 自动启动" -ForegroundColor Cyan
Write-Host "-----------------------------------"

# 先确保没有残留进程
Get-Process ilauncher -ErrorAction SilentlyContinue | Stop-Process -Force

Write-Host "▶️  启动 UI 进程..." -ForegroundColor Yellow
$uiProcess = Start-Process -FilePath $ExePath -PassThru
$uiPid = $uiProcess.Id

Write-Host "✓ UI 进程已启动 (PID: $uiPid)" -ForegroundColor Green
Write-Host "  等待 5 秒让 Service 启动..." -ForegroundColor Gray
Start-Sleep -Seconds 5

# 检查进程数量
$processes = Get-Process ilauncher -ErrorAction SilentlyContinue
if ($processes.Count -ge 2) {
    Write-Host "✓ 发现 $($processes.Count) 个 ilauncher 进程:" -ForegroundColor Green
    foreach ($proc in $processes) {
        $isAdmin = $false
        try {
            # 尝试获取进程优先级（管理员进程才能访问）
            $priority = $proc.PriorityClass
            $isAdmin = $true
        } catch {
            $isAdmin = $false
        }
        
        $procType = if ($proc.Id -eq $uiPid) { "UI" } else { "Service (Admin)" }
        Write-Host "  - PID: $($proc.Id) [$procType], 内存: $([math]::Round($proc.WorkingSet64/1MB, 2)) MB" -ForegroundColor White
    }
} else {
    Write-Host "⚠️  只发现 $($processes.Count) 个进程，Service 可能未启动" -ForegroundColor Yellow
}

Write-Host ""

# ==========================================
# 测试 2: 关闭 UI，检查 Service 是否自动退出
# ==========================================
Write-Host "测试 2: UI 退出后 Service 自动退出" -ForegroundColor Cyan
Write-Host "-----------------------------------"

Write-Host "▶️  关闭 UI 进程 (PID: $uiPid)..." -ForegroundColor Yellow
Stop-Process -Id $uiPid -Force

Write-Host "✓ UI 进程已关闭" -ForegroundColor Green
Write-Host "  等待 5 秒检查 Service 是否自动退出..." -ForegroundColor Gray

# 等待并持续检查
for ($i = 1; $i -le 10; $i++) {
    Start-Sleep -Seconds 1
    $remainingProcesses = Get-Process ilauncher -ErrorAction SilentlyContinue
    
    if ($remainingProcesses) {
        Write-Host "  [$i 秒] 还有 $($remainingProcesses.Count) 个进程运行中..." -ForegroundColor Gray
    } else {
        Write-Host ""
        Write-Host "✅ 成功！所有进程已退出 (耗时: $i 秒)" -ForegroundColor Green
        Write-Host ""
        Write-Host "╔═══════════════════════════════════════════╗" -ForegroundColor Green
        Write-Host "║  测试通过 ✓                               ║" -ForegroundColor Green
        Write-Host "╚═══════════════════════════════════════════╝" -ForegroundColor Green
        Write-Host ""
        Write-Host "结论:" -ForegroundColor Cyan
        Write-Host "  ✓ UI 进程退出后，Service 在 $i 秒内正确退出" -ForegroundColor Green
        Write-Host "  ✓ 没有僵尸进程残留" -ForegroundColor Green
        exit 0
    }
}

# 如果 10 秒后还有进程
Write-Host ""
if ($remainingProcesses) {
    Write-Host "❌ 测试失败！Service 未能自动退出" -ForegroundColor Red
    Write-Host ""
    Write-Host "残留进程:" -ForegroundColor Yellow
    foreach ($proc in $remainingProcesses) {
        Write-Host "  - PID: $($proc.Id), 内存: $([math]::Round($proc.WorkingSet64/1MB, 2)) MB" -ForegroundColor White
    }
    
    Write-Host ""
    Write-Host "正在强制停止残留进程..." -ForegroundColor Yellow
    $remainingProcesses | Stop-Process -Force
    
    Write-Host ""
    Write-Host "╔═══════════════════════════════════════════╗" -ForegroundColor Red
    Write-Host "║  测试失败 ✗                               ║" -ForegroundColor Red
    Write-Host "╚═══════════════════════════════════════════╝" -ForegroundColor Red
    Write-Host ""
    Write-Host "可能原因:" -ForegroundColor Yellow
    Write-Host "  1. UI PID 未正确传递给 Service" -ForegroundColor Gray
    Write-Host "  2. 进程监控线程未启动" -ForegroundColor Gray
    Write-Host "  3. ExitProcess 调用失败" -ForegroundColor Gray
    Write-Host ""
    Write-Host "调试建议:" -ForegroundColor Cyan
    Write-Host "  1. 查看日志: %LOCALAPPDATA%\iLauncher\logs\mft_scanner.log" -ForegroundColor White
    Write-Host "  2. 手动测试: .\ilauncher.exe --mft-service --ui-pid <PID>" -ForegroundColor White
    
    exit 1
} else {
    Write-Host "✅ 所有进程已退出" -ForegroundColor Green
    exit 0
}
