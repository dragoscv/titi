<#
.SYNOPSIS
  Fast E2E loop. Starts ONLY what is needed (local relay + web), reuses them if already running.
.EXAMPLE
  pwsh -NoProfile -File scripts/e2e.ps1                 # build web if stale, run full suite
  pwsh -NoProfile -File scripts/e2e.ps1 -Grep chat      # one flow
  pwsh -NoProfile -File scripts/e2e.ps1 -Smoke          # @smoke only (~20 s)
  pwsh -NoProfile -File scripts/e2e.ps1 -Dev            # next dev (hot reload) instead of a prod build
  pwsh -NoProfile -File scripts/e2e.ps1 -BaseUrl https://titi.dragoscatalin.ro -Smoke   # prod smoke, no servers
  Report: pnpm --filter @titi/e2e exec playwright show-report   · traces for failures in e2e/test-results
#>
param([string] $Grep, [switch] $Smoke, [switch] $Dev, [string] $BaseUrl, [string] $RelayUrl, [switch] $Headed)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if ($BaseUrl) { $env:E2E_BASE_URL = $BaseUrl }
if ($RelayUrl) { $env:E2E_RELAY_URL = $RelayUrl }
if ($Dev) { $env:E2E_WEB = 'dev' }
elseif (-not $BaseUrl) {
  # rebuild web only when app sources are newer than the last build
  $stamp = Get-Item 'apps/web/.next/BUILD_ID' -ErrorAction SilentlyContinue
  $newest = Get-ChildItem apps/web/src, packages/app-ui/src, packages/core-wasm/pkg -Recurse -File | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $stamp -or $newest.LastWriteTime -gt $stamp.LastWriteTime) {
    Write-Host 'web build is stale -> next build'
    $env:TITI_TYPECHECKED = '1'
    pnpm --filter @titi/web build
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  }
}
$a = @('--filter', '@titi/e2e', 'exec', 'playwright', 'test')
if ($Smoke) { $a += @('--grep', '@smoke') } elseif ($Grep) { $a += @('--grep', $Grep) }
if ($Headed) { $a += '--headed' }
pnpm @a
exit $LASTEXITCODE
