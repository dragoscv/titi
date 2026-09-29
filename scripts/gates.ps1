<#
.SYNOPSIS
  Run every quality gate with per-gate timing. Two lanes run in parallel:
    js     : turbo typecheck + lint + test (cached, parallel across packages)
    native : clippy -> rust tests -> gradle compile+lint (serial: cargo shares core/target's lock)
    desktop: Tauri crate clippy -D warnings + unit tests (own target dir, so it runs in parallel)
.EXAMPLE
  pwsh -NoProfile -File scripts/gates.ps1            # everything
  pwsh -NoProfile -File scripts/gates.ps1 -Only js   # one lane
  pwsh -NoProfile -File scripts/gates.ps1 -Build     # also production builds
#>
param(
  [ValidateSet('all', 'js', 'native', 'desktop')] [string] $Only = 'all',
  [switch] $Build
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$logDir = Join-Path $root ".copilot-tmp/gates/$stamp"
New-Item -ItemType Directory -Force $logDir | Out-Null
if (-not $env:JAVA_HOME -and (Test-Path 'C:\Program Files\Java\jdk-22')) { $env:JAVA_HOME = 'C:\Program Files\Java\jdk-22' }

$nextest = [bool](Get-Command cargo-nextest -ErrorAction SilentlyContinue)
$rustTest = if ($nextest) { 'cargo nextest run -p titi-core --features opus,json' } else { 'cargo test -p titi-core --features opus,json' }
$gradle = if ($IsWindows -or $env:OS -eq 'Windows_NT') { '.\gradlew.bat' } else { './gradlew' }
$gradleTasks = @(':app:compileGmsDebugKotlin', ':app:lintGmsDebug', ':wear:compileDebugKotlin', ':wear:lintDebug',
  ':app:compileFossDebugKotlin', ':app:checkFossNoGms', ':core-ffi:testDebugUnitTest', ':app:testGmsDebugUnitTest',
  ':tv:compileDebugKotlin', ':tv:lintDebug')
if ($Build) { $gradleTasks += @(':app:assembleGmsDebug', ':wear:assembleDebug', ':tv:assembleDebug') }

$lanes = [ordered]@{
  js     = @(
    @{ Name = 'turbo-check'; Dir = '.'; Cmd = 'pnpm turbo run typecheck lint test --output-logs=errors-only' }
  )
  desktop = @(
    @{ Name = 'desktop-rust'; Dir = '.'; Cmd = 'pwsh -NoProfile -File scripts/desktop-check.ps1' }
  )
  native = @(
    @{ Name = 'clippy';    Dir = 'core';    Cmd = 'cargo clippy --workspace --all-targets --features titi-ffi/cli -- -D warnings' }
    @{ Name = 'rust-test'; Dir = 'core';    Cmd = $rustTest }
    @{ Name = 'gradle';    Dir = 'android'; Cmd = "$gradle $($gradleTasks -join ' ') --console=plain" }
  )
}
if ($Build) {
  # the gate already type-checked; next.config.ts skips its duplicate check when this is set
  $env:TITI_TYPECHECKED = '1'
  $lanes.js += @{ Name = 'turbo-build'; Dir = '.'; Cmd = 'pnpm turbo run build --output-logs=errors-only' }
}
$selected = if ($Only -eq 'all') { @($lanes.Keys) } else { @($Only) }

$total = [Diagnostics.Stopwatch]::StartNew()
$jobs = foreach ($lane in $selected) {
  Start-ThreadJob -Name $lane -ArgumentList $root, $logDir, $lanes[$lane] -ScriptBlock {
    param($root, $logDir, $gates)
    foreach ($g in $gates) {
      $log = Join-Path $logDir "$($g.Name).log"
      $sw = [Diagnostics.Stopwatch]::StartNew()
      & pwsh -NoProfile -WorkingDirectory (Join-Path $root $g.Dir) -Command $g.Cmd *> $log
      $code = $LASTEXITCODE
      $sw.Stop()
      [pscustomobject]@{ Gate = $g.Name; Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1); Exit = $code; Log = $log }
      if ($code -ne 0) { break } # later gates in a lane depend on earlier ones
    }
  }
}
$results = $jobs | Receive-Job -Wait -AutoRemoveJob
$total.Stop()
$results | Sort-Object Seconds -Descending | Format-Table Gate, Seconds, Exit, Log -AutoSize | Out-String | Write-Host
'wall: {0:N1}s  logs: {1}' -f $total.Elapsed.TotalSeconds, $logDir | Write-Host
$failed = @($results | Where-Object { $_.Exit -ne 0 })
if ($failed.Count -gt 0) { Write-Host ('FAILED: ' + ($failed.Gate -join ', ')) -ForegroundColor Red; exit 1 }
Write-Host 'all gates passed' -ForegroundColor Green
