# 自动化内存测试脚本
param(
    [char]$Drive = 'D'
)

Write-Host "╔════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║          Scanner Memory Automated Test                    ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

$exePath = ".\src-tauri\target\release\ilauncher.exe"

if (!(Test-Path $exePath)) {
    Write-Host "❌ Executable not found: $exePath" -ForegroundColor Red
    Write-Host "🔧 Building release version..." -ForegroundColor Yellow
    cd src-tauri
    cargo build --release
    cd ..
}

Write-Host "🚀 Starting scanner test for drive ${Drive}:" -ForegroundColor Green
Write-Host "📊 Monitoring memory usage..." -ForegroundColor Yellow
Write-Host ""

# 启动监控进程
$monitorJob = Start-Job -ScriptBlock {
    param($ProcessName)
    
    $maxMemory = 0
    $startTime = Get-Date
    
    # 等待进程启动
    $timeout = 30
    $elapsed = 0
    while ($elapsed -lt $timeout) {
        $proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
        if ($proc) {
            break
        }
        Start-Sleep -Milliseconds 500
        $elapsed++
    }
    
    if (!$proc) {
        return @{Error = "Process not found"}
    }
    
    # 监控内存
    $samples = @()
    while (!$proc.HasExited) {
        $proc.Refresh()
        $memoryMB = [math]::Round($proc.WorkingSet64 / 1MB, 2)
        
        if ($memoryMB -gt $maxMemory) {
            $maxMemory = $memoryMB
        }
        
        $elapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)
        $samples += @{Time = $elapsed; Memory = $memoryMB}
        
        Start-Sleep -Milliseconds 500
    }
    
    return @{
        MaxMemory = $maxMemory
        Samples = $samples
    }
} -ArgumentList "ilauncher"

# 运行测试（自动按 Enter）
Write-Host "⏱️  Waiting for process to start..." -ForegroundColor Cyan

# 使用后台进程自动发送 Enter
$inputJob = Start-Job -ScriptBlock {
    param($ExePath, $Drive)
    
    # 创建进程
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $ExePath
    $psi.Arguments = "--test-memory"
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.CreateNoWindow = $true
    
    $proc = [System.Diagnostics.Process]::Start($psi)
    
    # 等待启动提示
    Start-Sleep -Seconds 2
    
    # 发送第一个 Enter（开始扫描）
    $proc.StandardInput.WriteLine()
    
    # 等待扫描完成（假设最多 5 分钟）
    $proc.WaitForExit(300000)
    
    # 发送第二个 Enter（退出）
    if (!$proc.HasExited) {
        $proc.StandardInput.WriteLine()
        $proc.WaitForExit(5000)
    }
    
    $output = $proc.StandardOutput.ReadToEnd()
    return $output
} -ArgumentList $exePath, $Drive

# 等待测试完成
Write-Host "⏳ Test running..." -ForegroundColor Yellow
$result = Wait-Job $inputJob | Receive-Job
$memoryData = Wait-Job $monitorJob | Receive-Job

# 清理
Remove-Job $inputJob, $monitorJob -Force

# 显示结果
Write-Host ""
Write-Host "╔════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║          Test Results                                      ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

if ($memoryData.Error) {
    Write-Host "❌ Error: $($memoryData.Error)" -ForegroundColor Red
} else {
    $maxMB = $memoryData.MaxMemory
    Write-Host "📊 Peak Memory: $maxMB MB" -ForegroundColor $(if ($maxMB -le 500) { 'Green' } else { 'Red' })
    Write-Host ""
    
    if ($maxMB -le 500) {
        Write-Host "✅ PASSED: Memory usage is within 500MB limit!" -ForegroundColor Green
    } else {
        Write-Host "❌ FAILED: Memory usage exceeded 500MB limit!" -ForegroundColor Red
    }
    
    # 显示内存曲线（简化版）
    Write-Host ""
    Write-Host "Memory Timeline:" -ForegroundColor Yellow
    $samples = $memoryData.Samples | Select-Object -First 20
    for ($i = 0; $i -lt $samples.Count; $i += 2) {
        $sample = $samples[$i]
        $time = $sample.Time
        $mem = $sample.Memory
        $bar = "=" * [math]::Min(50, [math]::Floor($mem / 10))
        Write-Host ("{0,5}s  {1,6} MB  {2}" -f $time, $mem, $bar)
    }
}

Write-Host ""
Write-Host "Test output:" -ForegroundColor Gray
Write-Host $result -ForegroundColor Gray
