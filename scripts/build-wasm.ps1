<#
.SYNOPSIS
  Build titi-wasm and emit wasm-bindgen JS/TS into packages/core-wasm/pkg (bundler target for Next.js).
#>
param([switch] $Debug)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location "$root\core"
try {
  $profile = if ($Debug) { 'debug' } else { 'release' }
  $args = @('build', '-p', 'titi-wasm', '--target', 'wasm32-unknown-unknown')
  if (-not $Debug) { $args += '--release' }
  cargo @args
  if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
  $out = "$root\packages\core-wasm\pkg"
  New-Item -ItemType Directory -Force $out | Out-Null
  wasm-bindgen "target\wasm32-unknown-unknown\$profile\titi_wasm.wasm" --out-dir $out --target bundler --typescript
  if ($LASTEXITCODE -ne 0) { throw 'wasm-bindgen failed' }
  $opt = Get-Command wasm-opt -ErrorAction SilentlyContinue
  if ($opt -and -not $Debug) { wasm-opt -Oz "$out\titi_wasm_bg.wasm" -o "$out\titi_wasm_bg.wasm" }
  Get-Item "$out\titi_wasm_bg.wasm" | Select-Object Name, @{n='KB';e={[math]::Round($_.Length/1KB)}}
} finally { Pop-Location }
