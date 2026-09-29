# Size budgets for shipped artifacts. Fails (exit 1) when an existing artifact is over budget;
# artifacts that were not built are reported as "skip" (build them with gates.ps1 -Build).
# Budgets sit ~15 % above the size measured when they were set (2026-09-29) — raise one only
# with a reason in the commit message.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$budgets = @(
  # debug APKs are unminified (R8 off): budgets catch dependency creep, not the shipped size.
  # 2026-09-29 before/after: phone 54.8 -> 45.5, watch 85.6 -> 50.9, tv 29.2 -> 17.8 MB
  # (abiFilters, icons-extended -> vendored icons, no ui-tooling)
  @{ Name = 'phone debug apk';   Path = 'android/app/build/outputs/apk/gms/debug/app-gms-debug.apk';        MB = 50 }
  @{ Name = 'watch debug apk';   Path = 'android/wear/build/outputs/apk/debug/wear-debug.apk';              MB = 56 }
  @{ Name = 'tv debug apk';      Path = 'android/tv/build/outputs/apk/debug/tv-debug.apk';                  MB = 20 }
  # shipped (R8) sizes; release builds only run from a clean worktree
  @{ Name = 'phone release apk'; Path = 'android/app/build/outputs/apk/gms/release/app-gms-release.apk';    MB = 30 }
  @{ Name = 'watch release apk'; Path = 'android/wear/build/outputs/apk/release/wear-release*.apk';         MB = 12 }
  @{ Name = 'tv release apk';    Path = 'android/tv/build/outputs/apk/release/tv-release*.apk';             MB = 12 }
  @{ Name = 'core wasm';         Path = 'packages/core-wasm/pkg/titi_wasm_bg.wasm';                         MB = 0.8 }
  @{ Name = 'tizen js';          Path = 'apps/tv/dist/assets/index-*.js';                                   MB = 0.6 }
  @{ Name = 'desktop js';        Path = 'apps/desktop/dist/assets/index-*.js';                              MB = 0.55 }
  @{ Name = 'desktop exe';       Path = 'apps/desktop/src-tauri/target/release/titi.exe';                   MB = 12 }
  @{ Name = 'desktop installer'; Path = 'apps/desktop/src-tauri/target/release/bundle/nsis/*-setup.exe';   MB = 4 }
)
$fail = 0
foreach ($b in $budgets) {
  $f = Get-ChildItem (Join-Path $root $b.Path) -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $f) { '{0,-18} skip (not built)' -f $b.Name; continue }
  $mb = $f.Length / 1MB
  $ok = $mb -le $b.MB
  if (-not $ok) { $fail++ }
  '{0,-18} {1,7:N2} MB / {2,5} MB  {3}' -f $b.Name, $mb, $b.MB, ($(if ($ok) { 'ok' } else { 'OVER BUDGET' }))
}
if ($fail) { "size budgets: $fail over"; exit 1 }
'size budgets: ok'
