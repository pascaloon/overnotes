# Prints style/exstyle/rect for windows whose title contains a substring.
param([Parameter(Mandatory = $true)][string]$Title)
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class StyleNative {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, [Out] StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern long GetWindowLongPtrW(IntPtr h, int i);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    public static List<IntPtr> hits = new List<IntPtr>();
    public static string needle = "";
    static bool Cb(IntPtr h, IntPtr l) {
        var sb = new StringBuilder(512);
        GetWindowTextW(h, sb, 512);
        if (sb.ToString().IndexOf(needle, StringComparison.OrdinalIgnoreCase) >= 0) hits.Add(h);
        return true;
    }
    public static void Find(string t) { needle = t; hits.Clear(); EnumWindows(Cb, IntPtr.Zero); }
}
'@
[StyleNative]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
[StyleNative]::Find($Title)
foreach ($h in [StyleNative]::hits) {
    $style = [StyleNative]::GetWindowLongPtrW($h, -16)
    $ex = [StyleNative]::GetWindowLongPtrW($h, -20)
    $r = New-Object StyleNative+RECT
    [StyleNative]::GetWindowRect($h, [ref]$r) | Out-Null
    $vis = [StyleNative]::IsWindowVisible($h)
    $flags = @()
    if ($ex -band 0x8)        { $flags += "TOPMOST" }
    if ($ex -band 0x20)       { $flags += "TRANSPARENT" }
    if ($ex -band 0x80)       { $flags += "TOOLWINDOW" }
    if ($ex -band 0x80000)    { $flags += "LAYERED" }
    if ($ex -band 0x8000000)  { $flags += "NOACTIVATE" }
    "hwnd $h vis=$vis rect=($($r.L),$($r.T))-($($r.R),$($r.B)) style=0x{0:X} ex=0x{1:X} [{2}]" -f $style, $ex, ($flags -join ",")
}
