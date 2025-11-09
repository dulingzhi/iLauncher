# Rebuild MFT Database with new schema
Write-Host "🔄 Rebuilding MFT Database with File-Engine schema..." -ForegroundColor Cyan

# Kill processes
Write-Host "Stopping processes..." -ForegroundColor Yellow
taskkill /F /IM ilauncher.exe 2>$null | Out-Null
Start-Sleep -Seconds 2

# Delete old databases
Write-Host "Deleting old databases..." -ForegroundColor Yellow
Remove-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db*" -Force -ErrorAction SilentlyContinue

# Rebuild
Write-Host "Compiling..." -ForegroundColor Green
cd src-tauri
cargo build --release 2>&1 | Select-String -Pattern "error" -Context 1
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Compilation failed!" -ForegroundColor Red
    exit 1
}
cd ..

Write-Host "✓ Compiled successfully" -ForegroundColor Green
Write-Host ""
Write-Host "Starting full scan (allow UAC prompt)..." -ForegroundColor Cyan
Write-Host ""

$startTime = Get-Date
Start-Process -FilePath ".\src-tauri\target\release\ilauncher.exe" `
    -ArgumentList "--mft-service", "--ui-pid", "99999", "--scan-only" `
    -Verb RunAs `
    -WindowStyle Normal `
    -Wait

$endTime = Get-Date
$elapsed = ($endTime - $startTime).TotalSeconds

Write-Host ""
Write-Host "=== SCAN COMPLETE ===" -ForegroundColor Green
Write-Host "Total time: $([math]::Round($elapsed, 2)) seconds" -ForegroundColor Cyan

# Show database info
Write-Host ""
Write-Host "Database files:" -ForegroundColor Cyan
Get-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db" -ErrorAction SilentlyContinue | 
    Select-Object Name, @{Name="MB";Expression={[math]::Round($_.Length/1MB,2)}} | 
    Format-Table -AutoSize

# Check schema
Write-Host ""
Write-Host "Verifying schema (list0 table):" -ForegroundColor Cyan
$db = "$env:LOCALAPPDATA\iLauncher\mft_databases\C.db"
if (Test-Path $db) {
    sqlite3 $db ".schema list0"
} else {
    Write-Host "❌ C.db not found!" -ForegroundColor Red
}

Write-Host ""
Write-Host "View log: $env:LOCALAPPDATA\iLauncher\logs\mft_service.log" -ForegroundColor Gray
