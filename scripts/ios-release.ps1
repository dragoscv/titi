<#
.SYNOPSIS
  Build / TestFlight-upload the Titi iPhone app on the devcloud macOS VM (vmui-mac).

.DESCRIPTION
  Ships the COMMITTED tree at -Ref (git archive of core/, ios/, scripts/, testvectors/, proto/ -- never the
  dirty working tree) to the guest, then runs ios/scripts/build.sh there. For -Mode upload the shared Apple
  team key is read from Secret Manager (project dragos-devcloud: apple-asc-key-p8 / -key-id / -issuer,
  apple-team-id), copied 0600 to the guest and deleted in the same remote command; nothing secret is printed.
  Build number = git rev-list --count <sha>. Runbook: E:\gh\codai\docs\runbooks\apple-publishing.md.

.EXAMPLE
  pwsh -NoProfile -File scripts/ios-release.ps1 -Mode build
  pwsh -NoProfile -File scripts/ios-release.ps1 -Mode upload -Ref origin/main
#>
[CmdletBinding()]
param(
  [ValidateSet('build', 'upload')] [string]$Mode = 'build',
  [string]$Ref = 'HEAD',
  [string]$GuestHost = '100.106.244.19',
  [int]$GuestPort = 10022,
  [string]$GuestUser = 'dragos',
  [string]$Key = (Join-Path $HOME '.ssh\id_ed25519_macvm'),
  [string]$Project = 'dragos-devcloud',
  [switch]$NoStart
)
$ErrorActionPreference = 'Stop'
$repo = (git rev-parse --show-toplevel).Trim()
$sha = (git -C $repo rev-parse --short=12 "$Ref^{commit}").Trim()
$tmp = Join-Path $repo ".copilot-tmp\ios-release\$sha"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$ssh = @('-o', 'BatchMode=yes', '-o', 'ConnectTimeout=20', '-o', 'ServerAliveInterval=30', '-i', $Key, '-p', "$GuestPort")
$target = "$GuestUser" + [char]64 + "$GuestHost"

function Invoke-Guest([string]$cmd) {
  & ssh @ssh $target $cmd
  if ($LASTEXITCODE -ne 0) { throw "guest command failed ($LASTEXITCODE): $($cmd.Split(';')[0])" }
}

if (-not $NoStart) {
  $st = (gcloud compute instances describe vmui-mac --zone europe-central2-a --project dragos-devcloud --format='value(status)' 2>$null)
  if ($st -eq 'SUSPENDED') { gcloud compute instances resume vmui-mac --zone europe-central2-a --project dragos-devcloud --quiet | Out-Null }
  elseif ($st -ne 'RUNNING') { & pwsh -NoProfile -File 'E:\gh\vmui\infra\devcloud\mac\mac-gce.ps1' -Start }
}
$ok = $false
for ($i = 0; $i -lt 30 -and -not $ok; $i++) {
  & ssh @ssh $target 'true' 2>$null
  $ok = $LASTEXITCODE -eq 0
  if (-not $ok) { Start-Sleep -Seconds 10 }
}
if (-not $ok) { throw "guest ssh not reachable at ${GuestHost}:$GuestPort" }

# Apple rejects API-key JWTs from a skewed clock (401 listTeams.action): refuse > 120 s.
$guestEpoch = [long]((& ssh @ssh $target 'date -u +%s') | Select-Object -Last 1)
$skew = [math]::Abs($guestEpoch - [DateTimeOffset]::UtcNow.ToUnixTimeSeconds())
if ($skew -gt 120) { throw "guest clock skew ${skew}s > 120 s: run 'sudo sntp -sS time.apple.com' on the guest" }

$tar = Join-Path $tmp 'src.tar'
git -C $repo archive --format=tar -o $tar $sha core ios scripts testvectors proto
$remote = "titi-ios/$sha"
Invoke-Guest "rm -rf ~/$remote && mkdir -p ~/$remote"
& scp -q -o BatchMode=yes -i $Key -P $GuestPort $tar "${target}:$remote/src.tar"
if ($LASTEXITCODE -ne 0) { throw 'scp failed' }
Invoke-Guest "cd ~/$remote && tar -xf src.tar && rm src.tar && chmod +x ios/scripts/*.sh scripts/*.sh"

$build = (git -C $repo rev-list --count $sha).Trim()
$log = Join-Path $tmp "$Mode.log"
if ($Mode -eq 'build') {
  & ssh @ssh $target "cd ~/$remote && BUILD_NUMBER=$build ./ios/scripts/build.sh build" *>&1 | Tee-Object -FilePath $log
  $rc = $LASTEXITCODE
} else {
  $p8 = Join-Path $tmp 'k.p8'
  try {
    gcloud secrets versions access latest --secret=apple-asc-key-p8 --project $Project --out-file=$p8 2>$null
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $p8)) { throw 'secret apple-asc-key-p8 missing' }
    $kid = (gcloud secrets versions access latest --secret=apple-asc-key-id --project $Project 2>$null).Trim()
    $iss = (gcloud secrets versions access latest --secret=apple-asc-issuer --project $Project 2>$null).Trim()
    $team = (gcloud secrets versions access latest --secret=apple-team-id --project $Project 2>$null).Trim()
    if ($kid -notmatch '^[A-Z0-9]{8,12}$' -or $team -notmatch '^[A-Z0-9]{10}$' -or $iss -notmatch '^[0-9a-f-]{36}$') { throw 'ASC ids have an unexpected shape' }
    & scp -q -o BatchMode=yes -i $Key -P $GuestPort $p8 "${target}:$remote/.k.p8"
    if ($LASTEXITCODE -ne 0) { throw 'scp key failed' }
  } finally {
    Remove-Item $p8 -Force -ErrorAction SilentlyContinue
  }
  $cmd = "cd ~/$remote && chmod 600 .k.p8 && DEVELOPMENT_TEAM=$team ASC_KEY_ID=$kid ASC_ISSUER_ID=$iss ASC_KEY_PATH=`$PWD/.k.p8 BUILD_NUMBER=$build ./ios/scripts/build.sh upload; rc=`$?; rm -f .k.p8; exit `$rc"
  & ssh @ssh $target $cmd *>&1 | Tee-Object -FilePath $log
  $rc = $LASTEXITCODE
}
Write-Host "ios-release $Mode @ $sha build $build -> rc=$rc (log $log)"
exit $rc
