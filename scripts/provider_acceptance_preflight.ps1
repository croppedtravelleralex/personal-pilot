param(
  [string]$Domain = "all",
  [string]$ProviderName = "",
  [string]$ManagerWiringStatus = "not_wired",
  [string]$CdpDetectStatus = "not_run",
  [string]$CdpFillStatus = "not_run",
  [string]$OperatorUiStatus = "not_wired",
  [string]$DryRunStatus = "contract_available",
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
  [pscustomobject]@{
    domain = "captcha"
    env = @("TWOCAPTCHA_API_KEY", "CAPSOLVER_API_KEY", "ANTICAPTCHA_API_KEY")
    requiredClosure = @("manager_wiring", "cdp_detect", "cdp_fill", "real_provider_smoke")
    dryRunContract = @(
      "credential_env_check",
      "challenge_detection_contract",
      "solver_request_shape_check",
      "token_fill_contract",
      "failure_reason_preservation"
    )
    failureTaxonomy = @(
      "credential_missing",
      "challenge_detection_not_passed",
      "solver_manager_not_wired",
      "solver_unavailable",
      "token_fill_not_wired",
      "real_smoke_required"
    )
  },
  [pscustomobject]@{
    domain = "sms"
    env = @("FIVESIM_API_KEY", "SMSPOOL_API_KEY", "HEROSMS_API_KEY")
    requiredClosure = @("manager_wiring", "number_purchase", "cdp_fill", "state_flow", "real_provider_smoke")
    dryRunContract = @(
      "credential_env_check",
      "number_purchase_contract",
      "phone_and_code_field_detection_contract",
      "code_wait_timeout_contract",
      "cancel_finish_state_contract"
    )
    failureTaxonomy = @(
      "credential_missing",
      "number_purchase_not_wired",
      "phone_field_detection_not_passed",
      "code_wait_timeout",
      "cancel_finish_not_wired",
      "real_smoke_required"
    )
  },
  [pscustomobject]@{
    domain = "email"
    env = @("MAILTM_API_TOKEN", "EMAIL_WORKER_URL", "EMAIL_WORKER_TOKEN")
    requiredClosure = @("session_persistence", "wait_code", "cdp_fill", "registration_flow_smoke")
    dryRunContract = @(
      "credential_or_endpoint_check",
      "inbox_create_contract",
      "email_session_identity_contract",
      "wait_code_timeout_contract",
      "cdp_fill_contract"
    )
    failureTaxonomy = @(
      "credential_missing",
      "inbox_create_not_wired",
      "wait_code_timeout",
      "session_persistence_not_proven",
      "cdp_fill_not_wired",
      "registration_smoke_required"
    )
  }
)

$selectedDomains = if ($Domain -eq "all") { $domains } else { @($domains | Where-Object { $_.domain -eq $Domain }) }
if ($selectedDomains.Count -eq 0) {
  throw "Domain must be one of all|captcha|sms|email. Got: $Domain"
}

$items = foreach ($domainSpec in $selectedDomains) {
  $domainName = [string]$domainSpec.domain
  $envNames = @($domainSpec.env)
  $requiredClosure = @($domainSpec.requiredClosure)
  $dryRunContract = @($domainSpec.dryRunContract)
  $failureTaxonomy = @($domainSpec.failureTaxonomy)
  $present = @(Test-AnyEnv $envNames)
  $credentialStatus = if ($present.Count -gt 0) { "credential_present" } else { "credential_missing" }
  $blockers = @()
  if ($present.Count -eq 0) { $blockers += "missing provider credential env: $($envNames -join '|')" }
  if ($ManagerWiringStatus -ne "wired") { $blockers += "production manager wiring is not connected to runtime flow" }
  if ($CdpDetectStatus -ne "passed") { $blockers += "CDP challenge/field detection has not passed" }
  if ($CdpFillStatus -ne "passed") { $blockers += "CDP fill automation has not passed" }
  if ($OperatorUiStatus -ne "wired") { $blockers += "operator UI closure is not wired" }
  if ($RealProviderSmokeStatus -ne "passed") { $blockers += "real provider smoke has not passed" }
  $realClosure = $blockers.Count -eq 0
  $dryRunNextAction = if ($blockers.Count -eq 0) {
    "preserve accepted report and rerun production regression when provider config changes"
  } else {
    $blockers[0]
  }
  [ordered]@{
    domain = $domainName
    providerName = $ProviderName
    credentialStatus = $credentialStatus
    presentCredentialNames = $present
    managerWiringStatus = $ManagerWiringStatus
    cdpDetectStatus = $CdpDetectStatus
    cdpFillStatus = $CdpFillStatus
    operatorUiStatus = $OperatorUiStatus
    dryRunStatus = $DryRunStatus
    dryRunAvailable = $true
    dryRunContract = $dryRunContract
    dryRunNextAction = $dryRunNextAction
    failureTaxonomyStatus = "present"
    failureTaxonomy = $failureTaxonomy
    realProviderSmokeStatus = $RealProviderSmokeStatus
    requiredClosure = $requiredClosure
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
$dryRunAvailableCount = @($items | Where-Object { $_.dryRunAvailable -eq $true }).Count
$status = if ($acceptedCount -eq $items.Count) {
  "accepted"
} elseif ($readyCredentialCount -gt 0) {
  "credential_ready_but_runtime_closure_required"
} else {
  "blocked_missing_credentials"
}

$report = [ordered]@{
  schemaVersion = "provider_acceptance_preflight_v3"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  dryRunStatus = if ($dryRunAvailableCount -eq $items.Count) { "available" } else { "incomplete" }
  failureTaxonomyStatus = if (@($items | Where-Object { @($_.failureTaxonomy).Count -eq 0 }).Count -eq 0) { "present" } else { "incomplete" }
  localDryRunOnly = $true
  readyCredentialCount = $readyCredentialCount
  acceptedCount = $acceptedCount
  dryRunAvailableCount = $dryRunAvailableCount
  items = $items
  notes = @(
    "This preflight intentionally does not call paid or external providers.",
    "Dry-run status is local contract evidence only; it does not buy numbers, solve CAPTCHA, create production inboxes, or prove target-site closure.",
    "Real provider acceptance requires configured credentials, production manager wiring, CDP detect/fill, failure handling, and preserved smoke evidence.",
    "Do not mark provider production closure complete from credential presence alone."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("provider-acceptance-preflight-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Provider acceptance preflight report: $reportPath"
Write-Host "Status: $status"

if ($status -ne "accepted") { exit 2 }
