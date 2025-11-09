# 测试 MFT 扫描速度
Write-Host "╔═══════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║      MFT Scanner Speed Test                       ║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# 1. 清理旧进程
Write-Host "🧹 Cleaning up old processes..." -ForegroundColor Yellow
taskkill /F /IM ilauncher.exe 2>$null
Start-Sleep -Seconds 2

# 2. 删除旧数据库（强制重新扫描）
Write-Host "🗑️  Deleting old databases..." -ForegroundColor Yellow
Remove-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db-*" -Force -ErrorAction SilentlyContinue

# 3. 启动 MFT Service
Write-Host "🚀 Starting MFT Service (请允许 UAC 提示)..." -ForegroundColor Cyan
$startTime = Get-Date

$process = Start-Process -FilePath ".\src-tauri\target\release\ilauncher.exe" `
    -ArgumentList "--mft-service", "--ui-pid", "99999", "--scan-only" `
    -Verb RunAs `
    -PassThru `
    -WindowStyle Hidden

# 4. 监控日志文件
Write-Host "📊 Monitoring scan progress..." -ForegroundColor Green
Write-Host ""

$logFile = "$env:LOCALAPPDATA\iLauncher\logs\mft_service.log"
$lastSize = 0
$scanComplete = $false
$timeout = 180  # 3 分钟超时

for ($i = 0; $i -lt $timeout; $i++) {
    Start-Sleep -Seconds 1
    
    if (Test-Path $logFile) {
        $currentSize = (Get-Item $logFile).Length
        if ($currentSize -gt $lastSize) {
            # 读取新增内容
            $content = Get-Content $logFile -Tail 10
            
            foreach ($line in $content) {
                if ($line -like "*Total scan time:*") {
                    if ($line -match "(\d+\.\d+)s") {
                    $endTime = Get-Date
                    $elapsed = ($endTime - $startTime).TotalSeconds
                    
                    Write-Host ""
                    Write-Host "╔═══════════════════════════════════════════════════╗" -ForegroundColor Green
                    Write-Host "║      Scan Complete!                               ║" -ForegroundColor Green
                    Write-Host "╚═══════════════════════════════════════════════════╝" -ForegroundColor Green
                    Write-Host "⏱️  Total time: $([math]::Round($elapsed, 2))s" -ForegroundColor Cyan
                    Write-Host "📝 Log time: $($matches[1])s" -ForegroundColor Cyan
                    
                    $scanComplete = $true
                    break
                }
                elseif ($line -like "*Progress:*files saved*") {
                    if ($line -match "(\d+) files") {
                        Write-Host "  💾 Saved $($matches[1]) files..." -ForegroundColor Gray
                    }
                }
                elseif ($line -like "*Building FRN map:*") {
                    if ($line -match "(\d+) entries") {
                        Write-Host "  🔍 Building index: $($matches[1]) entries" -ForegroundColor Gray
                    }
                }
            }
            
            $lastSize = $currentSize
        }
    }
    
    if ($scanComplete) { break }
    
    # 检查进程是否还在运行
    if ($process.HasExited) {
        Write-Host "⚠️  Process exited unexpectedly" -ForegroundColor Red
        break
    }
}

if (-not $scanComplete) {
    Write-Host "❌ Timeout after ${timeout}s" -ForegroundColor Red
}

# 5. 清理进程
Write-Host ""
Write-Host "🧹 Cleaning up..." -ForegroundColor Yellow
Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue

# 6. 显示数据库大小
Write-Host ""
Write-Host "📊 Database sizes:" -ForegroundColor Cyan
Get-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db" -ErrorAction SilentlyContinue | 
    Select-Object Name, @{Name="Size(MB)";Expression={[math]::Round($_.Length/1MB, 2)}} |
    Format-Table -AutoSize

Write-Host ""
Write-Host "✅ Test complete! Check the log for details:" -ForegroundColor Green
Write-Host "   $logFile" -ForegroundColor Gray
