param(
  [string]$Domain = "all",
  [string]$ProviderName = "",
  [string]$ManagerWiringStatus = "not_wired",
  [string]$CdpDetectStatus = "not_run",
  [string]$CdpFillStatus = "not_run",
  [string]$OperatorUiStatus = "not_wired",
  [string]$RealProviderSmokeStatus = "not_run",
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

$selectedDomains = if ($Domain -eq "all") { $domains } else { @($domains | Where-Object { $_.domain -eq $Domain }) }
if ($selectedDomains.Count -eq 0) {
  throw "Domain must be one of all|captcha|sms|email. Got: $Domain"
}

$items = foreach ($domain in $selectedDomains) {
  $present = Test-AnyEnv $domain.env
  $credentialStatus = if ($present.Count -gt 0) { "credential_present" } else { "credential_missing" }
  $blockers = @()
  if ($present.Count -eq 0) { $blockers += "missing provider credential env: $($domain.env -join '|')" }
  if ($ManagerWiringStatus -ne "wired") { $blockers += "production manager wiring is not connected to runtime flow" }
  if ($CdpDetectStatus -ne "passed") { $blockers += "CDP challenge/field detection has not passed" }
  if ($CdpFillStatus -ne "passed") { $blockers += "CDP fill automation has not passed" }
  if ($OperatorUiStatus -ne "wired") { $blockers += "operator UI closure is not wired" }
  if ($RealProviderSmokeStatus -ne "passed") { $blockers += "real provider smoke has not passed" }
  $realClosure = $blockers.Count -eq 0
  [ordered]@{
    domain = $domain.domain
    providerName = $ProviderName
    credentialStatus = $credentialStatus
    presentCredentialNames = $present
    managerWiringStatus = $ManagerWiringStatus
    cdpDetectStatus = $CdpDetectStatus
    cdpFillStatus = $CdpFillStatus
    operatorUiStatus = $OperatorUiStatus
    realProviderSmokeStatus = $RealProviderSmokeStatus
    requiredClosure = $domain.requiredClosure
    blockers = $blockers
    failureReason = if ($blockers.Count -eq 0) { "" } else { $blockers -join "; " }
    acceptanceStatus = if ($realClosure) {
      "accepted"
    } elseif ($present.Count -gt 0) {
      "credential_ready_but_runtime_closure_required"
    } else {
      "blocked_missing_credentials"
    }
  }
}

$readyCredentialCount = @($items | Where-Object { $_.credentialStatus -eq "credential_present" }).Count
$acceptedCount = @($items | Where-Object { $_.acceptanceStatus -eq "accepted" }).Count
$status = if ($acceptedCount -eq $items.Count) {
  "accepted"
} elseif ($readyCredentialCount -gt 0) {
  "credential_ready_but_runtime_closure_required"
} else {
  "blocked_missing_credentials"
}

$report = [ordered]@{
  schemaVersion = "provider_acceptance_preflight_v2"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  readyCredentialCount = $readyCredentialCount
  acceptedCount = $acceptedCount
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

if ($status -ne "accepted") { exit 2 }
