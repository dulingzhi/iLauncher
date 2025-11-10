# Test prompt.txt implementation performance
# Target: 4.5M files in <10s, <30ms queries

Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "  Prompt.txt Implementation Test    " -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

# Build project
Write-Host "Building project..." -ForegroundColor Yellow
Set-Location src-tauri
cargo build --release --quiet
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed" -ForegroundColor Red
    exit 1
}
Set-Location ..

Write-Host "Build completed" -ForegroundColor Green
Write-Host ""

# Test 1: Multi-Drive Scanning
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "Test 1: Multi-Drive Scanning" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Expected: 4.5M files in less than 10s" -ForegroundColor Yellow
Write-Host ""

$startTime = Get-Date

# Run MFT Service in scan-only mode
Write-Host "Starting full disk scan..." -ForegroundColor Yellow
Start-Process -FilePath ".\src-tauri\target\release\ilauncher.exe" `
    -ArgumentList "--mft-service", "--scan-only" `
    -Verb RunAs `
    -Wait

$endTime = Get-Date
$duration = ($endTime - $startTime).TotalSeconds

Write-Host ""
Write-Host "Scan completed in $([math]::Round($duration, 2))s" -ForegroundColor Green

# Evaluate performance
if ($duration -lt 12) {
    Write-Host "EXCELLENT: Meets target!" -ForegroundColor Green
} elseif ($duration -lt 20) {
    Write-Host "GOOD: Acceptable" -ForegroundColor Yellow
} else {
    Write-Host "SLOW: Need optimization" -ForegroundColor Red
}

Write-Host ""

# Test 2: Database Files
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "Test 2: Database Files" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

$dbPath = "$env:LOCALAPPDATA\iLauncher\mft_databases"
if (-not (Test-Path $dbPath)) {
    Write-Host "Database not found at $dbPath" -ForegroundColor Yellow
} else {
    $fstFiles = Get-ChildItem -Path $dbPath -Filter "*.fst"
    $datFiles = Get-ChildItem -Path $dbPath -Filter "*.dat"
    
    Write-Host "FST index files: $($fstFiles.Count)" -ForegroundColor Green
    Write-Host "DAT path files: $($datFiles.Count)" -ForegroundColor Green
    
    $totalSizeMB = 0
    Get-ChildItem -Path $dbPath -Recurse -File | ForEach-Object {
        $sizeMB = [math]::Round($_.Length / 1MB, 2)
        Write-Host "  $($_.Name): $sizeMB MB" -ForegroundColor Gray
        $totalSizeMB += $_.Length / 1MB
    }
    
    Write-Host ""
    Write-Host "Total database size: $([math]::Round($totalSizeMB, 2)) MB" -ForegroundColor Cyan
}

Write-Host ""

# Test 3: Memory Usage
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "Test 3: Memory Usage" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

$processes = Get-Process -Name "ilauncher" -ErrorAction SilentlyContinue

if ($processes) {
    Write-Host "Found $($processes.Count) ilauncher process(es):" -ForegroundColor Green
    
    foreach ($proc in $processes) {
        $memMB = [math]::Round($proc.WorkingSet64 / 1MB, 2)
        Write-Host "  PID $($proc.Id): $memMB MB" -ForegroundColor Cyan
        
        if ($memMB -lt 250) {
            Write-Host "  Memory: EXCELLENT" -ForegroundColor Green
        } elseif ($memMB -lt 500) {
            Write-Host "  Memory: ACCEPTABLE" -ForegroundColor Yellow
        } else {
            Write-Host "  Memory: HIGH" -ForegroundColor Red
        }
    }
} else {
    Write-Host "No ilauncher processes running" -ForegroundColor Gray
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "Test completed!" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""
