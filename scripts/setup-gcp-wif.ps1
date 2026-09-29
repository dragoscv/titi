<#
.SYNOPSIS
  Idempotent keyless GitHub Actions -> GCP setup (Workload Identity Federation). No JSON keys are created.
  Creates: WIF pool 'github' + OIDC provider restricted to one repo, deployer SA with least privilege.
.EXAMPLE
  pwsh -NoProfile -File scripts/setup-gcp-wif.ps1 -Repo dragoscv/titi
#>
param(
  [Parameter(Mandatory)] [string] $Repo,
  [string] $Project = 'hai-small-apps',
  [string] $Region = 'europe-west1',
  [string] $Pool = 'github',
  [string] $Provider = 'titi',
  [string] $Sa = 'titi-deployer'
)
$ErrorActionPreference = 'Stop'
$num = gcloud projects describe $Project --format 'value(projectNumber)'
$saEmail = "$Sa@$Project.iam.gserviceaccount.com"
function Exists([scriptblock] $b) { & $b 2>$null | Out-Null; return $LASTEXITCODE -eq 0 }

gcloud services enable iamcredentials.googleapis.com sts.googleapis.com run.googleapis.com cloudbuild.googleapis.com artifactregistry.googleapis.com --project $Project | Out-Null

if (-not (Exists { gcloud iam workload-identity-pools describe $Pool --location global --project $Project })) {
  gcloud iam workload-identity-pools create $Pool --location global --project $Project --display-name 'GitHub Actions'
}
if (-not (Exists { gcloud iam workload-identity-pools providers describe $Provider --workload-identity-pool $Pool --location global --project $Project })) {
  gcloud iam workload-identity-pools providers create-oidc $Provider --workload-identity-pool $Pool --location global --project $Project `
    --issuer-uri 'https://token.actions.githubusercontent.com' `
    --attribute-mapping 'google.subject=assertion.sub,attribute.repository=assertion.repository,attribute.ref=assertion.ref' `
    --attribute-condition "assertion.repository == '$Repo'"
}
if (-not (Exists { gcloud iam service-accounts describe $saEmail --project $Project })) {
  gcloud iam service-accounts create $Sa --project $Project --display-name 'titi CI deployer'
}
# least privilege: deploy Cloud Run, submit builds, push images, act as the runtime SA
foreach ($role in 'roles/run.admin', 'roles/cloudbuild.builds.editor', 'roles/artifactregistry.writer', 'roles/storage.admin', 'roles/serviceusage.serviceUsageConsumer', 'roles/logging.viewer') {
  gcloud projects add-iam-policy-binding $Project --member "serviceAccount:$saEmail" --role $role --condition None --quiet | Out-Null
}
gcloud iam service-accounts add-iam-policy-binding "$num-compute@developer.gserviceaccount.com" --project $Project `
  --member "serviceAccount:$saEmail" --role roles/iam.serviceAccountUser --quiet | Out-Null
gcloud iam service-accounts add-iam-policy-binding $saEmail --project $Project --role roles/iam.workloadIdentityUser `
  --member "principalSet://iam.googleapis.com/projects/$num/locations/global/workloadIdentityPools/$Pool/attribute.repository/$Repo" --quiet | Out-Null

[pscustomobject]@{
  GCP_WIF_PROVIDER = "projects/$num/locations/global/workloadIdentityPools/$Pool/providers/$Provider"
  GCP_DEPLOY_SA    = $saEmail
  GCP_PROJECT      = $Project
  GCP_REGION       = $Region
}
