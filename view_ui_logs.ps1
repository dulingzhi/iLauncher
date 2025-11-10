# 查看 iLauncher UI 日志
param(
    [string]$Filter = "MRU|Query",
    [int]$Lines = 50,
    [switch]$Follow
)

$logDir = "$env:LOCALAPPDATA\iLauncher\logs"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "iLauncher UI 日志查看器" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

if (!(Test-Path $logDir)) {
    Write-Host "❌ 日志目录不存在: $logDir" -ForegroundColor Red
    exit 1
}

# 查找最新的 ilauncher.log 文件
$logFiles = Get-ChildItem $logDir -Filter "ilauncher*.log" | Sort-Object LastWriteTime -Descending

if ($logFiles.Count -eq 0) {
    Write-Host "⚠️  没有找到 UI 日志文件" -ForegroundColor Yellow
    Write-Host "💡 请先运行一次应用以生成日志" -ForegroundColor Cyan
    exit 0
}

$logFile = $logFiles[0]

Write-Host "📁 日志目录: $logDir" -ForegroundColor Yellow
Write-Host "📄 日志文件: $($logFile.Name)" -ForegroundColor Yellow
Write-Host "📏 文件大小: $([math]::Round($logFile.Length / 1KB, 2)) KB" -ForegroundColor Yellow
Write-Host "🕐 最后更新: $($logFile.LastWriteTime)" -ForegroundColor Yellow
Write-Host ""

if ($Follow) {
    Write-Host "🔄 实时监控模式（按 Ctrl+C 退出）" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""
    
    Get-Content $logFile.FullName -Wait -Tail $Lines | Where-Object {
        $_ -match $Filter
    } | ForEach-Object {
        if ($_ -match "ERROR") {
            Write-Host $_ -ForegroundColor Red
        } elseif ($_ -match "WARN") {
            Write-Host $_ -ForegroundColor Yellow
        } elseif ($_ -match "MRU boosted") {
            Write-Host $_ -ForegroundColor Green
        } elseif ($_ -match "Query completed") {
            Write-Host $_ -ForegroundColor Cyan
        } else {
            Write-Host $_
        }
    }
} else {
    Write-Host "🔍 过滤条件: $Filter" -ForegroundColor Cyan
    Write-Host "📏 显示行数: $Lines" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""
    
    $logs = Get-Content $logFile.FullName | Where-Object {
        $_ -match $Filter
    } | Select-Object -Last $Lines
    
    if ($logs.Count -eq 0) {
        Write-Host "⚠️  没有匹配的日志" -ForegroundColor Yellow
    } else {
        Write-Host "找到 $($logs.Count) 条匹配日志:" -ForegroundColor Green
        Write-Host ""
        
        foreach ($log in $logs) {
            if ($log -match "ERROR") {
                Write-Host $log -ForegroundColor Red
            } elseif ($log -match "WARN") {
                Write-Host $log -ForegroundColor Yellow
            } elseif ($log -match "MRU boosted") {
                Write-Host $log -ForegroundColor Green
            } elseif ($log -match "Query completed") {
                Write-Host $log -ForegroundColor Cyan
            } elseif ($log -match "DEBUG") {
                Write-Host $log -ForegroundColor Gray
            } elseif ($log -match "record_result_click") {
                Write-Host $log -ForegroundColor Magenta
            } else {
                Write-Host $log
            }
        }
    }
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "💡 用法示例:" -ForegroundColor Yellow
Write-Host "  .\view_ui_logs.ps1                     # 查看 MRU 和 Query 日志" -ForegroundColor White
Write-Host "  .\view_ui_logs.ps1 -Filter 'error'     # 查看错误日志" -ForegroundColor White
Write-Host "  .\view_ui_logs.ps1 -Lines 100          # 显示最后 100 行" -ForegroundColor White
Write-Host "  .\view_ui_logs.ps1 -Follow             # 实时监控模式" -ForegroundColor White
Write-Host "========================================" -ForegroundColor Cyan
