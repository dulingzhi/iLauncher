$exe = 'D:\Projects\iLauncher\target\debug\ilauncher-gpui.exe'
$out = 'D:\Projects\iLauncher\docs\skins\main-full.png'
$env:ILAUNCHER_NO_AUTOHIDE = '1'
$env:ILAUNCHER_DEV_QUERY = 'ihud.exe'
$env:ILAUNCHER_SNAPSHOT = 'C:\Users\81468\AppData\Local\iLauncher\mft_databases\D.snapshot'
$proc = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 4
$wshell = New-Object -ComObject WScript.Shell
$wshell.AppActivate($proc.Id) | Out-Null
Start-Sleep -Milliseconds 500

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetProcessDPIAware();' -Name Dpi -Namespace Win32
[void][Win32.Dpi]::SetProcessDPIAware()
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32Rect {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
$r = New-Object Win32Rect+RECT
[void][Win32Rect]::GetWindowRect($proc.MainWindowHandle, [ref]$r)
$w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
Write-Output "main window: ${w}x${h} at $($r.Left),$($r.Top)"
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "captured: $out"
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
exit 0
