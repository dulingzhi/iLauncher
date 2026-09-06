$exe = 'D:\Projects\iLauncher\target\debug\ilauncher-gpui.exe'
$env:ILAUNCHER_DEV_QUERY = 'ihud.exe'
$env:ILAUNCHER_SNAPSHOT = 'C:\Users\81468\AppData\Local\iLauncher\mft_databases\D.snapshot'
Remove-Item Env:\ILAUNCHER_NO_AUTOHIDE -ErrorAction SilentlyContinue
$proc = Start-Process -FilePath $exe -RedirectStandardOutput 'D:\Projects\iLauncher\docs\skins\dbg-app.log' -RedirectStandardError 'D:\Projects\iLauncher\docs\skins\dbg-app.err.log' -PassThru
Start-Sleep -Seconds 4
$wshell = New-Object -ComObject WScript.Shell
$wshell.AppActivate($proc.Id) | Out-Null
Start-Sleep -Milliseconds 500
$mainHwnd = $proc.MainWindowHandle

Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();' -Name U32 -Namespace W32
Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetProcessDPIAware();' -Name Dpi -Namespace Win32
[void][Win32.Dpi]::SetProcessDPIAware()
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Collections.Generic;
public class EnumWin {
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h, int i);
    delegate bool EnumProc(IntPtr h, IntPtr l);
    public struct RECT { public int Left, Top, Right, Bottom; }
    public static List<IntPtr> TopLevelOf(uint pid) {
        var list = new List<IntPtr>();
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == pid) list.Add(h);
            return true;
        }, IntPtr.Zero);
        return list;
    }
}
"@
Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);' -Name M1 -Namespace W32
Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, System.IntPtr e);' -Name M2 -Namespace W32
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

function Shot([string]$name, $rect) {
    $w = $rect.Right - $rect.Left; $h = $rect.Bottom - $rect.Top
    if ($w -le 0 -or $h -le 0) { Write-Output "$name skip (bad rect)"; return }
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
    $bmp.Save("D:\Projects\iLauncher\docs\skins\dbg-$name.png", [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Output "captured dbg-$name.png"
}

Write-Output ("fg(start)=" + [W32.U32]::GetForegroundWindow() + " main=$mainHwnd")
# Alt+P 打开预览
$wshell.SendKeys('%p')
Start-Sleep -Seconds 2
Write-Output ("fg(after Alt+P)=" + [W32.U32]::GetForegroundWindow())

$wins = [EnumWin]::TopLevelOf([uint32]$proc.Id)
$preview = [IntPtr]::Zero
foreach ($h in $wins) {
    if ($h -ne $mainHwnd -and [EnumWin]::IsWindow($h)) {
        $r = New-Object EnumWin+RECT
        [void][EnumWin]::GetWindowRect($h, [ref]$r)
        $ww = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
        # 预览窗：贴主窗、宽 380 logical(~592 phys)、与主窗同高——按宽高区间精确匹配
        if ($ww -gt 400 -and $ww -lt 800 -and $hh -gt 300 -and $hh -lt 700) {
            $exh = [EnumWin]::GetWindowLongPtrW($h, -20)
            Write-Output ("cand hwnd=$h ${ww}x${hh} ex=0x" + $exh.ToString("X"))
            $preview = $h
        }
    }
}
$ex = [EnumWin]::GetWindowLongPtrW($preview, -20)  # GWL_EXSTYLE
Write-Output ("preview=$preview exstyle=0x" + $ex.ToString("X"))
Write-Output ("noactivate-bit=" + (($ex.ToInt64() -band 0x8000000) -ne 0))
Write-Output ("main-alive-before-click=" + [EnumWin]::IsWindow($mainHwnd))

$pr = New-Object EnumWin+RECT
[void][EnumWin]::GetWindowRect($preview, [ref]$pr)
$cx = [int](($pr.Left + $pr.Right) / 2)
$cy = [int]($pr.Bottom - 48)

[void][W32.M1]::SetCursorPos($cx, $cy)
Start-Sleep -Milliseconds 200
[W32.M2]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 80
[W32.M2]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Seconds 2

Write-Output ("fg(after click)=" + [W32.U32]::GetForegroundWindow())
Write-Output ("main-alive=" + [EnumWin]::IsWindow($mainHwnd))
Write-Output ("preview-alive=" + [EnumWin]::IsWindow($preview))
if ([EnumWin]::IsWindow($preview)) {
    $pr2 = New-Object EnumWin+RECT
    [void][EnumWin]::GetWindowRect($preview, [ref]$pr2)
    Shot 'after-click' $pr2
}
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
exit 0
