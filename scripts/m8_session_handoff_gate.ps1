param(
  [switch]$SkipPortabilityRefresh,
  [string]$OutputDir = "data/reports/m8-session-handoff"
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function New-GateResult([string]$Id, [bool]$Passed, [string]$Evidence) {
  return [ordered]@{
    id = $Id
    status = if ($Passed) { "passed" } else { "failed" }
    evidence = $Evidence
  }
}

function Test-FileContains([string]$Path, [string[]]$Tokens) {
  if (-not (Test-Path -LiteralPath $Path)) { return $false }
  $text = Get-Content -LiteralPath $Path -Raw -Encoding UTF8
  foreach ($token in $Tokens) {
    if ($text.IndexOf($token, [System.StringComparison]::OrdinalIgnoreCase) -lt 0) { return $false }
  }
  return $true
}

function Get-LatestJsonReport([string]$Dir, [string]$Pattern) {
  if (-not (Test-Path -LiteralPath $Dir)) { return $null }
  return Get-ChildItem -LiteralPath $Dir -Filter $Pattern -File |
    Sort-Object LastWriteTimeUtc, Name -Descending |
    Select-Object -First 1
}

$projectRoot = Resolve-ProjectRoot
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$runbookPath = Join-Path $projectRoot "docs/sessionbundle-cross-machine-portability-runbook.md"
$portabilityScriptPath = Join-Path $projectRoot "scripts/session_bundle_portability_smoke.ps1"
$desktopPath = Join-Path $projectRoot "src/desktop/mod.rs"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"
$settingsPath = Join-Path $projectRoot "src/modules/settings/SettingsPage.tsx"
$desktopServicePath = Join-Path $projectRoot "src/services/desktop.ts"
$desktopTypesPath = Join-Path $projectRoot "src/types/desktop.ts"

$portabilityRefresh = [ordered]@{
  command = "powershell -ExecutionPolicy Bypass -File scripts\session_bundle_portability_smoke.ps1"
  skipped = [bool]$SkipPortabilityRefresh
  exitCode = $null
  outputTail = ""
}

if (-not $SkipPortabilityRefresh) {
  Push-Location $projectRoot
  try {
    $output = & powershell -ExecutionPolicy Bypass -File $portabilityScriptPath 2>&1 | Out-String
    $portabilityRefresh.exitCode = $LASTEXITCODE
    $portabilityRefresh.outputTail = (($output -split "`r?`n") | Select-Object -Last 20) -join "`n"
  } finally {
    Pop-Location
  }
}

$sessionReportsDir = Join-Path $projectRoot "data/reports/session-portability"
$latestSessionReport = Get-LatestJsonReport $sessionReportsDir "session-bundle-portability-smoke-*.json"
$sessionReportStatus = "missing"
$sessionCrossMachineComplete = $false
$sessionReportPath = ""
if ($latestSessionReport) {
  $sessionReportPath = $latestSessionReport.FullName
  $sessionReportJson = Get-Content -LiteralPath $latestSessionReport.FullName -Raw | ConvertFrom-Json
  $sessionReportStatus = [string]$sessionReportJson.status
  $sessionCrossMachineComplete = [bool]$sessionReportJson.crossMachineComplete
}

$checks = @(
  (New-GateResult "portability_refresh_successful" ($SkipPortabilityRefresh -or $portabilityRefresh.exitCode -eq 0) "session_bundle_portability_smoke refresh skipped=$([bool]$SkipPortabilityRefresh) exitCode=$($portabilityRefresh.exitCode)"),
  (New-GateResult "runbook_ready" (Test-FileContains $runbookPath @("Source machine", "Target machine", "import preflight", "dry-run restore", "confirmed restore", "restart continuity")) "docs/sessionbundle-cross-machine-portability-runbook.md must describe source/target handoff, preflight, dry-run, confirmed restore, and restart continuity"),
  (New-GateResult "portability_smoke_contract_ready" (Test-FileContains $portabilityScriptPath @("session_bundle_portability_smoke_v2", "gateResults", "nextManualSteps", "cross-machine portability is not complete")) "scripts/session_bundle_portability_smoke.ps1 must expose v2 gate results and manual next steps"),
  (New-GateResult "local_portability_report_ready" ($sessionReportStatus -in @("local_contract_passed", "cross_machine_passed")) "latest session portability report status=$sessionReportStatus path=$sessionReportPath"),
  (New-GateResult "desktop_export_preflight_restore_ready" (Test-FileContains $desktopPath @("export_desktop_session_bundle", "preflight_desktop_session_bundle_import", "restore_desktop_session_bundle", "export_session_bundle_writes_manifest_and_respects_sensitive_payload_flag", "session_bundle_import_preflight_and_restore_write_confirmed_copy")) "desktop SessionBundle commands and tests must cover export, preflight, dry-run, and confirmed local restore"),
  (New-GateResult "settings_operator_surface_ready" (Test-FileContains $settingsPath @("handleSessionBundleExport", "handleSessionBundlePreflight", "handleSessionBundleRestore", "sessionBundleAction")) "Settings must expose the local operator loop"),
  (New-GateResult "typed_desktop_contract_ready" ((Test-FileContains $desktopServicePath @("exportSessionBundle", "preflightSessionBundleImport", "restoreSessionBundle")) -and (Test-FileContains $desktopTypesPath @("DesktopSessionBundleExport", "DesktopSessionBundleImportPreflight", "DesktopSessionBundleRestoreResult"))) "desktop service and shared TS types must include SessionBundle contracts"),
  (New-GateResult "m4_boundary_guard_ready" (Test-FileContains $m4GatePath @("session_bundle_operator_contract", "second-machine portability remains expected_blocked", "session_bundle_portability_smoke")) "M4 gate must keep cross-machine portability externally blocked until real evidence exists")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($sessionReportStatus -eq "cross_machine_passed" -and $sessionCrossMachineComplete -and $failed.Count -eq 0) {
  "passed_cross_machine_evidence_attached"
} elseif ($failed.Count -eq 0) {
  "passed_handoff_package_ready"
} else {
  "failed_handoff_package_contract"
}

$failureReason = if ($status -eq "failed_handoff_package_contract") {
  "M8 handoff package checks failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')"
} else {
  ""
}

$report = [ordered]@{
  schemaVersion = "m8_session_handoff_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  portabilityRefresh = $portabilityRefresh
  sessionPortabilityReport = [ordered]@{
    path = $sessionReportPath
    status = $sessionReportStatus
    crossMachineComplete = $sessionCrossMachineComplete
  }
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    runbookStatus = if (($checks | Where-Object { $_.id -eq "runbook_ready" }).status -eq "passed") { "present" } else { "missing" }
    portabilitySmokeStatus = if (($checks | Where-Object { $_.id -eq "portability_smoke_contract_ready" }).status -eq "passed") { "present" } else { "missing" }
    localPortabilityReportStatus = $sessionReportStatus
    desktopContractStatus = if (($checks | Where-Object { $_.id -eq "desktop_export_preflight_restore_ready" }).status -eq "passed") { "present" } else { "missing" }
    operatorSurfaceStatus = if (($checks | Where-Object { $_.id -eq "settings_operator_surface_ready" }).status -eq "passed") { "present" } else { "missing" }
    nextAction = if ($status -eq "passed_cross_machine_evidence_attached") {
      "Attach the cross-machine SessionBundle evidence to runtime_adapter/B1-B5 scoring and rerun scripts/m4_acceptance_gate.ps1."
    } elseif ($status -eq "passed_handoff_package_ready") {
      "Use the handoff manifest on a second clean Win11 target, then rerun scripts/session_bundle_portability_smoke.ps1 -CrossMachine with passed target statuses."
    } else {
      "Fix missing M8 runbook, smoke, desktop API, Settings operator surface, or M4 boundary checks, then rerun scripts/m8_session_handoff_gate.ps1."
    }
  }
  handoffManifest = [ordered]@{
    sourceMachineSteps = @(
      "Export a redacted test SessionBundle from Settings or export_session_bundle.",
      "Record source profile id, bundle id, collector version, export path, and sensitive payload setting.",
      "Copy the bundle and this report to a clean Win11 target machine."
    )
    targetMachineSteps = @(
      "Run import preflight with a new target profile id.",
      "Run dry-run restore and confirm writePerformed=false.",
      "Run confirmed restore and confirm target profile plus proxy_session_bindings were written.",
      "Restart PersonaPilot and record restart continuity evidence.",
      "Run scripts/session_bundle_portability_smoke.ps1 -CrossMachine with passed target statuses."
    )
    requiredEvidenceFields = @(
      "sourceMachine",
      "targetMachine",
      "bundlePath",
      "sourceProfileId",
      "targetProfileId",
      "preflightStatus",
      "dryRunStatus",
      "confirmedRestoreStatus",
      "restartContinuityStatus",
      "reportPaths"
    )
  }
  liveTruthBoundary = @(
    "This gate validates the M8 SessionBundle handoff package and local operator contract only.",
    "passed_handoff_package_ready means the second-machine runbook, commands, UI/API contracts, and local portability report are ready.",
    "It does not prove cross-machine SessionBundle portability until a real second Win11 target report has crossMachineComplete=true.",
    "It does not prove provider credentials, remote proxy/TLS, AdsPower refresh, full headed realism, or full 450 observed/replay coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m8-session-handoff-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 10 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M8 SessionBundle handoff gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "failed_handoff_package_contract") { exit 1 }
exit 0
