param(
    [Parameter(Mandatory = $true)][long]$Hwnd,
    [int]$X = 100,
    [int]$Y = 100,
    [int]$Width = 0,
    [int]$Height = 0,
    [switch]$Topmost,
    [switch]$NoTopmost
)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class MoveNative {
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
}
'@
[MoveNative]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
# SWP_NOSIZE(1) | SWP_NOACTIVATE not needed; insertAfter: HWND_TOPMOST(-1) / HWND_NOTOPMOST(-2)
$after = [IntPtr]::Zero
$flags = [uint32]5  # NOSIZE | NOZORDER
if ($Topmost) { $after = [IntPtr](-1); $flags = [uint32]1 }
if ($NoTopmost) { $after = [IntPtr](-2); $flags = [uint32]1 }
if ($Width -gt 0 -and $Height -gt 0) { $flags = $flags -band (-bnot [uint32]1) }
[MoveNative]::SetWindowPos([IntPtr]$Hwnd, $after, $X, $Y, $Width, $Height, $flags) | Out-Null
[MoveNative]::SetForegroundWindow([IntPtr]$Hwnd) | Out-Null
Write-Output "moved to $X,$Y (topmost=$Topmost) and focused"
