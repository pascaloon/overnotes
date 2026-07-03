# Puts a generated test image (or a PNG file) on the clipboard.
# Usage: clipimg.ps1 [-Path file.png]
param([string]$Path)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

if ($Path) {
    $img = [System.Drawing.Image]::FromFile($Path)
} else {
    $img = New-Object System.Drawing.Bitmap 320, 200
    $g = [System.Drawing.Graphics]::FromImage($img)
    $g.Clear([System.Drawing.Color]::FromArgb(255, 40, 90, 160))
    $g.FillEllipse([System.Drawing.Brushes]::Orange, 40, 30, 140, 140)
    $g.FillRectangle([System.Drawing.Brushes]::LimeGreen, 200, 60, 90, 90)
    $g.DrawString("CLIP", (New-Object System.Drawing.Font "Arial", 24, [System.Drawing.FontStyle]::Bold), [System.Drawing.Brushes]::White, 100, 80)
    $g.Dispose()
}
[System.Windows.Forms.Clipboard]::SetImage($img)
Write-Output "clipboard image set ($($img.Width)x$($img.Height))"
