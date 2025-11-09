# Simple MFT Scan Speed Test
Write-Host "Starting MFT Scanner Speed Test..." -ForegroundColor Cyan

# Cleanup
Write-Host "Cleaning up..." -ForegroundColor Yellow
taskkill /F /IM ilauncher.exe 2>$null | Out-Null
Start-Sleep -Seconds 2

# Delete old databases
Write-Host "Deleting old databases..." -ForegroundColor Yellow
Remove-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db*" -Force -ErrorAction SilentlyContinue

# Start scan
Write-Host "Starting scan (allow UAC prompt)..." -ForegroundColor Green
Write-Host ""

$startTime = Get-Date
Start-Process -FilePath ".\src-tauri\target\release\ilauncher.exe" `
    -ArgumentList "--mft-service", "--ui-pid", "99999", "--scan-only" `
    -Verb RunAs `
    -WindowStyle Normal

# Wait and monitor
$logFile = "$env:LOCALAPPDATA\iLauncher\logs\mft_service.log"
Write-Host "Waiting for scan to complete..."
Write-Host "Monitor log: $logFile"
Write-Host ""

# Simple wait loop
$maxWait = 180
for ($i = 0; $i -lt $maxWait; $i++) {
    Start-Sleep -Seconds 1
    
    if (Test-Path $logFile) {
        $lastLine = Get-Content $logFile -Tail 1
        if ($lastLine -like "*Scan Phase Complete*" -or $lastLine -like "*Total scan time*") {
            $endTime = Get-Date
            $elapsed = ($endTime - $startTime).TotalSeconds
            
            Write-Host ""
            Write-Host "=== SCAN COMPLETE ===" -ForegroundColor Green
            Write-Host "Total time: $([math]::Round($elapsed, 2)) seconds" -ForegroundColor Cyan
            break
        }
    }
    
    if ($i % 10 -eq 0) {
        Write-Host "  Waiting... $i seconds" -ForegroundColor Gray
    }
}

# Show database sizes
Write-Host ""
Write-Host "Database sizes:" -ForegroundColor Cyan
Get-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db" -ErrorAction SilentlyContinue | 
    Select-Object Name, @{Name="MB";Expression={[math]::Round($_.Length/1MB,2)}} | 
    Format-Table

Write-Host "View full log: $logFile" -ForegroundColor Gray
