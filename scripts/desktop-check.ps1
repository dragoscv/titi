# clippy (-D warnings) + unit tests for the Tauri desktop crate
param([switch] $NoTest)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path (Split-Path -Parent $PSScriptRoot) 'apps/desktop/src-tauri')
cargo clippy --all-targets --message-format short -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if (-not $NoTest) { cargo test --lib; exit $LASTEXITCODE }
