$exe = 'D:\Projects\iLauncher\target\debug\ilauncher-gpui.exe'
$out = 'D:\Projects\iLauncher\docs\skins\preview-window.png'
$env:ILAUNCHER_NO_AUTOHIDE = '1'
$env:ILAUNCHER_DEV_QUERY = 'ihud.exe'
$env:ILAUNCHER_SNAPSHOT = 'C:\Users\81468\AppData\Local\iLauncher\mft_databases\D.snapshot'
$proc = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 4
$wshell = New-Object -ComObject WScript.Shell
$wshell.AppActivate($proc.Id) | Out-Null
Start-Sleep -Milliseconds 300
$wshell.SendKeys('%p')   # Alt+P 打开预览窗口
Start-Sleep -Seconds 2

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetProcessDPIAware();' -Name Dpi -Namespace Win32
[void][Win32.Dpi]::SetProcessDPIAware()
$b = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "captured: $out"
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
exit 0
