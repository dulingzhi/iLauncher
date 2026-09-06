$exe = 'D:\Projects\iLauncher\target\debug\ilauncher-gpui.exe'
$env:ILAUNCHER_NO_AUTOHIDE = '1'
$env:ILAUNCHER_DEV_QUERY = 'ihud.exe'
$env:ILAUNCHER_SNAPSHOT = 'C:\Users\81468\AppData\Local\iLauncher\mft_databases\D.snapshot'
# 注意：NO_AUTOHIDE 会关掉失焦自动隐藏，但 Esc 路径与 focus 抢占判断不受影响
$proc = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 4
$wshell = New-Object -ComObject WScript.Shell
$wshell.AppActivate($proc.Id) | Out-Null
Start-Sleep -Milliseconds 500
$mainHwnd = $proc.MainWindowHandle

Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();' -Name U32 -Namespace W32
Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetProcessDPIAware();' -Name Dpi -Namespace Win32
[void][Win32.Dpi]::SetProcessDPIAware()
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

function Capture([string]$name) {
    $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
    $p = "D:\Projects\iLauncher\docs\skins\verify-$name.png"
    $bmp.Save($p, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Output "captured verify-$name.png"
}

$fgBefore = [W32.U32]::GetForegroundWindow()
Write-Output "main hwnd=$mainHwnd fg(before Alt+P)=$fgBefore match=$($fgBefore -eq $mainHwnd)"

# Alt+P 打开预览
$wshell.SendKeys('%p')
Start-Sleep -Seconds 2
$fgAfter = [W32.U32]::GetForegroundWindow()
Write-Output "fg(after Alt+P)=$fgAfter  main-kept-focus=$($fgAfter -eq $mainHwnd)"
Capture 'after-altp'

# Esc 隐藏主窗（NO_AUTOHIDE 下 Esc 仍然销毁窗口）
$wshell.AppActivate($proc.Id) | Out-Null
Start-Sleep -Milliseconds 300
$wshell.SendKeys('{ESC}')
Start-Sleep -Seconds 1
Capture 'after-esc'

# Ctrl+Space 重新唤起
$wshell.SendKeys('^ ')
Start-Sleep -Seconds 2
$mainHwnd2 = $proc.MainWindowHandle
Write-Output "resurrected main hwnd=$mainHwnd2 (0 = 未重建)"
Capture 'after-resummon'

Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
exit 0
