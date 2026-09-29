# Capture one top-level window (by title substring) to a PNG. Usage: win-shot.ps1 -Title Titi -Out .copilot-tmp/x.png
param([string] $Title = 'Titi', [string] $Out = '.copilot-tmp/win.png')
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  public struct RECT { public int L, T, R, B; }
}
'@
$p = Get-Process | Where-Object { $_.MainWindowTitle -eq $Title -or ($_.MainWindowTitle -like "*$Title*" -and $_.ProcessName -eq 'titi') } | Select-Object -First 1
if (-not $p) { throw "no window '$Title'" }
$h = $p.MainWindowHandle
[W]::ShowWindow($h, 9) | Out-Null
[W]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 400
$r = New-Object W+RECT; [W]::GetWindowRect($h, [ref]$r) | Out-Null
$bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc(); [W]::PrintWindow($h, $dc, 2) | Out-Null; $g.ReleaseHdc($dc)
$bmp.Save((Join-Path (Get-Location) $Out)); $g.Dispose(); $bmp.Dispose()
"saved $Out ($($r.R - $r.L)x$($r.B - $r.T))"
