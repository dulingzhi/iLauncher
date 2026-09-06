$exe = 'D:\Projects\iLauncher\target\debug\ilauncher-gpui.exe'
$log = 'D:\Projects\iLauncher\docs\skins\verify-app.log'
$env:ILAUNCHER_NO_AUTOHIDE = '1'
$env:ILAUNCHER_DEV_QUERY = 'ihud.exe'
$env:ILAUNCHER_SNAPSHOT = 'C:\Users\81468\AppData\Local\iLauncher\mft_databases\D.snapshot'
if (Test-Path $log) { Remove-Item $log }
$proc = Start-Process -FilePath $exe -RedirectStandardOutput $log -PassThru
Start-Sleep -Seconds 4
$wshell = New-Object -ComObject WScript.Shell
$wshell.AppActivate($proc.Id) | Out-Null
Start-Sleep -Milliseconds 500
# Alt+P 开预览
$wshell.SendKeys('%p')
Start-Sleep -Seconds 1
# Esc 隐藏主窗 + 预览
$wshell.SendKeys('{ESC}')
Start-Sleep -Seconds 1
# 托盘外任意处确保前台不是残留窗（NO_AUTOHIDE 下窗口已销毁）
$wshell.AppActivate($proc.Id) | Out-Null
Start-Sleep -Milliseconds 200
# Ctrl+Space 唤起
$wshell.SendKeys('^ ')
Start-Sleep -Seconds 2
$wshell.SendKeys('^ ')
Start-Sleep -Seconds 2

Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetProcessDPIAware();' -Name Dpi -Namespace Win32
[void][Win32.Dpi]::SetProcessDPIAware()
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$b = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
$bmp.Save('D:\Projects\iLauncher\docs\skins\verify-resummon2.png', [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Write-Output '--- app log tail ---'
Get-Content $log -Tail 15
exit 0
