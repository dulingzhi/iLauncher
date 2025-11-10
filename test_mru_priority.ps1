# 测试 MRU 优先显示功能
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "MRU 优先显示功能测试" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "📝 测试步骤:" -ForegroundColor Yellow
Write-Host "1. 启动 iLauncher" -ForegroundColor White
Write-Host "2. 搜索 'opera.exe' 或任意程序" -ForegroundColor White
Write-Host "3. 按 Enter 运行（会记录到 MRU）" -ForegroundColor White
Write-Host "4. 关闭窗口，再次打开" -ForegroundColor White
Write-Host "5. 搜索 'ope'（部分匹配）" -ForegroundColor White
Write-Host "6. 检查 opera.exe 是否排在第一位" -ForegroundColor White
Write-Host ""

Write-Host "🔍 查看详细日志:" -ForegroundColor Yellow
Write-Host "  日志位置: $env:LOCALAPPDATA\iLauncher\logs\" -ForegroundColor Gray
Write-Host "  搜索关键词: 'MRU boosted' 或 'MRU item matches search'" -ForegroundColor Gray
Write-Host ""

Write-Host "📊 数据库位置:" -ForegroundColor Yellow
Write-Host "  $env:LOCALAPPDATA\iLauncher\data\statistics.db" -ForegroundColor Gray
Write-Host ""

# 启动应用
Write-Host "🚀 正在启动应用（Debug 模式）..." -ForegroundColor Green
Write-Host ""

$env:RUST_LOG = "ilauncher=debug"
cd "$PSScriptRoot\src-tauri"

# 启动并捕获输出
Write-Host "⚡ 启动命令: cargo run --release" -ForegroundColor Cyan
Write-Host "⚡ 日志级别: DEBUG" -ForegroundColor Cyan
Write-Host "⚡ 重点关注: MRU 相关日志" -ForegroundColor Cyan
Write-Host ""
Write-Host "按 Ctrl+C 停止应用" -ForegroundColor Yellow
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

cargo run --release 2>&1 | Where-Object {
    $_ -match "MRU|Query completed|opera|boosted|result.*score"
} | ForEach-Object {
    if ($_ -match "MRU boosted") {
        Write-Host $_ -ForegroundColor Green
    } elseif ($_ -match "Query completed") {
        Write-Host $_ -ForegroundColor Cyan
    } elseif ($_ -match "warning|⚠") {
        Write-Host $_ -ForegroundColor Yellow
    } else {
        Write-Host $_
    }
}
