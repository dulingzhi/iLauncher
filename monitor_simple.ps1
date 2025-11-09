# 简单内存监控脚本
Write-Host "=== Memory Monitor ===" -ForegroundColor Cyan
Write-Host "Monitoring 'ilauncher' process..." -ForegroundColor Yellow
Write-Host ""

# 等待进程启动
Write-Host "Waiting for process to start..." -ForegroundColor Gray
$timeout = 30
for ($i = 0; $i -lt $timeout; $i++) {
    $proc = Get-Process -Name "ilauncher" -ErrorAction SilentlyContinue
    if ($proc) {
        Write-Host "✓ Process found (PID: $($proc.Id))" -ForegroundColor Green
        break
    }
    Start-Sleep -Seconds 1
}

if (!$proc) {
    Write-Host "✗ Process not found after ${timeout}s" -ForegroundColor Red
    exit 1
}

# 监控内存
Write-Host ""
Write-Host "Time(s)`tMemory(MB)" -ForegroundColor Yellow
$maxMemory = 0
$startTime = Get-Date

while (!$proc.HasExited) {
    try {
        $proc.Refresh()
        $memoryMB = [math]::Round($proc.WorkingSet64 / 1MB, 2)
        
        if ($memoryMB -gt $maxMemory) {
            $maxMemory = $memoryMB
        }
        
        $elapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)
        
        # 高亮显示峰值
        if ($memoryMB -eq $maxMemory -and $maxMemory -gt 100) {
            Write-Host "$elapsed`t$memoryMB (PEAK)" -ForegroundColor Red
        } else {
            Write-Host "$elapsed`t$memoryMB"
        }
        
        Start-Sleep -Milliseconds 500
    } catch {
        break
    }
}

# 总结
Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Cyan
Write-Host "Peak Memory: $maxMemory MB" -ForegroundColor $(if ($maxMemory -le 500) { 'Green' } else { 'Red' })

if ($maxMemory -le 500) {
    Write-Host "✅ PASSED: Within 500MB limit" -ForegroundColor Green
} else {
    Write-Host "❌ FAILED: Exceeded 500MB limit" -ForegroundColor Red
}
