# Package the release titi.exe as MSIX (winapp CLI). -Store = unsigned upload for
# Partner Center; default = signed with a local dev cert (~/.titi/msix-devcert.pfx).
param([switch] $Store)
$ErrorActionPreference = 'Stop'
$env:WINAPP_CLI_TELEMETRY_OPTOUT = '1'
$root = Split-Path -Parent $PSScriptRoot
$tauri = Join-Path $root 'apps/desktop/src-tauri'
$exe = Join-Path $tauri 'target/release/titi.exe'
if (-not (Test-Path $exe)) { throw "build first: $exe missing" }
$stage = Join-Path $tauri 'target/release/msix-stage'
$out = Join-Path $tauri 'target/release/bundle/msix'
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage, $out | Out-Null
Copy-Item $exe $stage
Copy-Item (Join-Path $tauri 'windows/msix/Package.appxmanifest') $stage
Push-Location $stage
try {
  winapp manifest update-assets (Join-Path $tauri 'icons/icon.png') --manifest Package.appxmanifest -q
  if ($LASTEXITCODE -ne 0) { throw 'update-assets failed' }
  # Brand pack AppList (taskbar/Start) assets: overwrite the generated targetsize set
  # with the hand-tuned plated/unplated/light-unplated PNGs, named after the manifest's
  # Square44x44Logo (e.g. Assets\AppList.png -> Assets\AppList.targetsize-48.png).
  [xml] $mf = Get-Content -Raw (Join-Path $stage 'Package.appxmanifest')
  $logo = ($mf.SelectNodes('//*[@Square44x44Logo]') | Select-Object -First 1).Square44x44Logo
  if (-not $logo) { throw 'Square44x44Logo not found in manifest' }
  $assetDir = Join-Path $stage (Split-Path -Parent $logo)
  $base = [IO.Path]::GetFileNameWithoutExtension($logo)
  New-Item -ItemType Directory -Force $assetDir | Out-Null
  $brand = Get-ChildItem (Join-Path $tauri 'icons/msix') -Filter 'AppList.targetsize-*.png'
  if (-not $brand) { throw 'brand AppList assets missing: run node brand/scripts/build.mjs' }
  foreach ($f in $brand) {
    $name = $base + $f.Name.Substring('AppList'.Length)
    Copy-Item -Force $f.FullName (Join-Path $assetDir $name)
  }
  $msix = Join-Path $out 'Titi_x64.msix'
  if ($Store) {
    winapp package $stage --manifest Package.appxmanifest --output $msix --no-sign
  } else {
    $cert = Join-Path $HOME '.titi/msix-devcert.pfx'
    if (-not (Test-Path $cert)) {
      New-Item -ItemType Directory -Force (Split-Path $cert) | Out-Null
      winapp cert generate --manifest Package.appxmanifest --output $cert -q
      if ($LASTEXITCODE -ne 0) { throw 'cert generate failed' }
    }
    winapp package $stage --manifest Package.appxmanifest --output $msix --cert $cert
  }
  if ($LASTEXITCODE -ne 0) { throw 'package failed' }
  Get-Item $msix | Select-Object FullName, Length | Format-List | Out-String
} finally { Pop-Location }
