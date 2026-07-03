# ShowWindow wrapper. Usage: showwin.ps1 -Hwnd 1234 -Cmd 6  (6=minimize, 9=restore, 3=maximize)
param(
    [Parameter(Mandatory = $true)][long]$Hwnd,
    [Parameter(Mandatory = $true)][int]$Cmd
)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class ShowNative {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
}
'@
[ShowNative]::ShowWindow([IntPtr]$Hwnd, $Cmd) | Out-Null
Write-Output "ShowWindow($Hwnd, $Cmd) done"
