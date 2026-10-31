<#
.SYNOPSIS
  Build the Titi relay container with Cloud Build and deploy it to Cloud Run.
.EXAMPLE
  pwsh -NoProfile -File scripts/deploy-backend.ps1 -Project titi-prod
#>
param(
  [Parameter(Mandatory)] [string] $Project,
  [string] $Region = 'europe-west1',
  [string] $Service = 'titi-relay',
  [string] $Repo = 'titi',
  [switch] $SkipBuild,
  # write url/wss/revision to $GITHUB_OUTPUT (CI)
  [switch] $GithubOutput
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# deploy-clean.ps1 / CI export DEPLOY_SHA; fall back to HEAD for manual runs
$sha = if ($env:DEPLOY_SHA) { $env:DEPLOY_SHA } else { (git rev-parse HEAD).Trim() }
$tag = $sha.Substring(0, 12)
$image = "$Region-docker.pkg.dev/$Project/$Repo/$Service`:$tag"

if (-not $SkipBuild) {
  # Artifact Registry repo is idempotent to create
  $exists = gcloud artifacts repositories describe $Repo --location $Region --project $Project --format json 2>$null
  if (-not $exists) {
    gcloud artifacts repositories create $Repo --repository-format docker --location $Region --project $Project
  }
  gcloud builds submit --project $Project --region $Region --timeout 1200 `
    --config apps/backend/cloudbuild.yaml --substitutions "_IMAGE=$image" .
  if ($LASTEXITCODE -ne 0) { throw 'cloud build failed' }
}

gcloud run deploy $Service --project $Project --region $Region --image $image `
  --platform managed --allow-unauthenticated --port 8080 `
  --execution-environment gen2 --session-affinity --timeout 3600 `
  --min-instances 0 --max-instances 1 --concurrency 1000 --cpu 1 --memory 512Mi `
  --set-env-vars "LOG_LEVEL=info,MAX_ROOM_SIZE=64,GIT_SHA=$sha,APNS_TEAM_ID=S6D533QSV2" `
  --set-secrets "APNS_KEY_P8=projects/296248082233/secrets/apns-auth-key-p8:latest,APNS_KEY_ID=projects/296248082233/secrets/apns-key-id:latest,DEMO_BOT_SEED=titi-demo-bot-seed:latest,DEMO_BOT_GROUPS=titi-demo-bot-groups:latest"
# max-instances 1: rooms, push registrations and the review demo bot live in process memory (ADR-0011);
# members on different instances would never meet. One vCPU forwards ~10k frames/s.
if ($LASTEXITCODE -ne 0) { throw 'cloud run deploy failed' }

$svc = gcloud run services describe $Service --project $Project --region $Region --format json | ConvertFrom-Json
$url = $svc.status.url
$rev = $svc.status.latestReadyRevisionName
Write-Host "live: $url  revision: $rev  sha: $sha"
if ($GithubOutput -and $env:GITHUB_OUTPUT) {
  "url=$url", "wss=$($url -replace '^https', 'wss')/v1/ws", "revision=$rev" | Add-Content -Path $env:GITHUB_OUTPUT
}
Invoke-RestMethod "$url/health" | ConvertTo-Json -Compress
