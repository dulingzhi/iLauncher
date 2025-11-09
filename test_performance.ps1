# 性能测试脚本

Write-Host "=== iLauncher 性能测试 ===" -ForegroundColor Cyan

# 1. 启动 UI
Write-Host "`n1. 启动 UI..." -ForegroundColor Yellow
$uiProcess = Start-Process -FilePath ".\target\release\ilauncher.exe" -PassThru -WindowStyle Normal
Write-Host "  ✓ UI 启动 (PID: $($uiProcess.Id))" -ForegroundColor Green

# 2. 等待 Service 启动
Write-Host "`n2. 等待 MFT Service 启动..." -ForegroundColor Yellow
$maxWait = 10
$waited = 0
$serviceFound = $false

while ($waited -lt $maxWait) {
    Start-Sleep -Seconds 1
    $waited++
    
    $allProcesses = Get-Process ilauncher -ErrorAction SilentlyContinue
    if ($allProcesses.Count -ge 2) {
        $serviceFound = $true
        Write-Host "  ✓ 检测到 MFT Service 启动" -ForegroundColor Green
        
        foreach ($p in $allProcesses) {
            $cmdLine = (Get-CimInstance Win32_Process -Filter "ProcessId = $($p.Id)" -ErrorAction SilentlyContinue).CommandLine
            if ($cmdLine -like "*--mft-service*") {
                Write-Host "    Service PID: $($p.Id)" -ForegroundColor Magenta
            } else {
                Write-Host "    UI PID: $($p.Id)" -ForegroundColor Cyan
            }
        }
        break
    }
    
    Write-Host "  ⏳ 等待中... ($waited/$maxWait)" -ForegroundColor Gray
}

if (-not $serviceFound) {
    Write-Host "  ⚠ 警告: 未检测到 MFT Service 启动" -ForegroundColor Yellow
}

# 3. 等待用户测试
Write-Host "`n3. 开始测试搜索性能..." -ForegroundColor Yellow
Write-Host "  请在 UI 中进行搜索测试" -ForegroundColor White
Write-Host "  按 Ctrl+C 结束测试" -ForegroundColor Yellow

# 监控进程
while ($true) {
    Start-Sleep -Seconds 5
    $currentProcesses = Get-Process ilauncher -ErrorAction SilentlyContinue
    if (-not $currentProcesses) {
        Write-Host "`n所有 ilauncher 进程已退出" -ForegroundColor Yellow
        break
    }
}

Write-Host "`n=== 测试结束 ===" -ForegroundColor Cyan
