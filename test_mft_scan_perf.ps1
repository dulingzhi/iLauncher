# MFT Scan Performance Test
Write-Host "=== MFT First Scan Performance Test ===" -ForegroundColor Cyan
Write-Host ""

# 1. Backup existing databases
$dbPath = "$env:LOCALAPPDATA\iLauncher\mft_databases"
$backupPath = "$env:LOCALAPPDATA\iLauncher\mft_databases_backup_$(Get-Date -Format 'yyyyMMdd_HHmmss')"

if (Test-Path $dbPath) {
    Write-Host "Backing up existing databases..." -ForegroundColor Yellow
    Copy-Item -Path $dbPath -Destination $backupPath -Recurse
    Write-Host "   Backup location: $backupPath" -ForegroundColor Green
    Write-Host ""
    
    Write-Host "Deleting old databases..." -ForegroundColor Yellow
    Remove-Item -Path "$dbPath\*.db" -Force
    Write-Host "   Old databases deleted" -ForegroundColor Green
    Write-Host ""
}

# 2. Drive information
Write-Host "System Drives:" -ForegroundColor Cyan
$drives = Get-PSDrive -PSProvider FileSystem | Where-Object { $_.Used -ne $null }
foreach ($drive in $drives) {
    $totalGB = [math]::Round($drive.Used / 1GB + $drive.Free / 1GB, 2)
    $usedGB = [math]::Round($drive.Used / 1GB, 2)
    $freeGB = [math]::Round($drive.Free / 1GB, 2)
    $usedPercent = [math]::Round(($drive.Used / ($drive.Used + $drive.Free)) * 100, 1)
    
    Write-Host "   $($drive.Name): $totalGB GB (Used: $usedGB GB, Free: $freeGB GB, $usedPercent%)" -ForegroundColor White
}
Write-Host ""

# 3. Compile release build
Write-Host "Compiling release build..." -ForegroundColor Yellow
Set-Location "$PSScriptRoot\src-tauri"
$compileStart = Get-Date
cargo build --release 2>&1 | Out-Null
$compileEnd = Get-Date
$compileDuration = ($compileEnd - $compileStart).TotalSeconds
Write-Host "   Compile time: $([math]::Round($compileDuration, 2)) seconds" -ForegroundColor Green
Write-Host ""

# 4. Create test program
Write-Host "Creating scan performance test program..." -ForegroundColor Yellow
$testCode = @'
// MFT Scan Performance Test
use std::time::Instant;
use std::process::Command;

fn main() {
    println!("\n=== MFT Scan Performance Analysis ===\n");
    
    let drives = vec!['C', 'D', 'E'];
    
    for drive_letter in drives {
        let drive_path = format!("{}:\\", drive_letter);
        
        if !std::path::Path::new(&drive_path).exists() {
            continue;
        }
        
        println!("Drive {}:", drive_letter);
        
        // Get database path
        let db_path = format!(
            "{}\\iLauncher\\mft_databases\\{}.db",
            std::env::var("LOCALAPPDATA").unwrap(),
            drive_letter
        );
        
        // Use tauri CLI to trigger scan
        let scan_start = Instant::now();
        
        // Scan using cargo run (invoke mft scanner)
        let output = Command::new("cargo")
            .args(&["run", "--release", "--bin", "ilauncher"])
            .env("SCAN_DRIVE", drive_letter.to_string())
            .output()
            .expect("Failed to run scanner");
        
        let scan_duration = scan_start.elapsed();
        
        // Check database file
        if let Ok(metadata) = std::fs::metadata(&db_path) {
            let size_mb = metadata.len() as f64 / 1024.0 / 1024.0;
            println!("   Total time: {:.2?}", scan_duration);
            println!("   Database size: {:.2} MB", size_mb);
            println!();
        } else {
            println!("   Database not found");
            println!();
        }
    }
}
'@

Set-Content -Path "examples\test_mft_scan_perf.rs" -Value $testCode -Encoding UTF8
Write-Host "   Test program created" -ForegroundColor Green
Write-Host ""

# 5. Run test
Write-Host "Running scan test..." -ForegroundColor Cyan
Write-Host ""
$testStart = Get-Date
cargo run --release --example test_mft_scan_perf
$testEnd = Get-Date
$testDuration = ($testEnd - $testStart).TotalSeconds
Write-Host ""
Write-Host "Test completed in $([math]::Round($testDuration, 2)) seconds" -ForegroundColor Green
Write-Host ""

# 6. Database size analysis
Write-Host "Database Size:" -ForegroundColor Cyan
Get-Item "$env:LOCALAPPDATA\iLauncher\mft_databases\*.db" | 
    Select-Object Name, LastWriteTime, @{Name="MB";Expression={[math]::Round($_.Length/1MB,2)}} | 
    Format-Table -AutoSize
Write-Host ""

# 7. Performance recommendations
Write-Host "Performance Optimization Suggestions:" -ForegroundColor Yellow
Write-Host "   1. MFT scan time depends on:" -ForegroundColor White
Write-Host "      - File count" -ForegroundColor Gray
Write-Host "      - Disk I/O speed (SSD vs HDD)" -ForegroundColor Gray
Write-Host "      - USN Journal query efficiency" -ForegroundColor Gray
Write-Host ""
Write-Host "   2. Database write optimizations:" -ForegroundColor White
Write-Host "      - Batch transaction (optimized)" -ForegroundColor Green
Write-Host "      - Disabled journal_mode (optimized)" -ForegroundColor Green
Write-Host "      - FTS5 virtual table (optimized)" -ForegroundColor Green
Write-Host ""
Write-Host "   3. Further optimizations:" -ForegroundColor White
Write-Host "      - Parallel scanning multiple drives" -ForegroundColor Cyan
Write-Host "      - Memory-mapped files (mmap)" -ForegroundColor Cyan
Write-Host "      - Optimize FTS5 tokenizer config" -ForegroundColor Cyan
Write-Host ""

Set-Location $PSScriptRoot
