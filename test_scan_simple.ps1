# MFT Scan Performance Test - Simple Version
Write-Host "=== MFT Scan Performance Test ===" -ForegroundColor Cyan
Write-Host ""

# 1. Backup databases
$dbPath = "$env:LOCALAPPDATA\iLauncher\mft_databases"
$backupPath = "$env:LOCALAPPDATA\iLauncher\mft_databases_backup_$(Get-Date -Format 'yyyyMMdd_HHmmss')"

if (Test-Path $dbPath) {
    Write-Host "Backing up databases..." -ForegroundColor Yellow
    Copy-Item -Path $dbPath -Destination $backupPath -Recurse
    Write-Host "   Backup: $backupPath" -ForegroundColor Green
    Write-Host ""
}

# 2. Get baseline
Write-Host "Baseline Database Size:" -ForegroundColor Cyan
if (Test-Path "$dbPath\*.db") {
    Get-Item "$dbPath\*.db" | 
        Select-Object Name, @{Name="MB";Expression={[math]::Round($_.Length/1MB,2)}} | 
        Format-Table -AutoSize
}
Write-Host ""

# 3. Delete databases for clean test
Write-Host "Deleting databases for clean test..." -ForegroundColor Yellow
Remove-Item -Path "$dbPath\*.db" -Force -ErrorAction SilentlyContinue
Write-Host ""

# 4. Start app and trigger scan
Write-Host "Starting app to trigger MFT scan..." -ForegroundColor Cyan
Write-Host "Please wait for scan to complete (app will show progress)..." -ForegroundColor Yellow
Write-Host ""

Set-Location "$PSScriptRoot\src-tauri"
$scanStart = Get-Date

# Run release build
& ".\target\release\ilauncher.exe"

Write-Host ""
Write-Host "Press Enter after scan completes..." -ForegroundColor Yellow
Read-Host

$scanEnd = Get-Date
$scanDuration = ($scanEnd - $scanStart).TotalSeconds

# 5. Analyze results
Write-Host ""
Write-Host "=== Scan Results ===" -ForegroundColor Cyan
Write-Host "Total time: $([math]::Round($scanDuration, 2)) seconds" -ForegroundColor Green
Write-Host ""

Write-Host "Database Size:" -ForegroundColor Cyan
Get-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db" | 
    Select-Object Name, LastWriteTime, @{Name="MB";Expression={[math]::Round($_.Length/1MB,2)}} | 
    Format-Table -AutoSize

Set-Location $PSScriptRoot
