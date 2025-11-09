# Kill all iLauncher processes (requires admin)
Write-Host "Killing all iLauncher processes..." -ForegroundColor Yellow
taskkill /F /IM ilauncher.exe 2>$null
Start-Sleep -Seconds 1

Write-Host "`n✓ All processes terminated" -ForegroundColor Green

# Rebuild
Write-Host "`n📦 Building Release version..." -ForegroundColor Cyan
Set-Location "src-tauri"
cargo build --release

Write-Host "`n✅ Build complete! Run ./src-tauri/target/release/ilauncher.exe to test" -ForegroundColor Green
