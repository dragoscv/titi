# Measure the Android gate: warm no-op run, then an incremental run after touching one :client file.
# Writes Gradle --profile HTML reports under android/build/reports/profile and prints the slowest tasks.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $root 'android')
if (-not $env:JAVA_HOME) { $env:JAVA_HOME = 'C:\Program Files\Java\jdk-22' }
$tasks = ':app:compileGmsDebugKotlin', ':app:lintGmsDebug', ':wear:compileDebugKotlin', ':wear:lintDebug', ':app:compileFossDebugKotlin', ':app:checkFossNoGms', ':core-ffi:testDebugUnitTest', ':app:testGmsDebugUnitTest', ':tv:compileDebugKotlin', ':tv:lintDebug'
function Run($label) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  & .\gradlew.bat @tasks --profile --console=plain *> "$env:TEMP\titi-perf-$label.log"
  $sw.Stop()
  $exec = (Select-String -Path "$env:TEMP\titi-perf-$label.log" -Pattern '^> Task ' | Where-Object { $_.Line -notmatch 'UP-TO-DATE|FROM-CACHE|NO-SOURCE|SKIPPED' }).Count
  '{0,-12} {1,6:N1}s  executed tasks: {2}  exit={3}' -f $label, $sw.Elapsed.TotalSeconds, $exec, $LASTEXITCODE
}
Run 'warm-noop'
$f = Join-Path $root 'android/client/src/main/kotlin/ro/titi/app/util/Lifecycle.kt'
Add-Content -Path $f -Value '' -NoNewline
(Get-Item $f).LastWriteTime = Get-Date
Add-Content -Path $f -Value "`n// perf-probe"
Run 'touch-client'
$c = Get-Content $f -Raw; [IO.File]::WriteAllText($f, $c.Replace("`n// perf-probe", ''))
Run 'revert'
$p = Get-ChildItem build/reports/profile -Filter *.html | Sort-Object LastWriteTime -Descending | Select-Object -First 1
"profile: $($p.FullName)"
