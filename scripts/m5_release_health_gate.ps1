param(
  [string]$ReportsDir = "data/reports/release-smoke",
  [string]$OutputDir = "data/reports/m5-release-health"
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

function Get-LatestReleaseReport([string]$Directory) {
  if (-not (Test-Path $Directory)) { return $null }
  return Get-ChildItem -Path $Directory -Filter "release-performance-smoke-*.json" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
}

function Test-String([object]$Value) {
  return ($null -ne $Value) -and (-not [string]::IsNullOrWhiteSpace([string]$Value))
}

$projectRoot = Resolve-ProjectRoot
$absoluteReportsDir = Join-Path $projectRoot $ReportsDir
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$latestReport = Get-LatestReleaseReport $absoluteReportsDir
$releaseReport = $null
if ($null -ne $latestReport) {
  $releaseReport = Get-Content -Path $latestReport.FullName -Raw | ConvertFrom-Json
}

$budgetResults = @()
if ($null -ne $releaseReport -and $null -ne $releaseReport.budgetResults) {
  $budgetResults = @($releaseReport.budgetResults)
}
$healthSummary = if ($null -ne $releaseReport) { $releaseReport.healthSummary } else { $null }
$withinDrift = @($budgetResults | Where-Object { $_.status -eq "within_10_percent_drift" })
$withinDriftMissingReason = @($withinDrift | Where-Object { -not (Test-String $_.driftReason) })
$mitigationHints = if ($null -ne $releaseReport -and $null -ne $releaseReport.mitigationHints) { @($releaseReport.mitigationHints) } else { @() }

$checks = @(
  (New-GateResult "latest_release_report_exists" ($null -ne $latestReport) $(if ($null -ne $latestReport) { $latestReport.FullName } else { "missing release performance smoke report" })),
  (New-GateResult "schema_v2" ($null -ne $releaseReport -and $releaseReport.schemaVersion -eq "release_performance_smoke_v2") "schemaVersion must be release_performance_smoke_v2"),
  (New-GateResult "budget_status_present" ($null -ne $releaseReport -and (Test-String $releaseReport.budgetStatus)) "budgetStatus=$($releaseReport.budgetStatus)"),
  (New-GateResult "budget_results_complete" ($budgetResults.Count -ge 3 -and (@($budgetResults | Where-Object { (Test-String $_.id) -and $null -ne $_.target -and $null -ne $_.driftTarget -and (Test-String $_.status) }).Count -ge 3)) "budgetResults count=$($budgetResults.Count)"),
  (New-GateResult "health_summary_present" ($null -ne $healthSummary -and (Test-String $healthSummary.status) -and (Test-String $healthSummary.nextAction)) "healthSummary.status=$($healthSummary.status)"),
  (New-GateResult "mitigation_hints_present" ($mitigationHints.Count -gt 0) "mitigationHints count=$($mitigationHints.Count)"),
  (New-GateResult "drift_reason_enforced" ($withinDriftMissingReason.Count -eq 0) "within-drift metrics without reason=$($withinDriftMissingReason.Count)")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$budgetStatus = if ($null -ne $releaseReport -and (Test-String $releaseReport.budgetStatus)) { [string]$releaseReport.budgetStatus } else { "missing" }
$status = if ($failed.Count -gt 0) {
  "failed"
} elseif ($budgetStatus -eq "within_budget") {
  "passed"
} elseif ($budgetStatus -like "within_10_percent_drift*") {
  "passed_with_recorded_drift"
} else {
  "passed_with_budget_overrun"
}

$report = [ordered]@{
  schemaVersion = "m5_release_health_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  budgetStatus = $budgetStatus
  releaseReportPath = if ($null -ne $latestReport) { $latestReport.FullName } else { $null }
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    budgetResultCount = $budgetResults.Count
    healthStatus = if ($null -ne $healthSummary) { $healthSummary.status } else { "missing" }
    exceededMetricIds = if ($null -ne $healthSummary -and $null -ne $healthSummary.exceededMetricIds) { @($healthSummary.exceededMetricIds) } else { @() }
    nextAction = "Keep this report as local startup diagnostics; release performance budget green is cancelled for current scope."
  }
  liveTruthBoundary = @(
    "This gate validates the historical M5 release health report contract as local diagnostics only.",
    "Release performance budget green is cancelled for current scope.",
    "Provider, remote proxy/TLS, SessionBundle local restore, AdsPower refresh, and full 450 coverage remain outside this diagnostic gate."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m5-release-health-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M5 release health gate report: $reportPath"
Write-Host "Status: $status"
Write-Host "Budget: $budgetStatus"
if ($failed.Count -gt 0) {
  Write-Host "Failed checks: $(@($failed | ForEach-Object { $_.id }) -join ', ')"
  exit 1
}
