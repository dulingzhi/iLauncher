# MFT 扫描内存监控脚本
# 用于测试内存优化效果

param(
    [int]$IntervalSeconds = 1,
    [int]$DurationSeconds = 120
)

Write-Host "🔍 MFT 扫描内存监控工具" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════"
Write-Host ""
Write-Host "⚙️  配置:"
Write-Host "   采样间隔: $IntervalSeconds 秒"
Write-Host "   监控时长: $DurationSeconds 秒"
Write-Host ""
Write-Host "按 Ctrl+C 停止监控"
Write-Host "─────────────────────────────────────────────────────────────"
Write-Host ""

# 查找 ilauncher 进程
$processName = "ilauncher"
$startTime = Get-Date
$samples = @()
$maxMemory = 0
$peakTime = $null

Write-Host "等待 ilauncher 进程启动..." -ForegroundColor Yellow

# 等待进程启动
while (-not (Get-Process -Name $processName -ErrorAction SilentlyContinue)) {
    Start-Sleep -Milliseconds 500
    if ((Get-Date) -gt $startTime.AddSeconds(30)) {
        Write-Host "❌ 超时: 未检测到 ilauncher 进程" -ForegroundColor Red
        exit 1
    }
}

Write-Host "✅ 检测到进程,开始监控..." -ForegroundColor Green
Write-Host ""
Write-Host "时间(s) | 内存(MB) | CPU(%) | 工作集(MB) | 状态"
Write-Host "─────────────────────────────────────────────────────────────"

$monitorStart = Get-Date
$sampleCount = 0

try {
    while ($true) {
        $elapsed = (Get-Date) - $monitorStart
        
        if ($elapsed.TotalSeconds -gt $DurationSeconds) {
            break
        }
        
        $process = Get-Process -Name $processName -ErrorAction SilentlyContinue
        
        if ($process) {
            $memoryMB = [math]::Round($process.WorkingSet64 / 1MB, 2)
            $privateMemMB = [math]::Round($process.PrivateMemorySize64 / 1MB, 2)
            $cpu = [math]::Round($process.CPU, 1)
            
            # 记录峰值
            if ($memoryMB -gt $maxMemory) {
                $maxMemory = $memoryMB
                $peakTime = $elapsed.TotalSeconds
            }
            
            # 保存样本
            $samples += [PSCustomObject]@{
                Time = $elapsed.TotalSeconds
                Memory = $memoryMB
                PrivateMemory = $privateMemMB
                CPU = $cpu
            }
            
            # 显示实时数据
            $status = if ($cpu -gt 50) { "🔥" } elseif ($cpu -gt 20) { "⚡" } else { "💤" }
            Write-Host ("{0,7:F1} | {1,8:F2} | {2,6:F1} | {3,10:F2} | {4}" -f `
                $elapsed.TotalSeconds, $memoryMB, $cpu, $privateMemMB, $status)
            
            $sampleCount++
        } else {
            Write-Host "⚠️  进程已退出" -ForegroundColor Yellow
            break
        }
        
        Start-Sleep -Seconds $IntervalSeconds
    }
} catch {
    Write-Host ""
    Write-Host "⚠️  监控中断: $_" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════"
Write-Host "📊 统计报告" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════"

if ($samples.Count -gt 0) {
    $avgMemory = ($samples | Measure-Object -Property Memory -Average).Average
    $minMemory = ($samples | Measure-Object -Property Memory -Minimum).Minimum
    $avgCPU = ($samples | Measure-Object -Property CPU -Average).Average
    
    Write-Host ""
    Write-Host "📈 内存使用:"
    Write-Host "   峰值: $($maxMemory) MB (在 $($peakTime)s)"
    Write-Host "   平均: $([math]::Round($avgMemory, 2)) MB"
    Write-Host "   最低: $([math]::Round($minMemory, 2)) MB"
    Write-Host ""
    Write-Host "⚙️  CPU 使用:"
    Write-Host "   平均: $([math]::Round($avgCPU, 1))%"
    Write-Host ""
    Write-Host "📊 采样统计:"
    Write-Host "   样本数: $($samples.Count)"
    Write-Host "   监控时长: $([math]::Round($elapsed.TotalSeconds, 1)) 秒"
    Write-Host ""
    
    # 内存趋势分析
    if ($samples.Count -ge 3) {
        $first = $samples[0].Memory
        $last = $samples[-1].Memory
        $trend = $last - $first
        
        Write-Host "📉 内存趋势:"
        if ($trend -gt 0) {
            Write-Host "   ⬆️  增长 $([math]::Round($trend, 2)) MB" -ForegroundColor Yellow
        } elseif ($trend -lt 0) {
            Write-Host "   ⬇️  下降 $([math]::Round(-$trend, 2)) MB" -ForegroundColor Green
        } else {
            Write-Host "   ➡️  稳定" -ForegroundColor Green
        }
    }
    
    # 导出 CSV (可选)
    $exportPath = "$env:TEMP\ilauncher_memory_$((Get-Date).ToString('yyyyMMdd_HHmmss')).csv"
    $samples | Export-Csv -Path $exportPath -NoTypeInformation -Encoding UTF8
    Write-Host ""
    Write-Host "💾 详细数据已导出: $exportPath" -ForegroundColor Gray
} else {
    Write-Host "❌ 无监控数据" -ForegroundColor Red
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════"
