# MFT 性能分析启动脚本
# 自动请求管理员权限

param(
    [switch]$Force
)

# 检查是否以管理员身份运行
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "🔐 请求管理员权限..." -ForegroundColor Yellow
    
    # 重新以管理员身份运行
    $scriptPath = $MyInvocation.MyCommand.Path
    Start-Process powershell.exe -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$scriptPath`" -Force" -Verb RunAs
    exit
}

Write-Host "✅ 管理员权限已获取" -ForegroundColor Green
Write-Host ""

# 切换到项目目录
$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

# 运行性能测试
Write-Host "🚀 开始性能分析..." -ForegroundColor Cyan
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
Write-Host ""

cd src-tauri
cargo run --release --example profile_mft_scan

Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
Write-Host "✅ 测试完成!" -ForegroundColor Green
Write-Host ""
Write-Host "按任意键退出..."
$null = $host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
