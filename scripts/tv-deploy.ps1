<#
.SYNOPSIS
  Package a Tizen web app dir (with config.xml) as .wgt, install and launch it on a Samsung TV/monitor.
.EXAMPLE
  pwsh -NoProfile -File scripts/tv-deploy.ps1 -Dir apps/tv/probe -AppId TitiProbe0.Probe
  pwsh -NoProfile -File scripts/tv-deploy.ps1 -Dir apps/tv/dist -AppId TitiTvApp0.Titi -Inspect
#>
param(
  [Parameter(Mandatory)] [string] $Dir,
  [Parameter(Mandatory)] [string] $AppId,
  [string] $Device = '192.168.100.135:26101',
  [string] $Profile = 'mixai-samsung',
  [switch] $Inspect
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$ts = Join-Path $env:USERPROFILE 'tizen-studio'
$tz = Join-Path $ts 'tools\ide\bin\tizen.bat'
$sdb = Join-Path $ts 'tools\sdb.exe'
$src = Resolve-Path (Join-Path $root $Dir)
$out = Join-Path $root '.copilot-tmp\tizen-out'
New-Item -ItemType Directory -Force $out | Out-Null
Get-ChildItem $out -Filter *.wgt | Remove-Item

& $sdb connect ($Device.Split(':')[0]) | Out-Null
$devs = & $sdb devices
if (-not ($devs -match [regex]::Escape($Device))) { throw "device $Device not connected: $devs" }

# tizen package signs in place; stage a copy so no signature files land in the source tree
$stage = Join-Path $out 'stage'
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
Copy-Item -Recurse $src $stage
& $tz package -t wgt -s $Profile -o $out -- $stage
if ($LASTEXITCODE -ne 0) { throw 'tizen package failed' }
$wgt = Get-ChildItem $out -Filter *.wgt | Select-Object -First 1
Write-Host "package: $($wgt.Name) $([math]::Round($wgt.Length/1KB)) KB"

& $tz install -n $wgt.Name -s $Device -- $out
if ($LASTEXITCODE -ne 0) { throw 'tizen install failed' }
if ($Inspect) {
  $dbg = & $sdb -s $Device shell 0 debug $AppId 2>&1
  Write-Host $dbg
  if ("$dbg" -match 'port:\s*(\d+)') {
    & $sdb -s $Device forward tcp:9222 "tcp:$($Matches[1])" | Out-Null
    Write-Host 'DevTools: http://localhost:9222/json'
  }
} else {
  & $tz run -p $AppId -s $Device
}
