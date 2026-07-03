param([Parameter(Mandatory = $true)][long]$Hwnd)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class RectNative {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
'@
[RectNative]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
$r = New-Object RectNative+RECT
[RectNative]::GetWindowRect([IntPtr]$Hwnd, [ref]$r) | Out-Null
$c = New-Object RectNative+RECT
[RectNative]::GetClientRect([IntPtr]$Hwnd, [ref]$c) | Out-Null
$p = New-Object RectNative+POINT
[RectNative]::ClientToScreen([IntPtr]$Hwnd, [ref]$p) | Out-Null
Write-Output "window: $($r.Left) $($r.Top) $($r.Right) $($r.Bottom)"
Write-Output "client-origin: $($p.X) $($p.Y) client-size: $($c.Right) $($c.Bottom)"
