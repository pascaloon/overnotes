# Integrated overlay test: force the game foreground (AttachThreadInput trick),
# verify the overlay window becomes visible, then optionally click and report.
# Usage: overlaytest.ps1 -Game 1181280 -Overlay 4786530 [-ClickX 4051 -ClickY 488]
param(
    [Parameter(Mandatory = $true)][long]$Game,
    [Parameter(Mandatory = $true)][long]$Overlay,
    # Semicolon-separated list of clicks: "x1,y1;x2,y2;..."
    [string]$Clicks = "",
    [int]$DelayMs = 350
)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class OtNative {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr pid);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public MOUSEINPUT mi; }
    public static bool ForceForeground(IntPtr target) {
        IntPtr fg = GetForegroundWindow();
        uint fgTid = GetWindowThreadProcessId(fg, IntPtr.Zero);
        uint targetTid = GetWindowThreadProcessId(target, IntPtr.Zero);
        uint myTid = GetCurrentThreadId();
        AttachThreadInput(myTid, fgTid, true);
        AttachThreadInput(myTid, targetTid, true);
        BringWindowToTop(target);
        bool ok = SetForegroundWindow(target);
        AttachThreadInput(myTid, fgTid, false);
        AttachThreadInput(myTid, targetTid, false);
        return ok;
    }
    public static void ClickAt(int x, int y) {
        int vx = GetSystemMetrics(76), vy = GetSystemMetrics(77);
        int vw = GetSystemMetrics(78), vh = GetSystemMetrics(79);
        INPUT[] inputs = new INPUT[3];
        inputs[0].type = 0;
        inputs[0].mi.dwFlags = 0x0001 | 0x8000 | 0x4000;
        inputs[0].mi.dx = (int)Math.Round((x - vx) * 65535.0 / (vw - 1));
        inputs[0].mi.dy = (int)Math.Round((y - vy) * 65535.0 / (vh - 1));
        inputs[1].type = 0; inputs[1].mi.dwFlags = 0x0002;
        inputs[2].type = 0; inputs[2].mi.dwFlags = 0x0004;
        SendInput(3, inputs, Marshal.SizeOf(typeof(INPUT)));
    }
}
'@
[OtNative]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

$ok = [OtNative]::ForceForeground([IntPtr]$Game)
Start-Sleep -Milliseconds 400
$fg = [OtNative]::GetForegroundWindow()
Write-Output "force-foreground ok=$ok, foreground now=$fg (game=$Game overlay=$Overlay)"
Write-Output "overlay visible: $([OtNative]::IsWindowVisible([IntPtr]$Overlay))"

if ($Clicks) {
    foreach ($pair in $Clicks -split ';') {
        $xy = $pair -split ','
        [OtNative]::ClickAt([int]$xy[0], [int]$xy[1])
        Start-Sleep -Milliseconds $DelayMs
        $fg2 = [OtNative]::GetForegroundWindow()
        Write-Output "clicked $pair : foreground=$fg2 overlayVisible=$([OtNative]::IsWindowVisible([IntPtr]$Overlay))"
    }
}
