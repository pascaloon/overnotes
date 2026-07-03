param(
    [Parameter(Mandatory = $true)][long]$Hwnd,
    [Parameter(Mandatory = $true)][string]$Out,
    [switch]$NoFocus
)

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class CapNative {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
}
'@
# -4 = DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
[CapNative]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
Add-Type -AssemblyName System.Drawing

if (-not $NoFocus) {
    [CapNative]::SetForegroundWindow([IntPtr]$Hwnd) | Out-Null
    Start-Sleep -Milliseconds 600
}

$r = New-Object CapNative+RECT
[CapNative]::GetWindowRect([IntPtr]$Hwnd, [ref]$r) | Out-Null
$w = $r.Right - $r.Left
$h = $r.Bottom - $r.Top
if ($w -le 0 -or $h -le 0) { Write-Output "invalid rect $($r.Left),$($r.Top),$($r.Right),$($r.Bottom)"; exit 1 }

$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
$bmp.Save($Out)
$g.Dispose()
$bmp.Dispose()
Write-Output "saved $Out ($w x $h)"
