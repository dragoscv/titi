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
  [switch] $SkipBuild
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$tag = (git rev-parse --short HEAD).Trim()
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
  --min-instances 0 --max-instances 3 --concurrency 250 --cpu 1 --memory 512Mi `
  --set-env-vars "LOG_LEVEL=info,MAX_ROOM_SIZE=64"
if ($LASTEXITCODE -ne 0) { throw 'cloud run deploy failed' }

$url = gcloud run services describe $Service --project $Project --region $Region --format 'value(status.url)'
Write-Host "live: $url"
curl.exe -sS "$url/health"
