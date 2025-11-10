# 测试 MRU 优先显示功能
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "MRU 优先显示功能测试" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$logFile = "$env:LOCALAPPDATA\iLauncher\logs\ilauncher.log"

Write-Host "📝 测试步骤:" -ForegroundColor Yellow
Write-Host "1. 启动 iLauncher" -ForegroundColor White
Write-Host "2. 搜索 'opera.exe' 或任意程序" -ForegroundColor White
Write-Host "3. 按 Enter 运行（会记录到 MRU）" -ForegroundColor White
Write-Host "4. 关闭窗口，再次打开" -ForegroundColor White
Write-Host "5. 搜索 'ope'（部分匹配）" -ForegroundColor White
Write-Host "6. 检查 opera.exe 是否排在第一位" -ForegroundColor White
Write-Host ""

Write-Host "� 日志文件:" -ForegroundColor Yellow
Write-Host "  $logFile" -ForegroundColor Gray
Write-Host ""

# 清空旧日志（可选）
$clearLog = Read-Host "是否清空旧日志便于观察？(y/n)"
if ($clearLog -eq 'y') {
    if (Test-Path $logFile) {
        Remove-Item $logFile -Force
        Write-Host "✓ 已清空旧日志" -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "� 正在启动应用..." -ForegroundColor Green
Write-Host "⚡ 请在应用中测试 MRU 功能" -ForegroundColor Cyan
Write-Host "⚡ 应用关闭后将自动显示相关日志" -ForegroundColor Cyan
Write-Host ""

# 启动应用（Release 模式）
cd "$PSScriptRoot\src-tauri"
$env:RUST_LOG = "ilauncher=debug"

# 启动进程并等待退出
$process = Start-Process -FilePath "cargo" -ArgumentList "run","--release" -PassThru -NoNewWindow
$process.WaitForExit()

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "应用已关闭，分析日志..." -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 检查日志文件
if (!(Test-Path $logFile)) {
    Write-Host "❌ 日志文件不存在: $logFile" -ForegroundColor Red
    exit 1
}

# 提取 MRU 相关日志
Write-Host "🔍 MRU 相关日志:" -ForegroundColor Yellow
Write-Host ""

$mruLogs = Get-Content $logFile | Where-Object {
    $_ -match "MRU|Query completed|boosted|record_result_click"
} | Select-Object -Last 50

if ($mruLogs.Count -eq 0) {
    Write-Host "⚠️  没有找到 MRU 相关日志" -ForegroundColor Yellow
    Write-Host "💡 请确保:" -ForegroundColor Cyan
    Write-Host "   1. 已经搜索并运行过程序" -ForegroundColor White
    Write-Host "   2. 再次搜索时能看到 MRU 提升效果" -ForegroundColor White
} else {
    foreach ($log in $mruLogs) {
        if ($log -match "MRU boosted|score 10\d{2}") {
            Write-Host $log -ForegroundColor Green
        } elseif ($log -match "Query completed") {
            Write-Host $log -ForegroundColor Cyan
        } elseif ($log -match "record_result_click") {
            Write-Host $log -ForegroundColor Magenta
        } elseif ($log -match "⚠|warning") {
            Write-Host $log -ForegroundColor Yellow
        } else {
            Write-Host $log -ForegroundColor White
        }
    }
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "📄 完整日志文件: $logFile" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
