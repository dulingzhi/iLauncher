# 完全自动化的测试脚本（使用 .NET Process）
Write-Host "╔════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║          Automated Scanner Memory Test                    ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

$exePath = ".\src-tauri\target\release\ilauncher.exe"

if (!(Test-Path $exePath)) {
    Write-Host "❌ Executable not found: $exePath" -ForegroundColor Red
    exit 1
}

Write-Host "🚀 Starting test..." -ForegroundColor Green
Write-Host "📊 Monitoring memory usage..." -ForegroundColor Yellow
Write-Host ""

# 创建进程配置
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exePath
$psi.Arguments = "--test-memory"
$psi.UseShellExecute = $false
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.CreateNoWindow = $false

# 启动进程
$process = New-Object System.Diagnostics.Process
$process.StartInfo = $psi
$process.Start() | Out-Null

Write-Host "✓ Process started (PID: $($process.Id))" -ForegroundColor Green
Start-Sleep -Seconds 1

# 发送第一个 Enter（开始扫描）
Write-Host "⏎ Sending Enter to start scan..." -ForegroundColor Yellow
$process.StandardInput.WriteLine()
$process.StandardInput.Flush()

# 监控内存
Write-Host ""
Write-Host "Time(s)`tMemory(MB)" -ForegroundColor Cyan
$maxMemory = 0
$startTime = Get-Date
$samples = @()

while (!$process.HasExited) {
    try {
        $process.Refresh()
        $memoryMB = [math]::Round($process.WorkingSet64 / 1MB, 2)
        
        if ($memoryMB -gt $maxMemory) {
            $maxMemory = $memoryMB
        }
        
        $elapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)
        $samples += @{Time = $elapsed; Memory = $memoryMB}
        
        # 每 2 秒输出一次
        if ($samples.Count % 4 -eq 0) {
            Write-Host "$elapsed`t$memoryMB"
        }
        
        Start-Sleep -Milliseconds 500
        
        # 超时保护（10 分钟）
        if ($elapsed -gt 600) {
            Write-Host "⚠️  Timeout reached, stopping..." -ForegroundColor Yellow
            $process.Kill()
            break
        }
    } catch {
        break
    }
}

# 发送第二个 Enter（退出）
if (!$process.HasExited) {
    Write-Host "⏎ Sending Enter to exit..." -ForegroundColor Yellow
    $process.StandardInput.WriteLine()
    $process.StandardInput.Flush()
    $process.WaitForExit(5000)
}

# 读取输出
$output = $process.StandardOutput.ReadToEnd()
$errors = $process.StandardError.ReadToEnd()

# 显示结果
Write-Host ""
Write-Host "╔════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║          Test Results                                      ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""
Write-Host "📊 Peak Memory: $maxMemory MB" -ForegroundColor $(if ($maxMemory -le 500) { 'Green' } else { 'Red' })
Write-Host ""

if ($maxMemory -le 500) {
    Write-Host "✅ PASSED: Memory usage is within 500MB limit!" -ForegroundColor Green
} else {
    Write-Host "❌ FAILED: Memory usage exceeded 500MB limit!" -ForegroundColor Red
}

# 显示内存峰值时间点
$peakSample = $samples | Where-Object { $_.Memory -eq $maxMemory } | Select-Object -First 1
if ($peakSample) {
    Write-Host "🔝 Peak occurred at: $($peakSample.Time)s" -ForegroundColor Yellow
}

# 显示程序输出
if ($output) {
    Write-Host ""
    Write-Host "Program Output:" -ForegroundColor Gray
    Write-Host $output -ForegroundColor Gray
}

if ($errors) {
    Write-Host ""
    Write-Host "Errors:" -ForegroundColor Red
    Write-Host $errors -ForegroundColor Red
}

# 清理
$process.Dispose()

Write-Host ""
Write-Host "Test completed." -ForegroundColor Cyan
