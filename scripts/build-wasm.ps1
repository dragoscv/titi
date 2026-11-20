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
  # The wasm-bindgen CLI must match the crate in Cargo.lock exactly, and other repos on the same
  # machine lock other versions, so a global CLI drifts. Use a per-version install instead.
  $lockLine = Select-String -Path "$root\core\Cargo.lock" -Pattern '^name = "wasm-bindgen"$' -Context 0, 1 | Select-Object -First 1
  $bgVersion = ($lockLine.Context.PostContext[0] -replace '^version = "(.+)"$', '$1')
  $bindgen = 'wasm-bindgen'
  $globalVersion = if (Get-Command wasm-bindgen -ErrorAction SilentlyContinue) { (& wasm-bindgen --version 2>$null) -replace '^wasm-bindgen\s+', '' } else { '' }
  if ($globalVersion -ne $bgVersion) {
    $bgRoot = Join-Path $env:USERPROFILE ".cargo\wasm-bindgen\$bgVersion"
    $bindgen = Join-Path $bgRoot 'bin\wasm-bindgen.exe'
    if (-not (Test-Path $bindgen)) { $bindgen = Join-Path $bgRoot 'bin\wasm-bindgen' }
    if (-not (Test-Path $bindgen)) {
      Write-Host "installing wasm-bindgen-cli $bgVersion into $bgRoot (global is '$globalVersion')"
      $ErrorActionPreference = 'Continue'
      cargo install wasm-bindgen-cli --version $bgVersion --locked --root $bgRoot 2>&1 | ForEach-Object { "$_" }
      $ErrorActionPreference = 'Stop'
      if ($LASTEXITCODE -ne 0) { throw "cargo install wasm-bindgen-cli $bgVersion failed" }
      $bindgen = Join-Path $bgRoot 'bin\wasm-bindgen.exe'
      if (-not (Test-Path $bindgen)) { $bindgen = Join-Path $bgRoot 'bin\wasm-bindgen' }
    }
  }
  $opt = Get-Command wasm-opt -ErrorAction SilentlyContinue
  $outs = @()
  if ($Target -in 'all', 'web') { $outs += @{ dir = "$root\packages\core-wasm\pkg"; target = 'bundler' } }
  if ($Target -in 'all', 'node') { $outs += @{ dir = "$root\packages\core-wasm\pkg-node"; target = 'nodejs' } }
  foreach ($o in $outs) {
    New-Item -ItemType Directory -Force $o.dir | Out-Null
    & $bindgen $wasm --out-dir $o.dir --target $o.target --typescript
    if ($LASTEXITCODE -ne 0) { throw "wasm-bindgen ($($o.target)) failed" }
    if ($opt -and -not $Debug) { wasm-opt -Oz "$($o.dir)\titi_wasm_bg.wasm" -o "$($o.dir)\titi_wasm_bg.wasm" }
    if ($o.target -eq 'nodejs') {
      # the nodejs target is CommonJS while @titi/core-wasm is "type": "module"
      Set-Content -NoNewline -Encoding ascii "$($o.dir)\package.json" "{ `"type`": `"commonjs`" }`n"
    }
    Get-Item "$($o.dir)\titi_wasm_bg.wasm" | Select-Object @{n='Out';e={$o.target}}, @{n='KB';e={[math]::Round($_.Length/1KB)}}
  }
} finally { Pop-Location }
