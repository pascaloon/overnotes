param([Parameter(Mandatory = $true)][long]$Hwnd)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class PidNative {
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
'@
$p = [uint32]0
[PidNative]::GetWindowThreadProcessId([IntPtr]$Hwnd, [ref]$p) | Out-Null
$proc = Get-Process -Id $p -ErrorAction SilentlyContinue
Write-Output "hwnd $Hwnd -> pid $p ($($proc.ProcessName))"
