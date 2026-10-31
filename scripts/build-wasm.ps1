<#
.SYNOPSIS
  Build titi-wasm and emit wasm-bindgen JS/TS into packages/core-wasm/pkg (bundler target for
  Next.js) and packages/core-wasm/pkg-node (CommonJS nodejs target for the relay's demo bot).
.PARAMETER Target
  all (default) | web | node — which outputs to regenerate.
#>
param([switch] $Debug, [ValidateSet('all', 'web', 'node')] [string] $Target = 'all')
$ErrorActionPreference = 'Stop'
# cargo writes warnings to stderr; under redirection (run-build.ps1 logs) that
# must not become a terminating error — exit codes are checked explicitly.
$PSNativeCommandUseErrorActionPreference = $false
$root = Split-Path -Parent $PSScriptRoot
Push-Location "$root\core"
try {
  $profile = if ($Debug) { 'debug' } else { 'release' }
  $args = @('build', '-p', 'titi-wasm', '--target', 'wasm32-unknown-unknown')
  if (-not $Debug) { $args += '--release' }
  $ErrorActionPreference = 'Continue'
  cargo @args 2>&1 | ForEach-Object { "$_" }
  $ErrorActionPreference = 'Stop'
  if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
  $wasm = "target\wasm32-unknown-unknown\$profile\titi_wasm.wasm"
  $opt = Get-Command wasm-opt -ErrorAction SilentlyContinue
  $outs = @()
  if ($Target -in 'all', 'web') { $outs += @{ dir = "$root\packages\core-wasm\pkg"; target = 'bundler' } }
  if ($Target -in 'all', 'node') { $outs += @{ dir = "$root\packages\core-wasm\pkg-node"; target = 'nodejs' } }
  foreach ($o in $outs) {
    New-Item -ItemType Directory -Force $o.dir | Out-Null
    wasm-bindgen $wasm --out-dir $o.dir --target $o.target --typescript
    if ($LASTEXITCODE -ne 0) { throw "wasm-bindgen ($($o.target)) failed" }
    if ($opt -and -not $Debug) { wasm-opt -Oz "$($o.dir)\titi_wasm_bg.wasm" -o "$($o.dir)\titi_wasm_bg.wasm" }
    if ($o.target -eq 'nodejs') {
      # the nodejs target is CommonJS while @titi/core-wasm is "type": "module"
      Set-Content -NoNewline -Encoding ascii "$($o.dir)\package.json" "{ `"type`": `"commonjs`" }`n"
    }
    Get-Item "$($o.dir)\titi_wasm_bg.wasm" | Select-Object @{n='Out';e={$o.target}}, @{n='KB';e={[math]::Round($_.Length/1KB)}}
  }
} finally { Pop-Location }
