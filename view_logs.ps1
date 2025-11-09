# 查看 MFT Service 日志

$logFile = "$env:LOCALAPPDATA\iLauncher\logs\mft_service.log"

Write-Host "=== MFT Service 日志查看器 ===" -ForegroundColor Cyan
Write-Host "日志文件: $logFile" -ForegroundColor Gray
Write-Host ""

if (Test-Path $logFile) {
    Write-Host "显示最后 50 行:" -ForegroundColor Yellow
    Get-Content $logFile -Tail 50
    
    Write-Host ""
    Write-Host "=== 实时监控模式（按 Ctrl+C 退出）===" -ForegroundColor Cyan
    Get-Content $logFile -Wait -Tail 0
} else {
    Write-Host "日志文件不存在" -ForegroundColor Red
    Write-Host ""
    Write-Host "可能原因:" -ForegroundColor Yellow
    Write-Host "  1. MFT Service 未启动" -ForegroundColor Gray
    Write-Host "  2. Service 启动失败" -ForegroundColor Gray
    Write-Host "  3. 日志目录权限问题" -ForegroundColor Gray
    Write-Host ""
    
    # 检查进程
    $processes = Get-Process ilauncher -ErrorAction SilentlyContinue
    if ($processes) {
        Write-Host "检测到 $($processes.Count) 个 ilauncher 进程:" -ForegroundColor Green
        foreach ($p in $processes) {
            $cmdLine = (Get-CimInstance Win32_Process -Filter "ProcessId = $($p.Id)" -ErrorAction SilentlyContinue).CommandLine
            Write-Host "  PID $($p.Id): $cmdLine" -ForegroundColor Gray
        }
    } else {
        Write-Host "没有运行的 ilauncher 进程" -ForegroundColor Red
    }
}
