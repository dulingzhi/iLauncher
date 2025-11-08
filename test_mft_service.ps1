# MFT Service 测试脚本
# 需要以管理员身份运行 PowerShell

Write-Host "🧪 MFT Service 测试脚本" -ForegroundColor Cyan
Write-Host "=" * 50

# 检查管理员权限
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "❌ 错误: 需要管理员权限" -ForegroundColor Red
    Write-Host "请右键选择 '以管理员身份运行' PowerShell" -ForegroundColor Yellow
    exit 1
}

Write-Host "✓ 管理员权限检查通过" -ForegroundColor Green

# 设置路径
$projectRoot = Split-Path -Parent $PSScriptRoot
$exePath = Join-Path $projectRoot "src-tauri\target\release\mft_service.exe"
$testOutput = Join-Path $env:TEMP "mft_test_db"

# 检查可执行文件
if (-not (Test-Path $exePath)) {
    Write-Host "❌ 错误: 找不到 mft_service.exe" -ForegroundColor Red
    Write-Host "路径: $exePath" -ForegroundColor Yellow
    Write-Host "请先编译: cargo build --bin mft_service --release" -ForegroundColor Yellow
    exit 1
}

Write-Host "✓ 找到可执行文件: $exePath" -ForegroundColor Green

# 创建测试输出目录
if (Test-Path $testOutput) {
    Write-Host "🗑️  清理旧测试数据..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force $testOutput
}

New-Item -ItemType Directory -Path $testOutput | Out-Null
Write-Host "✓ 测试输出目录: $testOutput" -ForegroundColor Green

Write-Host ""
Write-Host "╔═══════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║    测试 1: 仅扫描模式（--scan-only）      ║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# 测试 1: 仅扫描 C 盘
Write-Host "▶️  启动扫描（仅 C 盘，10 秒后自动退出）..." -ForegroundColor Yellow

$process = Start-Process -FilePath $exePath `
    -ArgumentList "--output", $testOutput, "--drives", "C", "--scan-only" `
    -PassThru `
    -NoNewWindow `
    -RedirectStandardOutput "$testOutput\scan.log" `
    -RedirectStandardError "$testOutput\scan_error.log"

# 等待最多 60 秒
$timeout = 60
$elapsed = 0
while (-not $process.HasExited -and $elapsed -lt $timeout) {
    Start-Sleep -Seconds 1
    $elapsed++
    if ($elapsed % 5 -eq 0) {
        Write-Host "  ⏱️  已运行 $elapsed 秒..." -ForegroundColor Gray
    }
}

if ($process.HasExited) {
    Write-Host "✓ 扫描完成，退出码: $($process.ExitCode)" -ForegroundColor Green
    
    # 检查输出日志
    if (Test-Path "$testOutput\scan.log") {
        Write-Host ""
        Write-Host "📄 扫描日志（最后 20 行）:" -ForegroundColor Cyan
        Get-Content "$testOutput\scan.log" -Tail 20
    }
    
    # 检查数据库文件
    Write-Host ""
    Write-Host "📁 生成的数据库文件:" -ForegroundColor Cyan
    Get-ChildItem "$testOutput\*.db" -ErrorAction SilentlyContinue | ForEach-Object {
        $sizeMB = [math]::Round($_.Length / 1MB, 2)
        Write-Host "  - $($_.Name) ($sizeMB MB)" -ForegroundColor White
    }
} else {
    Write-Host "⚠️  超时（60秒），强制停止进程..." -ForegroundColor Yellow
    $process.Kill()
}

Write-Host ""
Write-Host "╔═══════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║    测试 2: 监控模式（手动按 Ctrl+C）      ║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""
Write-Host "💡 提示: 此测试会启动监控模式，需要手动按 Ctrl+C 停止" -ForegroundColor Yellow
Write-Host "是否运行监控测试？(Y/N)" -ForegroundColor Yellow

$response = Read-Host
if ($response -eq 'Y' -or $response -eq 'y') {
    Write-Host ""
    Write-Host "▶️  启动监控模式（按 Ctrl+C 停止）..." -ForegroundColor Yellow
    Write-Host ""
    
    & $exePath --output $testOutput --drives C
} else {
    Write-Host "⏭️  跳过监控测试" -ForegroundColor Gray
}

Write-Host ""
Write-Host "🎉 测试完成！" -ForegroundColor Green
Write-Host "测试数据位置: $testOutput" -ForegroundColor Cyan
