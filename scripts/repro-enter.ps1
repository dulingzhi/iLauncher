param(
    [Parameter(Mandatory = $true)][string]$Query,
    [string]$Exe = 'D:\Projects\iLauncher\target\release\ilauncher-gpui.exe'
)
$out = 'D:\Projects\iLauncher\scripts\repro-out.log'
$err = 'D:\Projects\iLauncher\scripts\repro-err.log'
Remove-Item $out, $err -ErrorAction SilentlyContinue
$env:ILAUNCHER_DEV_QUERY = $Query
$env:RUST_BACKTRACE = 'full'
$proc = Start-Process -FilePath $Exe -RedirectStandardOutput $out -RedirectStandardError $err -PassThru
Start-Sleep -Seconds 3
$wshell = New-Object -ComObject WScript.Shell
$activated = $wshell.AppActivate($proc.Id)
Write-Output "activated: $activated"
Start-Sleep -Milliseconds 300
$wshell.SendKeys('{ENTER}')
Start-Sleep -Seconds 3
$exited = $proc.HasExited
Write-Output "process-exited-after-enter: $exited"
if (-not $exited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
Write-Output '--- stdout ---'
if (Test-Path $out) { Get-Content $out -Tail 30 }
Write-Output '--- stderr ---'
if (Test-Path $err) { Get-Content $err -Tail 30 }
exit 0
