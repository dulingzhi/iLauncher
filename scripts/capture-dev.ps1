param(
    [Parameter(Mandatory = $true)][string]$Which,
    [Parameter(Mandatory = $true)][string]$OutPng
)

# 副窗口截图：ILAUNCHER_DEV_OPEN 唤起窗口后，用 ShowWindow 强制显示（绕过 WS_VISIBLE=false 的既有问题），再截屏
$ErrorActionPreference = 'Continue'
$exe = 'D:\Projects\iLauncher\target\debug\ilauncher-gpui.exe'

$env:ILAUNCHER_NO_AUTOHIDE = '1'
$env:ILAUNCHER_DEV_OPEN = $Which
$proc = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 8

Add-Type -AssemblyName System.Drawing
Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetProcessDPIAware();' -Name Dpi -Namespace Win32
[void][Win32.Dpi]::SetProcessDPIAware()
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class EnumDev {
    [DllImport("user32.dll")] private static extern bool EnumWindows(Callback cb, IntPtr l);
    private delegate bool Callback(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern long GetWindowLong(IntPtr h, int idx);
    [StructLayout(LayoutKind.Sequential)] public struct R { public int L; public int T; public int Rt; public int B; }
    public static IntPtr FindByTitle(uint pid, string exclude) {
        // 收集全部 Zed::Window；副窗口与主窗口同尺寸且叠放，靠后（z-order 底）的是后开的副窗口。
        // 返回前先把其余窗口隐藏，避免遮挡目标窗口。
        var wins = new System.Collections.Generic.List<IntPtr>();
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == pid) {
                var cls = new System.Text.StringBuilder(256);
                GetClassName(h, cls, 256);
                if (cls.ToString() == "Zed::Window") wins.Add(h);
            }
            return true;
        }, IntPtr.Zero);
        if (wins.Count == 0) return IntPtr.Zero;
        var target = wins[wins.Count - 1];
        foreach (var w in wins) {
            if (w != target) ShowWindow(w, 0);  // SW_HIDE
        }
        ShowWindow(target, 5);                  // SW_SHOW
        SetForegroundWindow(target);
        return target;
    }
}
"@

$hwnd = [EnumDev]::FindByTitle([uint32]$proc.Id, 'iLauncher')
if ($hwnd -eq [IntPtr]::Zero) {
    Write-Output 'window-not-found'
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}
[void][EnumDev]::ShowWindow($hwnd, 5)  # SW_SHOW
Start-Sleep -Milliseconds 800
[void][EnumDev]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 500

$r = New-Object EnumDev+R
[void][EnumDev]::GetWindowRect($hwnd, [ref]$r)
$w = $r.Rt - $r.L; $h = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
$bmp.Save($OutPng, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500
Write-Output "captured: $OutPng ($w x $h)"
