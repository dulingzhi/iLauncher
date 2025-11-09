# Analyze MFT Scan Performance from Logs
Write-Host "=== MFT Scan Performance Analysis ===" -ForegroundColor Cyan
Write-Host ""

# 1. Check if log file exists
$logFile = "$env:LOCALAPPDATA\iLauncher\logs\mft_service.log"

if (!(Test-Path $logFile)) {
    Write-Host "Log file not found. Please run the app first." -ForegroundColor Red
    exit
}

# 2. Analyze scan times
Write-Host "Analyzing scan times from logs..." -ForegroundColor Yellow
Write-Host ""

$logContent = Get-Content $logFile

# Find scan start and completion times
$scanData = @{}

foreach ($line in $logContent) {
    # Match start: "Starting scan for drive X:"
    if ($line -match "(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z).*Starting scan for drive ([A-Z]):") {
        $timestamp = [DateTime]::Parse($Matches[1])
        $drive = $Matches[2]
        
        if (!$scanData.ContainsKey($drive)) {
            $scanData[$drive] = @()
        }
        
        $scanData[$drive] += @{
            Start = $timestamp
            End = $null
            Duration = $null
        }
    }
    
    # Match completion: "Drive X scan completed"
    if ($line -match "(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z).*Drive ([A-Z]) scan completed") {
        $timestamp = [DateTime]::Parse($Matches[1])
        $drive = $Matches[2]
        
        if ($scanData.ContainsKey($drive) -and $scanData[$drive].Count -gt 0) {
            $lastScan = $scanData[$drive][-1]
            if ($lastScan.End -eq $null -and $lastScan.Start -ne $null) {
                $lastScan.End = $timestamp
                $lastScan.Duration = ($timestamp - $lastScan.Start).TotalSeconds
            }
        }
    }
}

if ($scanData.Count -eq 0) {
    Write-Host "No scan records found in logs." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Trigger a new scan by:" -ForegroundColor Cyan
    Write-Host "   1. Delete databases: Remove-Item '$env:LOCALAPPDATA\iLauncher\mft_databases\*.db' -Force" -ForegroundColor White
    Write-Host "   2. Run app: .\src-tauri\target\release\ilauncher.exe" -ForegroundColor White
    exit
}

# 3. Display results
Write-Host "Scan Performance Results:" -ForegroundColor Cyan
Write-Host ""

$totalLatest = 0
foreach ($drive in $scanData.Keys | Sort-Object) {
    $scans = $scanData[$drive]
    $completedScans = $scans | Where-Object { $_.Duration -ne $null }
    
    if ($completedScans.Count -eq 0) {
        continue
    }
    
    $latestScan = $completedScans[-1]
    $avgDuration = ($completedScans | Measure-Object -Property Duration -Average).Average
    
    Write-Host "Drive $drive`:"
    Write-Host "   Latest scan: $([math]::Round($latestScan.Duration, 2)) seconds" -ForegroundColor Green
    if ($completedScans.Count > 1) {
        Write-Host "   Average: $([math]::Round($avgDuration, 2)) seconds (from $($completedScans.Count) scans)" -ForegroundColor Gray
    }
    Write-Host ""
    
    $totalLatest += $latestScan.Duration
}

# 4. Database size
Write-Host "Current Database Sizes:" -ForegroundColor Cyan
Get-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db" -ErrorAction SilentlyContinue | 
    Select-Object Name, LastWriteTime, @{Name="MB";Expression={[math]::Round($_.Length/1MB,2)}} | 
    Format-Table -AutoSize

# 5. Performance recommendations
Write-Host "Performance Summary:" -ForegroundColor Yellow
Write-Host "   Total scan time: $([math]::Round($totalLatest, 2)) seconds" -ForegroundColor White
if ($totalLatest -lt 30) {
    Write-Host "   Status: Excellent (< 30s)" -ForegroundColor Green
} elseif ($totalLatest -lt 60) {
    Write-Host "   Status: Good (30-60s)" -ForegroundColor Green
} elseif ($totalLatest -lt 120) {
    Write-Host "   Status: Acceptable (60-120s)" -ForegroundColor Yellow
} else {
    Write-Host "   Status: Needs optimization (> 120s)" -ForegroundColor Red
}
Write-Host ""

Write-Host "Optimization Suggestions:" -ForegroundColor Cyan
Write-Host "   - Current: Sequential scanning" -ForegroundColor White
Write-Host "   - Potential: Parallel scanning (3-5x faster)" -ForegroundColor Cyan
Write-Host "   - Potential: Incremental updates (10-100x faster after first scan)" -ForegroundColor Cyan
Write-Host ""
