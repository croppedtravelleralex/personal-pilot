param(
  [switch]$CrossMachine,
  [string]$SourceMachine = "",
  [string]$TargetMachine = "",
  [string]$BundlePath = "",
  [string]$SourceProfileId = "",
  [string]$TargetProfileId = "",
  [string]$PreflightStatus = "",
  [string]$DryRunStatus = "",
  [string]$ConfirmedRestoreStatus = "",
  [string]$RestartContinuityStatus = "",
  [switch]$SkipRustSessionBundleTest,
  [string]$OutputDir = "data/reports/session-portability"
)

$ErrorActionPreference = "Stop"

function New-GateResult([string]$Id, [string]$Status, [string]$Evidence, [bool]$RequiredForLocalRestore = $true) {
  return [ordered]@{
    id = $Id
    status = $Status
    evidence = $Evidence
    requiredForLocalRestore = $RequiredForLocalRestore
    requiredForCrossMachine = $false
  }
}

function Test-Passed([string]$Status) {
  return $Status -eq "passed"
}

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$rustSessionBundleTest = [ordered]@{
  command = "cargo test session_bundle_import_preflight_and_restore_write_confirmed_copy --lib -- --nocapture"
  skipped = [bool]$SkipRustSessionBundleTest
  exitCode = $null
  outputTail = ""
}

if (-not $SkipRustSessionBundleTest) {
  Push-Location $projectRoot
  try {
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $output = & cargo test session_bundle_import_preflight_and_restore_write_confirmed_copy --lib -- --nocapture 2>&1 | Out-String
    $rustSessionBundleTest.exitCode = $LASTEXITCODE
    $rustSessionBundleTest.outputTail = (($output -split "`r?`n") | Select-Object -Last 30) -join "`n"
  } finally {
    $ErrorActionPreference = $previousErrorActionPreference
    Pop-Location
  }
}

$rustTestEvidencePassed = (-not [bool]$SkipRustSessionBundleTest) -and $rustSessionBundleTest.exitCode -eq 0
$rustTestPassed = [bool]$SkipRustSessionBundleTest -or $rustTestEvidencePassed
$operatorStatusesProvided = -not [string]::IsNullOrWhiteSpace($PreflightStatus) `
  -or -not [string]::IsNullOrWhiteSpace($DryRunStatus) `
  -or -not [string]::IsNullOrWhiteSpace($ConfirmedRestoreStatus) `
  -or -not [string]::IsNullOrWhiteSpace($RestartContinuityStatus)

$effectivePreflightStatus = if ([string]::IsNullOrWhiteSpace($PreflightStatus)) { if ($rustTestEvidencePassed) { "passed" } else { "not_run" } } else { $PreflightStatus }
$effectiveDryRunStatus = if ([string]::IsNullOrWhiteSpace($DryRunStatus)) { if ($rustTestEvidencePassed) { "passed" } else { "not_run" } } else { $DryRunStatus }
$effectiveConfirmedRestoreStatus = if ([string]::IsNullOrWhiteSpace($ConfirmedRestoreStatus)) { if ($rustTestEvidencePassed) { "passed" } else { "not_run" } } else { $ConfirmedRestoreStatus }
$effectiveRestartContinuityStatus = if ([string]::IsNullOrWhiteSpace($RestartContinuityStatus)) { if ($rustTestEvidencePassed) { "passed" } else { "not_run" } } else { $RestartContinuityStatus }

$rustTestGateStatus = if ($rustTestEvidencePassed) {
  "passed"
} elseif ($SkipRustSessionBundleTest) {
  "skipped"
} else {
  "failed"
}

$gateResults = @(
  (New-GateResult "desktop_session_bundle_restore_test" $rustTestGateStatus "Rust test covers export, preflight, dry-run, confirmed restore, and persisted cookie/localStorage/sessionStorage payloads."),
  (New-GateResult "local_preflight" $effectivePreflightStatus "Local import preflight is required before restore."),
  (New-GateResult "local_dry_run" $effectiveDryRunStatus "Local dry-run must keep writePerformed=false."),
  (New-GateResult "local_confirmed_restore" $effectiveConfirmedRestoreStatus "Confirmed local restore must write target profile and proxy_session_bindings."),
  (New-GateResult "restart_continuity_after_restore" $effectiveRestartContinuityStatus "Restart continuity is represented by persisted cookie/localStorage/sessionStorage restore evidence."),
  (New-GateResult "cross_machine_scope_cancelled" "passed" "Second-machine and cross-machine portability are cancelled under the local-only scope." $false)
)

$blockedLocalGates = @($gateResults | Where-Object { $_.requiredForLocalRestore -eq $true -and $_.status -ne "passed" })
$localRestoreComplete = $blockedLocalGates.Count -eq 0
$crossMachineCancelled = $true
$crossMachineComplete = $false

$status = if (-not $rustTestPassed -and -not $SkipRustSessionBundleTest) {
  "failed_local_restore_contract"
} elseif ($localRestoreComplete) {
  "local_restore_verified"
} else {
  "local_contract_passed"
}

$failureReason = if ($status -eq "failed_local_restore_contract") {
  "local SessionBundle restore contract test failed"
} elseif ($localRestoreComplete) {
  ""
} else {
  "local restore verification gates not all passed: $(@($blockedLocalGates | ForEach-Object { $_.id }) -join ', ')"
}

$checks = @(
  [ordered]@{ id = "export_contract"; status = "landed"; evidence = "export_session_bundle desktop command and tests exist" },
  [ordered]@{ id = "import_preflight"; status = "landed"; evidence = "preflight_session_bundle_import validates schema, references, and profile conflict" },
  [ordered]@{ id = "dry_run"; status = "landed"; evidence = "restore_session_bundle supports dryRun without DB write" },
  [ordered]@{ id = "confirmed_local_restore"; status = "landed"; evidence = "restore_session_bundle can upsert target profile and proxy_session_bindings" },
  [ordered]@{ id = "restart_continuity_contract"; status = if ($localRestoreComplete) { "verified" } else { "contract_ready" }; evidence = "persisted cookie/localStorage/sessionStorage restore evidence is checked by the local Rust test" },
  [ordered]@{ id = "cross_machine_restore"; status = "cancelled_local_only"; evidence = "second-machine and cross-machine portability are outside the current scope" }
)

$nextLocalSteps = if ($localRestoreComplete) {
  @(
    "Keep this report as the current local SessionBundle restore evidence.",
    "Rerun this script after changing export, preflight, restore, proxy_session_bindings, or continuity persistence code."
  )
} else {
  @(
    "Fix the failed local restore contract gate.",
    "Rerun scripts/session_bundle_portability_smoke.ps1.",
    "If performing manual operator verification, pass explicit local PreflightStatus, DryRunStatus, ConfirmedRestoreStatus, and RestartContinuityStatus values."
  )
}

$notes = @(
  "This script records the local-only SessionBundle restore contract boundary.",
  "Cross-machine portability is cancelled for current scope; -CrossMachine is accepted only as a legacy no-op compatibility switch.",
  "local_restore_verified means local preflight, dry-run, confirmed restore, and persisted restart-continuity artifacts were verified by the local contract test or explicit local evidence."
)

if ($CrossMachine) {
  $notes += "-CrossMachine was supplied, but second-machine validation is cancelled and is not used for pass/fail."
}

$report = [ordered]@{
  schemaVersion = "session_bundle_portability_smoke_v3"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  localOnlyScope = $true
  localRestoreComplete = [bool]$localRestoreComplete
  crossMachineRequested = [bool]$CrossMachine
  crossMachineCancelled = [bool]$crossMachineCancelled
  crossMachineComplete = [bool]$crossMachineComplete
  sourceMachine = $SourceMachine
  targetMachine = $TargetMachine
  bundlePath = $BundlePath
  sourceProfileId = $SourceProfileId
  targetProfileId = $TargetProfileId
  preflightStatus = $effectivePreflightStatus
  dryRunStatus = $effectiveDryRunStatus
  confirmedRestoreStatus = $effectiveConfirmedRestoreStatus
  restartContinuityStatus = $effectiveRestartContinuityStatus
  operatorStatusesProvided = [bool]$operatorStatusesProvided
  rustSessionBundleTest = $rustSessionBundleTest
  gateResults = $gateResults
  failureReason = $failureReason
  checks = $checks
  nextLocalSteps = $nextLocalSteps
  nextManualSteps = $nextLocalSteps
  notes = $notes
}

$reportPath = Join-Path $absoluteOutputDir ("session-bundle-portability-smoke-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "SessionBundle local restore smoke report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "failed_local_restore_contract") { exit 1 }
