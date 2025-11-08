# iLauncher 功能测试脚本
# 需要以管理员身份运行 PowerShell

Write-Host "🧪 iLauncher 多模式测试" -ForegroundColor Cyan
Write-Host "=" * 60

# 检查管理员权限
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "⚠️  警告: 某些测试需要管理员权限" -ForegroundColor Yellow
    Write-Host "GUI 模式测试可以继续，但 MFT Service 测试将被跳过" -ForegroundColor Yellow
    Write-Host ""
} else {
    Write-Host "✓ 管理员权限检查通过" -ForegroundColor Green
}

# 设置路径
$exePath = ".\src-tauri\target\release\ilauncher.exe"
$testOutput = Join-Path $env:TEMP "mft_test_db"

# 检查可执行文件
if (-not (Test-Path $exePath)) {
    Write-Host "❌ 错误: 找不到 ilauncher.exe" -ForegroundColor Red
    Write-Host "路径: $exePath" -ForegroundColor Yellow
    Write-Host "请先编译: cargo build --release" -ForegroundColor Yellow
    exit 1
}

$fileInfo = Get-Item $exePath
Write-Host "✓ 找到可执行文件: $exePath" -ForegroundColor Green
Write-Host "  大小: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor Gray
Write-Host "  修改时间: $($fileInfo.LastWriteTime)" -ForegroundColor Gray
Write-Host ""

# ============================================================
# 测试 1: 检查帮助信息（验证参数解析）
# ============================================================
Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  测试 1: 验证命令行参数识别                             ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

Write-Host "▶️  测试无效参数（应输出错误或启动 GUI）..." -ForegroundColor Yellow
$process = Start-Process -FilePath $exePath -ArgumentList "--help" -PassThru -NoNewWindow -Wait
Write-Host "  退出码: $($process.ExitCode)" -ForegroundColor Gray
Write-Host ""

# ============================================================
# 测试 2: MFT Service - 仅扫描模式
# ============================================================
if ($isAdmin) {
    Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║  测试 2: MFT Service - 仅扫描模式                        ║" -ForegroundColor Cyan
    Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""

    # 创建测试输出目录
    if (Test-Path $testOutput) {
        Write-Host "🗑️  清理旧测试数据..." -ForegroundColor Yellow
        Remove-Item -Recurse -Force $testOutput
    }
    New-Item -ItemType Directory -Path $testOutput | Out-Null

    Write-Host "▶️  启动扫描（仅 C 盘，--scan-only）..." -ForegroundColor Yellow
    Write-Host "  命令: ilauncher.exe --mft-service --drives C --output '$testOutput' --scan-only" -ForegroundColor Gray
    Write-Host ""

    $logFile = "$testOutput\scan.log"
    $process = Start-Process -FilePath $exePath `
        -ArgumentList "--mft-service", "--drives", "C", "--output", $testOutput, "--scan-only" `
        -PassThru `
        -NoNewWindow `
        -RedirectStandardOutput $logFile `
        -RedirectStandardError "$testOutput\scan_error.log"

    # 等待最多 120 秒
    $timeout = 120
    $elapsed = 0
    while (-not $process.HasExited -and $elapsed -lt $timeout) {
        Start-Sleep -Seconds 1
        $elapsed++
        if ($elapsed % 10 -eq 0) {
            Write-Host "  ⏱️  已运行 $elapsed 秒..." -ForegroundColor Gray
        }
    }

    if ($process.HasExited) {
        Write-Host "✓ 扫描完成" -ForegroundColor Green
        Write-Host "  退出码: $($process.ExitCode)" -ForegroundColor Gray
        
        # 检查日志
        if (Test-Path $logFile) {
            Write-Host ""
            Write-Host "📄 扫描日志（最后 30 行）:" -ForegroundColor Cyan
            Get-Content $logFile -Tail 30 | ForEach-Object {
                Write-Host "  $_" -ForegroundColor White
            }
        }
        
        # 检查数据库
        Write-Host ""
        Write-Host "📁 生成的数据库文件:" -ForegroundColor Cyan
        $dbFiles = Get-ChildItem "$testOutput\*.db" -ErrorAction SilentlyContinue
        if ($dbFiles) {
            $dbFiles | ForEach-Object {
                $sizeMB = [math]::Round($_.Length / 1MB, 2)
                Write-Host "  ✓ $($_.Name) ($sizeMB MB)" -ForegroundColor Green
            }
        } else {
            Write-Host "  ⚠️  未找到数据库文件" -ForegroundColor Yellow
        }
    } else {
        Write-Host "⚠️  超时（120秒），强制停止..." -ForegroundColor Yellow
        $process.Kill()
    }
    
    Write-Host ""
} else {
    Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Yellow
    Write-Host "║  测试 2: MFT Service - 跳过（需要管理员权限）            ║" -ForegroundColor Yellow
    Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Yellow
    Write-Host ""
}

# ============================================================
# 测试 3: MFT Service - 监控模式（交互式）
# ============================================================
if ($isAdmin) {
    Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║  测试 3: MFT Service - 监控模式（可选）                  ║" -ForegroundColor Cyan
    Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "💡 此测试会启动监控模式，需要手动按 Ctrl+C 停止" -ForegroundColor Yellow
    Write-Host "是否运行监控测试？(Y/N)" -ForegroundColor Yellow
    
    $response = Read-Host
    if ($response -eq 'Y' -or $response -eq 'y') {
        Write-Host ""
        Write-Host "▶️  启动监控模式（按 Ctrl+C 停止）..." -ForegroundColor Yellow
        Write-Host "  命令: ilauncher.exe --mft-service --drives C --output '$testOutput'" -ForegroundColor Gray
        Write-Host ""
        
        & $exePath --mft-service --drives C --output $testOutput
    } else {
        Write-Host "⏭️  跳过监控测试" -ForegroundColor Gray
    }
    Write-Host ""
}

# ============================================================
# 测试 4: GUI 模式（非阻塞启动）
# ============================================================
Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  测试 4: GUI 模式（可选）                                ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""
Write-Host "是否启动 GUI 模式测试窗口？(Y/N)" -ForegroundColor Yellow

$response = Read-Host
if ($response -eq 'Y' -or $response -eq 'y') {
    Write-Host ""
    Write-Host "▶️  启动 GUI 窗口..." -ForegroundColor Yellow
    Write-Host "  命令: ilauncher.exe" -ForegroundColor Gray
    Write-Host ""
    
    $process = Start-Process -FilePath $exePath -PassThru
    Write-Host "✓ GUI 进程已启动 (PID: $($process.Id))" -ForegroundColor Green
    Write-Host "  请手动关闭窗口以继续..." -ForegroundColor Yellow
    
    $process.WaitForExit()
    Write-Host "✓ GUI 窗口已关闭" -ForegroundColor Green
} else {
    Write-Host "⏭️  跳过 GUI 测试" -ForegroundColor Gray
}
Write-Host ""

# ============================================================
# 测试总结
# ============================================================
Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║  🎉 测试完成！                                           ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

Write-Host "📊 测试结果总结:" -ForegroundColor Cyan
Write-Host "  ✓ 可执行文件: $exePath" -ForegroundColor Green
Write-Host "  ✓ 文件大小: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor Green

if ($isAdmin) {
    if (Test-Path "$testOutput\*.db") {
        Write-Host "  ✓ MFT 扫描测试: 通过" -ForegroundColor Green
        Write-Host "  ✓ 测试数据: $testOutput" -ForegroundColor Green
    } else {
        Write-Host "  ⚠️  MFT 扫描测试: 未生成数据库" -ForegroundColor Yellow
    }
} else {
    Write-Host "  ⏭️  MFT 测试: 跳过（需要管理员权限）" -ForegroundColor Gray
}

Write-Host ""
Write-Host "📖 使用指南:" -ForegroundColor Cyan
Write-Host "  • GUI 模式:       .\ilauncher.exe" -ForegroundColor White
Write-Host "  • MFT 扫描:       .\ilauncher.exe --mft-service --scan-only" -ForegroundColor White
Write-Host "  • MFT 监控:       .\ilauncher.exe --mft-service" -ForegroundColor White
Write-Host "  • 详细文档:       查看 ILAUNCHER_USAGE.md" -ForegroundColor White
Write-Host ""
