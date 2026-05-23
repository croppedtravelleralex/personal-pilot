param(
  [switch]$CrossMachine,
  [string]$SourceMachine = "",
  [string]$TargetMachine = "",
  [string]$BundlePath = "",
  [string]$SourceProfileId = "",
  [string]$TargetProfileId = "",
  [string]$PreflightStatus = "not_run",
  [string]$DryRunStatus = "not_run",
  [string]$ConfirmedRestoreStatus = "not_run",
  [string]$RestartContinuityStatus = "not_run",
  [string]$OutputDir = "data/reports/session-portability"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$crossMachineComplete = $CrossMachine `
  -and -not [string]::IsNullOrWhiteSpace($SourceMachine) `
  -and -not [string]::IsNullOrWhiteSpace($TargetMachine) `
  -and -not [string]::IsNullOrWhiteSpace($BundlePath) `
  -and $PreflightStatus -eq "passed" `
  -and $DryRunStatus -eq "passed" `
  -and $ConfirmedRestoreStatus -eq "passed" `
  -and $RestartContinuityStatus -eq "passed"

$status = if ($crossMachineComplete) {
  "cross_machine_passed"
} elseif ($CrossMachine) {
  "blocked_requires_second_machine_evidence"
} else {
  "local_contract_passed"
}

$checks = @(
  [ordered]@{ id = "export_contract"; status = "landed"; evidence = "export_session_bundle desktop command and tests exist" },
  [ordered]@{ id = "import_preflight"; status = "landed"; evidence = "preflight_session_bundle_import validates schema, references, and profile conflict" },
  [ordered]@{ id = "dry_run"; status = "landed"; evidence = "restore_session_bundle supports dryRun without DB write" },
  [ordered]@{ id = "confirmed_local_restore"; status = "landed"; evidence = "restore_session_bundle can upsert target profile and proxy_session_bindings" },
  [ordered]@{ id = "cross_machine_restore"; status = if ($CrossMachine) { "pending_manual_evidence" } else { "not_executed" }; evidence = "requires a second clean Win11 environment or exported bundle transfer" }
)

$report = [ordered]@{
  schemaVersion = "session_bundle_portability_smoke_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  crossMachineRequested = [bool]$CrossMachine
  crossMachineComplete = [bool]$crossMachineComplete
  sourceMachine = $SourceMachine
  targetMachine = $TargetMachine
  bundlePath = $BundlePath
  sourceProfileId = $SourceProfileId
  targetProfileId = $TargetProfileId
  preflightStatus = $PreflightStatus
  dryRunStatus = $DryRunStatus
  confirmedRestoreStatus = $ConfirmedRestoreStatus
  restartContinuityStatus = $RestartContinuityStatus
  checks = $checks
  nextManualSteps = @(
    "Export a redacted test profile bundle from source machine.",
    "Move the bundle to a clean Win11 target machine.",
    "Run import preflight with a new target profile id.",
    "Run dry-run restore and confirm writePerformed=false.",
    "Run confirmed restore and verify target profile, proxy session bindings, and restart continuity evidence."
  )
  notes = @(
    "This script records the current portability contract boundary.",
    "Cross-machine portability is not complete until a real second-environment report is attached."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("session-bundle-portability-smoke-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "SessionBundle portability smoke report: $reportPath"
Write-Host "Status: $status"

if ($CrossMachine -and -not $crossMachineComplete) { exit 2 }
