param(
  [string]$OutputDir = "data/reports/provider-acceptance"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

function Test-AnyEnv([string[]]$Names) {
  $present = @()
  foreach ($name in $Names) {
    $value = [Environment]::GetEnvironmentVariable($name)
    if (-not [string]::IsNullOrWhiteSpace($value)) {
      $present += $name
    }
  }
  return $present
}

$domains = @(
  [ordered]@{
    domain = "captcha"
    env = @("TWOCAPTCHA_API_KEY", "CAPSOLVER_API_KEY", "ANTICAPTCHA_API_KEY")
    requiredClosure = @("manager_wiring", "cdp_detect", "cdp_fill", "real_provider_smoke")
  },
  [ordered]@{
    domain = "sms"
    env = @("FIVESIM_API_KEY", "SMSPOOL_API_KEY", "HEROSMS_API_KEY")
    requiredClosure = @("manager_wiring", "number_purchase", "cdp_fill", "state_flow", "real_provider_smoke")
  },
  [ordered]@{
    domain = "email"
    env = @("MAILTM_API_TOKEN", "EMAIL_WORKER_URL", "EMAIL_WORKER_TOKEN")
    requiredClosure = @("session_persistence", "wait_code", "cdp_fill", "registration_flow_smoke")
  }
)

$items = foreach ($domain in $domains) {
  $present = Test-AnyEnv $domain.env
  $credentialStatus = if ($present.Count -gt 0) { "credential_present" } else { "credential_missing" }
  [ordered]@{
    domain = $domain.domain
    credentialStatus = $credentialStatus
    presentCredentialNames = $present
    requiredClosure = $domain.requiredClosure
    acceptanceStatus = if ($present.Count -gt 0) { "ready_for_real_provider_smoke_after_manager_wiring" } else { "blocked_missing_credentials" }
  }
}

$readyCredentialCount = @($items | Where-Object { $_.credentialStatus -eq "credential_present" }).Count
$status = if ($readyCredentialCount -gt 0) { "credential_ready_but_runtime_closure_required" } else { "blocked_missing_credentials" }

$report = [ordered]@{
  schemaVersion = "provider_acceptance_preflight_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  readyCredentialCount = $readyCredentialCount
  items = $items
  notes = @(
    "This preflight intentionally does not call paid or external providers.",
    "Real provider acceptance requires configured credentials, production manager wiring, CDP detect/fill, failure handling, and preserved smoke evidence.",
    "Do not mark provider production closure complete from credential presence alone."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("provider-acceptance-preflight-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Provider acceptance preflight report: $reportPath"
Write-Host "Status: $status"

if ($status -eq "blocked_missing_credentials") { exit 2 }
