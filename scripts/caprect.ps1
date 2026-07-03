# Captures an arbitrary screen rectangle (physical pixels) to a PNG.
# Usage: caprect.ps1 -X 4000 -Y 150 -Width 1100 -Height 700 -Out shot.png
param(
    [Parameter(Mandatory = $true)][int]$X,
    [Parameter(Mandatory = $true)][int]$Y,
    [Parameter(Mandatory = $true)][int]$Width,
    [Parameter(Mandatory = $true)][int]$Height,
    [Parameter(Mandatory = $true)][string]$Out
)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class CapRectNative {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
}
'@
[CapRectNative]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
Add-Type -AssemblyName System.Drawing

$bmp = New-Object System.Drawing.Bitmap($Width, $Height)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($X, $Y, 0, 0, $bmp.Size)
$bmp.Save($Out)
$g.Dispose()
$bmp.Dispose()
Write-Output "saved $Out ($Width x $Height at $X,$Y)"
