# Build the Windows installers for apps/desktop (NSIS per-machine; MSIX via -Msix).
param([string] $Bundles = 'nsis', [switch] $Msix)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $root 'apps/desktop')
pnpm exec tauri build --bundles $Bundles
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if ($Msix) {
  & (Join-Path $PSScriptRoot 'desktop-msix.ps1')
  exit $LASTEXITCODE
}
Get-ChildItem (Join-Path $root 'apps/desktop/src-tauri/target/release/bundle') -Recurse -Include *.exe, *.msi -ErrorAction SilentlyContinue |
  Select-Object FullName, Length, LastWriteTime | Format-Table -AutoSize | Out-String -Width 300
