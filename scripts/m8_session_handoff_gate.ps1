param(
  [switch]$SkipPortabilityRefresh,
  [switch]$SkipRustSessionBundleTest,
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
  command = if ($SkipRustSessionBundleTest) {
    "powershell -ExecutionPolicy Bypass -File scripts\session_bundle_portability_smoke.ps1 -SkipRustSessionBundleTest"
  } else {
    "powershell -ExecutionPolicy Bypass -File scripts\session_bundle_portability_smoke.ps1"
  }
  skipped = [bool]$SkipPortabilityRefresh
  exitCode = $null
  outputTail = ""
}

if (-not $SkipPortabilityRefresh) {
  Push-Location $projectRoot
  try {
    $args = @("-ExecutionPolicy", "Bypass", "-File", $portabilityScriptPath)
    if ($SkipRustSessionBundleTest) { $args += "-SkipRustSessionBundleTest" }
    $output = & powershell @args 2>&1 | Out-String
    $portabilityRefresh.exitCode = $LASTEXITCODE
    $portabilityRefresh.outputTail = (($output -split "`r?`n") | Select-Object -Last 30) -join "`n"
  } finally {
    Pop-Location
  }
}

$sessionReportsDir = Join-Path $projectRoot "data/reports/session-portability"
$latestSessionReport = Get-LatestJsonReport $sessionReportsDir "session-bundle-portability-smoke-*.json"
$sessionReportStatus = "missing"
$sessionReportPath = ""
$sessionLocalRestoreComplete = $false
$sessionCrossMachineCancelled = $true
if ($latestSessionReport) {
  $sessionReportPath = $latestSessionReport.FullName
  $sessionReportJson = Get-Content -LiteralPath $latestSessionReport.FullName -Raw | ConvertFrom-Json
  $sessionReportStatus = [string]$sessionReportJson.status
  $sessionLocalRestoreComplete = (Get-Member -InputObject $sessionReportJson -Name "localRestoreComplete" -MemberType NoteProperty) -and [bool]$sessionReportJson.localRestoreComplete
  $sessionCrossMachineCancelled = -not (Get-Member -InputObject $sessionReportJson -Name "crossMachineCancelled" -MemberType NoteProperty) -or [bool]$sessionReportJson.crossMachineCancelled
}

$localReadyStatuses = @("local_restore_verified", "local_contract_passed", "cross_machine_passed")
$checks = @(
  (New-GateResult "portability_refresh_successful" ($SkipPortabilityRefresh -or $portabilityRefresh.exitCode -eq 0) "session_bundle_portability_smoke refresh skipped=$([bool]$SkipPortabilityRefresh) exitCode=$($portabilityRefresh.exitCode)"),
  (New-GateResult "runbook_historical_cancelled" (Test-FileContains $runbookPath @("CANCELLED", "historical-only", "confirmed local restore", "restart continuity")) "historical runbook must state that second-machine portability is cancelled and local restore remains current scope"),
  (New-GateResult "local_restore_smoke_contract_ready" (Test-FileContains $portabilityScriptPath @("session_bundle_portability_smoke_v3", "localRestoreComplete", "crossMachineCancelled", "nextLocalSteps")) "scripts/session_bundle_portability_smoke.ps1 must expose v3 local restore fields"),
  (New-GateResult "local_portability_report_ready" ($sessionReportStatus -in $localReadyStatuses) "latest session portability report status=$sessionReportStatus path=$sessionReportPath"),
  (New-GateResult "desktop_export_preflight_restore_ready" (Test-FileContains $desktopPath @("export_desktop_session_bundle", "preflight_desktop_session_bundle_import", "restore_desktop_session_bundle", "export_session_bundle_writes_manifest_and_respects_sensitive_payload_flag", "session_bundle_import_preflight_and_restore_write_confirmed_copy")) "desktop SessionBundle commands and tests must cover export, preflight, dry-run, confirmed local restore, and persisted session artifacts"),
  (New-GateResult "settings_operator_surface_ready" (Test-FileContains $settingsPath @("handleSessionBundleExport", "handleSessionBundlePreflight", "handleSessionBundleRestore", "sessionBundleAction")) "Settings must expose the local operator loop"),
  (New-GateResult "typed_desktop_contract_ready" ((Test-FileContains $desktopServicePath @("exportSessionBundle", "preflightSessionBundleImport", "restoreSessionBundle")) -and (Test-FileContains $desktopTypesPath @("DesktopSessionBundleExport", "DesktopSessionBundleImportPreflight", "DesktopSessionBundleRestoreResult"))) "desktop service and shared TS types must include SessionBundle contracts"),
  (New-GateResult "m4_local_only_boundary_ready" (Test-FileContains $m4GatePath @("session_bundle_operator_contract", "local-only SessionBundle", "session_bundle_portability_smoke")) "M4 gate must treat SessionBundle as local-only and not require second-machine evidence")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -gt 0) {
  "failed_local_restore_contract"
} elseif ($sessionReportStatus -eq "local_restore_verified" -and $sessionLocalRestoreComplete) {
  "passed_local_restore_verified"
} else {
  "passed_local_restore_contract_ready"
}

$failureReason = if ($status -eq "failed_local_restore_contract") {
  "M8 local restore checks failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')"
} else {
  ""
}

$nextAction = if ($status -eq "passed_local_restore_verified") {
  "Keep this local SessionBundle restore report attached; rerun scripts/m8_session_handoff_gate.ps1 after changing export, preflight, restore, or continuity persistence."
} elseif ($status -eq "passed_local_restore_contract_ready") {
  "Run scripts/session_bundle_portability_smoke.ps1 without -SkipRustSessionBundleTest to refresh local restore verification when needed."
} else {
  "Fix missing local restore smoke, desktop API, Settings operator surface, typed contract, or M4 local-only boundary checks, then rerun scripts/m8_session_handoff_gate.ps1."
}

$report = [ordered]@{
  schemaVersion = "m8_session_handoff_gate_v2"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  localOnlyScope = $true
  portabilityRefresh = $portabilityRefresh
  sessionPortabilityReport = [ordered]@{
    path = $sessionReportPath
    status = $sessionReportStatus
    localRestoreComplete = $sessionLocalRestoreComplete
    crossMachineCancelled = $sessionCrossMachineCancelled
    crossMachineComplete = $false
  }
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    runbookStatus = if (($checks | Where-Object { $_.id -eq "runbook_historical_cancelled" }).status -eq "passed") { "historical_cancelled" } else { "missing" }
    portabilitySmokeStatus = if (($checks | Where-Object { $_.id -eq "local_restore_smoke_contract_ready" }).status -eq "passed") { "present" } else { "missing" }
    localPortabilityReportStatus = $sessionReportStatus
    localRestoreStatus = if ($sessionLocalRestoreComplete) { "verified" } else { "contract_ready" }
    desktopContractStatus = if (($checks | Where-Object { $_.id -eq "desktop_export_preflight_restore_ready" }).status -eq "passed") { "present" } else { "missing" }
    operatorSurfaceStatus = if (($checks | Where-Object { $_.id -eq "settings_operator_surface_ready" }).status -eq "passed") { "present" } else { "missing" }
    crossMachineScope = "cancelled_local_only"
    nextAction = $nextAction
  }
  localRestoreManifest = [ordered]@{
    localSteps = @(
      "Export a redacted or sensitive-included SessionBundle from Settings or export_session_bundle.",
      "Run import preflight with a target profile id.",
      "Run dry-run restore and confirm writePerformed=false.",
      "Run confirmed restore and confirm target profile plus proxy_session_bindings were written.",
      "Confirm persisted cookie/localStorage/sessionStorage artifacts are available for restart continuity."
    )
    requiredEvidenceFields = @(
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
    "This gate validates the local-only SessionBundle restore package and operator contract.",
    "Second-machine, clean Win11 target, and cross-machine portability are cancelled under the current scope.",
    "passed_local_restore_verified means local export/preflight/dry-run/confirmed restore and persisted restart-continuity artifacts are represented by local evidence.",
    "CAPTCHA/SMS/Email credential-backed provider smoke remains separate from this local restore gate."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m8-session-handoff-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 10 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M8 SessionBundle local restore gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "failed_local_restore_contract") { exit 1 }
exit 0
