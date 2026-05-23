param(
  [string]$LightpandaSmokeStatus = "not_run",
  [string]$HeadedProcessLifecycleStatus = "not_implemented",
  [string]$HeadedCdpAttachStatus = "not_implemented",
  [string]$HeadedProfileCompatibilityStatus = "not_run",
  [string]$OutputDir = "data/reports/runtime-adapter"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$adapters = @(
  [ordered]@{
    adapterId = "fake"
    status = "available_for_contract_tests"
    processLifecycleStatus = "in_process"
    cdpAttachStatus = "not_applicable"
    profileRuntimeEvidence = "warning_stub_only"
    fingerprintRuntimeDepth = "none"
    capabilities = @("contract_tests")
    blockers = @("not real browser evidence")
  },
  [ordered]@{
    adapterId = "lightpanda"
    status = if ($LightpandaSmokeStatus -eq "passed") { "evidence_recorded" } else { "contract_ready_smoke_required" }
    processLifecycleStatus = "external_process_contract"
    cdpAttachStatus = if ($LightpandaSmokeStatus -eq "passed") { "passed" } else { "validation_probe_contract" }
    profileRuntimeEvidence = "validation_probe_contract"
    fingerprintRuntimeDepth = "26 projected fields; observed proof requires real Lightpanda/CDP smoke"
    capabilities = @("profile_runtime_probe", "cdp_validation_smoke")
    blockers = if ($LightpandaSmokeStatus -eq "passed") { @() } else { @("repeatable Lightpanda/CDP smoke not attached to this report") }
  },
  [ordered]@{
    adapterId = "headed_external"
    status = if ($HeadedProcessLifecycleStatus -eq "passed" -and $HeadedCdpAttachStatus -eq "passed" -and $HeadedProfileCompatibilityStatus -eq "passed") { "evidence_recorded" } else { "planned_contract_only" }
    processLifecycleStatus = $HeadedProcessLifecycleStatus
    cdpAttachStatus = $HeadedCdpAttachStatus
    profileRuntimeEvidence = $HeadedProfileCompatibilityStatus
    fingerprintRuntimeDepth = "not_implemented"
    capabilities = @("future_headed_process_lifecycle", "future_cdp_session_attach", "future_profile_runtime_compatibility_report")
    blockers = if ($HeadedProcessLifecycleStatus -eq "passed" -and $HeadedCdpAttachStatus -eq "passed" -and $HeadedProfileCompatibilityStatus -eq "passed") { @() } else { @("headed external adapter implementation/evidence smoke not complete", "repository must not host Chromium/Firefox fork") }
  }
)

$blockedAdapters = @($adapters | Where-Object { $_.blockers.Count -gt 0 })
$headedReady = @($adapters | Where-Object { $_.adapterId -eq "headed_external" -and $_.status -eq "evidence_recorded" }).Count -eq 1
$lightpandaReady = @($adapters | Where-Object { $_.adapterId -eq "lightpanda" -and $_.status -eq "evidence_recorded" }).Count -eq 1

$b1b5Evidence = [ordered]@{
  validationObservedCoverage = "partial"
  fingerprintRuntimeDepth = "partial_26_projected_fields"
  sessionPortability = "blocked_requires_second_machine_evidence"
  providerProductionClosure = "blocked_requires_credentials_and_real_smoke"
  runtimeAdapterEvidence = if ($headedReady -and $lightpandaReady) { "passed" } else { "partial_or_blocked" }
}
$adspowerRefreshStatus = if ($b1b5Evidence.validationObservedCoverage -eq "passed" `
  -and $b1b5Evidence.fingerprintRuntimeDepth -eq "passed" `
  -and $b1b5Evidence.sessionPortability -eq "passed" `
  -and $b1b5Evidence.providerProductionClosure -eq "passed" `
  -and $b1b5Evidence.runtimeAdapterEvidence -eq "passed") {
  "ready_for_refresh"
} else {
  "deferred_by_evidence_gate"
}

$status = if ($blockedAdapters.Count -eq 0 -and $adspowerRefreshStatus -eq "ready_for_refresh") {
  "passed"
} else {
  "blocked_evidence_required"
}
$failureReason = if ($status -eq "passed") {
  ""
} else {
  "runtime adapter evidence or B1-B5 AdsPower refresh evidence is incomplete"
}

$report = [ordered]@{
  schemaVersion = "runtime_adapter_evidence_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  adapters = $adapters
  b1b5Evidence = $b1b5Evidence
  adspowerRefreshStatus = $adspowerRefreshStatus
  failureReason = $failureReason
  notes = @(
    "This report is an evidence gate, not a headed browser implementation.",
    "Headed external runtime must remain an adapter boundary and must not turn this repository into a Chromium/Firefox fork host.",
    "AdsPower scoring must stay deferred until B1-B5 evidence is complete."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("runtime-adapter-evidence-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Runtime adapter evidence gate report: $reportPath"
Write-Host "Status: $status"
Write-Host "AdsPower refresh: $adspowerRefreshStatus"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "blocked_evidence_required") { exit 2 }
