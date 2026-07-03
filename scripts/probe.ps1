param([int]$X = 2100, [int]$Y = 600)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class ProbeNative {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public MOUSEINPUT mi; }
    public static void Click() {
        INPUT[] inputs = new INPUT[2];
        inputs[0].type = 0; inputs[0].mi.dwFlags = 0x0002; // LEFTDOWN
        inputs[1].type = 0; inputs[1].mi.dwFlags = 0x0004; // LEFTUP
        uint sent = SendInput(2, inputs, Marshal.SizeOf(typeof(INPUT)));
        Console.WriteLine("SendInput sent: " + sent + " err: " + Marshal.GetLastWin32Error());
    }
}
'@
[ProbeNative]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
[ProbeNative]::SetCursorPos($X, $Y) | Out-Null
Start-Sleep -Milliseconds 150
$p = New-Object ProbeNative+POINT
[ProbeNative]::GetCursorPos([ref]$p) | Out-Null
Write-Output "cursor at: $($p.X),$($p.Y) (wanted $X,$Y)"
$h = [ProbeNative]::WindowFromPoint($p)
$sb = New-Object System.Text.StringBuilder 256
[ProbeNative]::GetWindowText($h, $sb, 256) | Out-Null
Write-Output "window under cursor: $($sb.ToString())"
[ProbeNative]::Click()
Start-Sleep -Milliseconds 300
$fg = [ProbeNative]::GetForegroundWindow()
$sb2 = New-Object System.Text.StringBuilder 256
[ProbeNative]::GetWindowText($fg, $sb2, 256) | Out-Null
Write-Output "foreground after click: $($sb2.ToString())"
