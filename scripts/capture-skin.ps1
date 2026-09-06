param(
    [Parameter(Mandatory = $true)][string]$Skin,
    [Parameter(Mandatory = $true)][string]$OutPng,
    [string]$Mode = 'bench',
    [string]$Query = '',
    [string]$Snapshot = ''
)

$ErrorActionPreference = 'Continue'
$exe = 'D:\Projects\iLauncher\target\debug\ilauncher-gpui.exe'

# 设置皮肤注册表值（"default" 表示删除该值；default-light 固定浅色）
if ($Skin -eq 'default') {
    & reg.exe delete "HKCU\Software\iLauncher" /v Skin /f 2>$null | Out-Null
    & reg.exe add "HKCU\Software\iLauncher" /v ThemeMode /t REG_SZ /d dark /f | Out-Null
} elseif ($Skin -eq 'default-light') {
    & reg.exe delete "HKCU\Software\iLauncher" /v Skin /f 2>$null | Out-Null
    & reg.exe add "HKCU\Software\iLauncher" /v ThemeMode /t REG_SZ /d light /f | Out-Null
} else {
    & reg.exe add "HKCU\Software\iLauncher" /v Skin /t REG_SZ /d $Skin /f | Out-Null
}

# bench：主窗 5 秒内滚动 10 万条 Demo 数据（列表选中高亮/预览面板可见），5 秒后自动退出
# normal：空查询 → 验证空状态。ILAUNCHER_NO_AUTOHIDE=1 防失焦销毁（自动化截图用）
$env:ILAUNCHER_NO_AUTOHIDE = '1'
if ($Query -ne '') { $env:ILAUNCHER_DEV_QUERY = $Query }
if ($Snapshot -ne '') { $env:ILAUNCHER_SNAPSHOT = $Snapshot }
if ($Mode -eq 'bench') {
    $proc = Start-Process -FilePath $exe -ArgumentList '--bench' -PassThru
} else {
    $proc = Start-Process -FilePath $exe -PassThru
}
Start-Sleep -Seconds 3

Add-Type -AssemblyName System.Drawing
# DPI 感知：否则 GetWindowRect（物理像素）与 CopyFromScreen（逻辑像素）错位
Add-Type '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetProcessDPIAware();' -Name Dpi -Namespace Win32
[void][Win32.Dpi]::SetProcessDPIAware()
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public class EnumWin2 {
    [DllImport("user32.dll")] private static extern bool EnumWindows(Callback cb, IntPtr l);
    private delegate bool Callback(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [StructLayout(LayoutKind.Sequential)] public struct R { public int L; public int T; public int Rt; public int B; }
    public static IntPtr FindLargestVisible(uint pid) {
        IntPtr best = IntPtr.Zero; long bestArea = 0;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == pid && IsWindowVisible(h)) {
                R r; GetWindowRect(h, out r);
                long area = (long)(r.Rt - r.L) * (r.B - r.T);
                if (area > bestArea) { bestArea = area; best = h; }
            }
            return true;
        }, IntPtr.Zero);
        return best;
    }
}
"@

$hwnd = [EnumWin2]::FindLargestVisible([uint32]$proc.Id)
if ($hwnd -eq [IntPtr]::Zero) {
    Write-Output 'window-not-found'
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}

$r = New-Object EnumWin2+R
[void][EnumWin2]::GetWindowRect($hwnd, [ref]$r)
$w = $r.Rt - $r.L; $h = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
$bmp.Save($OutPng, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500
Write-Output "captured: $OutPng ($w x $h)"
