# Sends WM_NCHITTEST to a window for a screen point and prints the result.
param(
    [Parameter(Mandatory = $true)][long]$Hwnd,
    [Parameter(Mandatory = $true)][int]$X,
    [Parameter(Mandatory = $true)][int]$Y
)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class HitNative {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern int GetWindowRgn(IntPtr h, IntPtr rgn);
    [DllImport("gdi32.dll")] public static extern IntPtr CreateRectRgn(int a, int b, int c, int d);
    [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr o);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    public static string Under(int x, int y) {
        POINT p; p.X = x; p.Y = y;
        IntPtr h = WindowFromPoint(p);
        var sb = new System.Text.StringBuilder(256);
        GetWindowTextW(h, sb, 256);
        return h + " '" + sb + "'";
    }
}
'@
[HitNative]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
$l = [IntPtr](($Y -shl 16) -bor ($X -band 0xFFFF))
$ht = [HitNative]::SendMessage([IntPtr]$Hwnd, 0x0084, [IntPtr]::Zero, $l)
$rgn = [HitNative]::CreateRectRgn(0,0,0,0)
$hasRgn = [HitNative]::GetWindowRgn([IntPtr]$Hwnd, $rgn)
[HitNative]::DeleteObject($rgn) | Out-Null
Write-Output "WM_NCHITTEST($X,$Y) = $ht (1=HTCLIENT, -1=HTTRANSPARENT, 0=HTNOWHERE); windowRgn=$hasRgn (0=none)"
Write-Output ("WindowFromPoint = " + [HitNative]::Under($X, $Y))
