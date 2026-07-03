# Simulated input helper (SendInput-based).
# Usage:
#   input.ps1 -Click 100,200            (left click)
#   input.ps1 -DoubleClick 100,200
#   input.ps1 -Drag 100,200,300,400     (left-button drag)
#   input.ps1 -Text "hello"             (type text via SendKeys)
#   input.ps1 -Keys "^+e"               (SendKeys syntax: ctrl+shift+e)
#   input.ps1 -Wheel 120 -At 500,400    (mouse wheel at position)
param(
    [string]$Click,
    [string]$DoubleClick,
    [string]$Drag,
    [string]$Text,
    [string]$Keys,
    [int]$Wheel = 0,
    [string]$At
)

# When invoked with `powershell -File`, "4014,58" arrives as one string and a
# naive [int] cast would treat the comma as a thousands separator. Split it.
function Parse-Coords([string]$s) {
    if (-not $s) { return $null }
    return @($s -split ',' | ForEach-Object { [int]($_.Trim()) })
}
$Click = Parse-Coords $Click
$DoubleClick = Parse-Coords $DoubleClick
$Drag = Parse-Coords $Drag
$At = Parse-Coords $At

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class SendNative {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public MOUSEINPUT mi; }
    public static string UnderCursor() {
        POINT p; GetCursorPos(out p);
        IntPtr h = WindowFromPoint(p);
        var sb = new System.Text.StringBuilder(256);
        GetWindowText(h, sb, 256);
        IntPtr root = GetAncestor(h, 2); // GA_ROOT
        var sb2 = new System.Text.StringBuilder(256);
        GetWindowText(root, sb2, 256);
        return "(" + p.X + "," + p.Y + ") '" + sb.ToString() + "' root '" + sb2.ToString() + "'";
    }
    static void Send(uint flags, uint data) {
        INPUT[] inputs = new INPUT[1];
        inputs[0].type = 0;
        inputs[0].mi.dwFlags = flags;
        inputs[0].mi.mouseData = data;
        SendInput(1, inputs, Marshal.SizeOf(typeof(INPUT)));
    }
    public static void Down() { Send(0x0002, 0); }
    public static void Up() { Send(0x0004, 0); }
    public static void WheelBy(int amount) { Send(0x0800, (uint)amount); }
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
    // Atomic click: absolute move + down + up in ONE SendInput batch, so a
    // concurrent real mouse (user's hand on the device) cannot interleave
    // and displace the click.
    public static void ClickAt(int x, int y) {
        int vx = GetSystemMetrics(76), vy = GetSystemMetrics(77);   // virtual screen origin
        int vw = GetSystemMetrics(78), vh = GetSystemMetrics(79);   // virtual screen size
        INPUT[] inputs = new INPUT[3];
        inputs[0].type = 0;
        inputs[0].mi.dwFlags = 0x0001 | 0x8000 | 0x4000; // MOVE | ABSOLUTE | VIRTUALDESK
        inputs[0].mi.dx = (int)Math.Round((x - vx) * 65535.0 / (vw - 1));
        inputs[0].mi.dy = (int)Math.Round((y - vy) * 65535.0 / (vh - 1));
        inputs[1].type = 0; inputs[1].mi.dwFlags = 0x0002; // LEFTDOWN
        inputs[2].type = 0; inputs[2].mi.dwFlags = 0x0004; // LEFTUP
        SendInput(3, inputs, Marshal.SizeOf(typeof(INPUT)));
    }
}
'@
[SendNative]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
Add-Type -AssemblyName System.Windows.Forms

function Move-To([int]$x, [int]$y) {
    for ($i = 0; $i -lt 5; $i++) {
        $ok = [SendNative]::SetCursorPos($x, $y)
        Start-Sleep -Milliseconds 120
        $p = New-Object SendNative+POINT
        [SendNative]::GetCursorPos([ref]$p) | Out-Null
        if ($p.X -eq $x -and $p.Y -eq $y) { return }
        Write-Output "move retry $i : SetCursorPos=$ok cursor at $($p.X),$($p.Y) wanted $x,$y"
    }
}

function LClick([int]$x, [int]$y) {
    # Atomic move+down+up in one SendInput batch. A stray absolute pointing
    # device on this machine keeps warping the cursor to ~(52,48), so anything
    # non-atomic loses the race.
    [SendNative]::ClickAt($x, $y)
    Start-Sleep -Milliseconds 150
}

if ($Click) { LClick $Click[0] $Click[1]; Write-Output "clicked $($Click -join ',')" }

if ($DoubleClick) {
    Move-To $DoubleClick[0] $DoubleClick[1]
    [SendNative]::Down(); Start-Sleep -Milliseconds 40; [SendNative]::Up()
    Start-Sleep -Milliseconds 90
    [SendNative]::Down(); Start-Sleep -Milliseconds 40; [SendNative]::Up()
    Write-Output "double-clicked $($DoubleClick -join ',')"
}

if ($Drag) {
    Move-To $Drag[0] $Drag[1]
    Start-Sleep -Milliseconds 100
    [SendNative]::Down()
    Start-Sleep -Milliseconds 120
    $steps = 14
    for ($i = 1; $i -le $steps; $i++) {
        $x = $Drag[0] + ($Drag[2] - $Drag[0]) * $i / $steps
        $y = $Drag[1] + ($Drag[3] - $Drag[1]) * $i / $steps
        [SendNative]::SetCursorPos([int]$x, [int]$y) | Out-Null
        Start-Sleep -Milliseconds 30
    }
    Start-Sleep -Milliseconds 120
    [SendNative]::Up()
    Write-Output "dragged $($Drag -join ',')"
}

if ($Wheel -ne 0) {
    if ($At) { Move-To $At[0] $At[1] }
    [SendNative]::WheelBy($Wheel)
    Write-Output "wheel $Wheel"
}

if ($Text) {
    Start-Sleep -Milliseconds 150
    [System.Windows.Forms.SendKeys]::SendWait($Text)
    Write-Output "typed"
}

if ($Keys) {
    Start-Sleep -Milliseconds 150
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Write-Output "sent keys"
}
