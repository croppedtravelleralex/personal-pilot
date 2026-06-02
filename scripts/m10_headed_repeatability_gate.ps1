param(
  [string]$HeadedReportsDir = "data/reports/headed-external-smoke",
  [string]$OutputDir = "data/reports/m10-headed-repeatability"
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Get-Field($Value, [string]$Name) {
  if ($null -eq $Value) { return $null }
  $property = $Value.PSObject.Properties[$Name]
  if ($null -ne $property) { return $property.Value }
  return $null
}

function Test-String([object]$Value) {
  return ($null -ne $Value) -and (-not [string]::IsNullOrWhiteSpace([string]$Value))
}

function ConvertTo-Int([object]$Value) {
  if ($null -eq $Value) { return 0 }
  $number = 0
  if ([int]::TryParse([string]$Value, [ref]$number)) { return $number }
  return 0
}

function New-GateResult([string]$Id, [bool]$Passed, [string]$Evidence) {
  return [ordered]@{
    id = $Id
    status = if ($Passed) { "passed" } else { "failed" }
    evidence = $Evidence
  }
}

function ConvertTo-ReportSortKey($Value, $Fallback) {
  if ($null -ne $Value) {
    $text = [string]$Value
    $epochMs = 0L
    if ([Int64]::TryParse($text, [ref]$epochMs)) {
      return [DateTimeOffset]::FromUnixTimeMilliseconds($epochMs).UtcDateTime
    }

    try {
      return ([DateTimeOffset]::Parse($text)).UtcDateTime
    } catch {}
  }

  return $Fallback.ToUniversalTime()
}

function Get-ValidationSignals($Report) {
  $task = Get-Field $Report "realBinaryTask"
  $result = Get-Field $task "result"
  $signals = Get-Field $result "validation_signals"
  if ($null -eq $signals) {
    $signals = Get-Field $task "validation_signals"
  }
  if ($null -eq $signals) { return @() }
  return @($signals)
}

function Get-SignalCategories($Signals) {
  $categories = @()
  foreach ($signal in @($Signals)) {
    $category = [string](Get-Field $signal "category")
    if (-not [string]::IsNullOrWhiteSpace($category)) {
      $categories += $category
    }
  }
  return @($categories | Sort-Object -Unique)
}

function Count-SignalStatus($Signals, [string]$Status) {
  return @(@($Signals) | Where-Object { [string](Get-Field $_ "status") -eq $Status }).Count
}

function Get-HeadedReportEvidenceRank($Report) {
  $status = [string](Get-Field $Report "status")
  $task = Get-Field $Report "realBinaryTask"
  $taskStatus = [string](Get-Field $task "status")
  if ($taskStatus -ne "passed") { return 0 }

  $result = Get-Field $task "result"
  $action = [string](Get-Field $result "action")
  if ([string]::IsNullOrWhiteSpace($action)) {
    $action = [string](Get-Field $task "action")
  }
  $signals = Get-ValidationSignals $Report
  $repeatability = Get-Field $Report "repeatability"
  $repeatabilityStatus = [string](Get-Field $repeatability "status")

  if ($status -eq "passed_real_binary_repeatability" -and $repeatabilityStatus -eq "passed_repeatability_partial_coherence") {
    return 30
  }
  if ($status -eq "passed_real_binary_validation_probe" -and $action -eq "validation_probe" -and @($signals).Count -gt 0) {
    return 20
  }
  if ($status -eq "passed_real_binary_task") {
    return 10
  }
  return 0
}

function Read-HeadedReportItems([string]$Directory) {
  if (-not (Test-Path $Directory)) { return @() }

  $items = @()
  Get-ChildItem -Path $Directory -Filter *.json -File | ForEach-Object {
    try {
      $value = Get-Content -Path $_.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
      $items += [pscustomobject]@{
        path = $_.FullName
        report = $value
        generatedAt = Get-Field $value "generatedAt"
        sortKey = ConvertTo-ReportSortKey (Get-Field $value "generatedAt") $_.LastWriteTimeUtc
        rank = Get-HeadedReportEvidenceRank $value
      }
    } catch {}
  }

  return @($items | Sort-Object -Property @{Expression = { $_.rank }; Descending = $true }, @{Expression = { $_.sortKey }; Descending = $true })
}

$projectRoot = Resolve-ProjectRoot
$absoluteHeadedReportsDir = Join-Path $projectRoot $HeadedReportsDir
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$items = Read-HeadedReportItems $absoluteHeadedReportsDir
$best = if ($items.Count -gt 0) { $items[0] } else { $null }
$headedReport = if ($null -ne $best) { $best.report } else { $null }
$headedReportPath = if ($null -ne $best) { $best.path } else { $null }
$headedReportStatus = [string](Get-Field $headedReport "status")
$task = Get-Field $headedReport "realBinaryTask"
$taskStatus = [string](Get-Field $task "status")
$result = Get-Field $task "result"
$action = [string](Get-Field $result "action")
if ([string]::IsNullOrWhiteSpace($action)) {
  $action = [string](Get-Field $task "action")
}
$signals = Get-ValidationSignals $headedReport
$signalCount = @($signals).Count
$warningCount = Count-SignalStatus $signals "warning"
$categories = Get-SignalCategories $signals
$repeatability = Get-Field $headedReport "repeatability"
$repeatabilityStatus = [string](Get-Field $repeatability "status")
$requestedCount = ConvertTo-Int (Get-Field $repeatability "requestedCount")
$executedCount = ConvertTo-Int (Get-Field $repeatability "executedCount")
$passedCount = ConvertTo-Int (Get-Field $repeatability "passedCount")
$allAttemptsPassed = (Get-Field $repeatability "allAttemptsPassed") -eq $true
$stableSignalCount = (Get-Field $repeatability "stableSignalCount") -eq $true
$stableCategories = (Get-Field $repeatability "stableCategories") -eq $true
$stableSignalStatuses = (Get-Field $repeatability "stableSignalStatuses") -eq $true
$repeatabilityComplete = $requestedCount -ge 2 -and $executedCount -eq $requestedCount -and $passedCount -eq $requestedCount
$repeatabilityStable = $stableSignalCount -and $stableCategories -and $stableSignalStatuses
$repeatabilityPassed = $headedReportStatus -eq "passed_real_binary_repeatability" `
  -and $repeatabilityStatus -eq "passed_repeatability_partial_coherence" `
  -and $repeatabilityComplete `
  -and $repeatabilityStable

$checks = @(
  (New-GateResult "headed_external_report_exists" ($null -ne $headedReport) $(if ($null -ne $headedReportPath) { $headedReportPath } else { "missing headed_external smoke report" })),
  (New-GateResult "real_binary_task_passed" ($taskStatus -eq "passed") "realBinaryTask.status=$taskStatus"),
  (New-GateResult "validation_probe_signals_present" ($action -eq "validation_probe" -and $signalCount -gt 0) "action=$action signalCount=$signalCount"),
  (New-GateResult "repeatability_object_present" ($null -ne $repeatability) "repeatability.status=$repeatabilityStatus"),
  (New-GateResult "repeatability_partial_coherence" ($repeatabilityStatus -eq "passed_repeatability_partial_coherence") "repeatability.status=$repeatabilityStatus"),
  (New-GateResult "repeatability_attempts_complete" $repeatabilityComplete "requested=$requestedCount executed=$executedCount passed=$passedCount"),
  (New-GateResult "repeatability_shape_stable" $repeatabilityStable "stableSignalCount=$stableSignalCount stableCategories=$stableCategories stableSignalStatuses=$stableSignalStatuses")
)

$status = if ($null -eq $headedReport) {
  "blocked_missing_headed_report"
} elseif ($taskStatus -ne "passed" -or $action -ne "validation_probe" -or $signalCount -eq 0) {
  "blocked_missing_headed_validation_probe"
} elseif ($null -eq $repeatability) {
  "partial_validation_probe_without_repeatability"
} elseif ($repeatabilityPassed) {
  "passed_repeatability_partial_coherence"
} elseif ($repeatabilityStatus -eq "failed_repeatability") {
  "failed_repeatability"
} else {
  "partial_repeatability_or_coherence"
}

$failureReason = if ($status -eq "passed_repeatability_partial_coherence") {
  ""
} elseif ($status -eq "blocked_missing_headed_report") {
  "no headed_external smoke report found"
} elseif ($status -eq "blocked_missing_headed_validation_probe") {
  "latest ranked headed_external report does not contain a passed real binary validation_probe with signals"
} elseif ($status -eq "partial_validation_probe_without_repeatability") {
  "headed_external validation_probe exists but repeatability was not requested or recorded"
} elseif ($status -eq "failed_repeatability") {
  "headed_external repeatability attempts failed"
} else {
  "headed_external repeatability attempts did not produce the same signal/category/status shape"
}

$nextAction = if ($status -eq "passed_repeatability_partial_coherence") {
  "Keep this M10 partial repeatability report attached; continue with long-task stability, remote proxy/TLS, and full 450 observed coverage separately."
} elseif ($status -eq "partial_validation_probe_without_repeatability") {
  "Run scripts/headed_external_smoke.ps1 -Action validation_probe -RepeatValidationProbeCount 2, then rerun this gate."
} elseif ($status -eq "blocked_missing_headed_report" -or $status -eq "blocked_missing_headed_validation_probe") {
  "Run scripts/headed_external_smoke.ps1 -Action validation_probe -RepeatValidationProbeCount 2 against a real CDP-capable browser binary."
} else {
  "Inspect repeatability attempts in the headed_external report, fix unstable signal shape, and rerun the repeatability smoke."
}

$failedChecks = @($checks | Where-Object { $_.status -ne "passed" })
$repeatabilitySummary = if ($null -eq $repeatability) {
  $null
} else {
  [ordered]@{
    status = $repeatabilityStatus
    requestedCount = $requestedCount
    executedCount = $executedCount
    passedCount = $passedCount
    allAttemptsPassed = $allAttemptsPassed
    stableSignalCount = $stableSignalCount
    stableCategories = $stableCategories
    stableSignalStatuses = $stableSignalStatuses
    signalCount = ConvertTo-Int (Get-Field $repeatability "signalCount")
    categories = if ($null -ne (Get-Field $repeatability "categories")) { @((Get-Field $repeatability "categories")) } else { $categories }
    durationStats = Get-Field $repeatability "durationStats"
    failureReason = Get-Field $repeatability "failureReason"
    evidenceBoundary = Get-Field $repeatability "evidenceBoundary"
  }
}

$report = [ordered]@{
  schemaVersion = "m10_headed_repeatability_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  headedExternalReportPath = $headedReportPath
  headedExternalReportStatus = $headedReportStatus
  realBinaryTaskStatus = $taskStatus
  action = $action
  signalCount = $signalCount
  warningCount = $warningCount
  categories = $categories
  repeatability = $repeatabilitySummary
  checks = $checks
  summary = [ordered]@{
    failed = $failedChecks.Count
    requestedCount = $requestedCount
    executedCount = $executedCount
    passedCount = $passedCount
    signalCount = $signalCount
    repeatabilityStatus = if (Test-String $repeatabilityStatus) { $repeatabilityStatus } else { "missing" }
    nextAction = $nextAction
  }
  failureReason = $failureReason
  liveTruthBoundary = @(
    "This gate validates M10 headed_external validation_probe repeatability shape only.",
    "passed_repeatability_partial_coherence is partial runtime realism evidence, not full headed runtime closure.",
    "Long-task stability, remote proxy/TLS proof, provider production closure, cross-machine SessionBundle, AdsPower refresh, and full 450 observed/replay coverage remain outside this gate."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m10-headed-repeatability-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 10 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M10 headed repeatability gate report: $reportPath"
Write-Host "Status: $status"
Write-Host "Headed report: $headedReportPath"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "passed_repeatability_partial_coherence") { exit 0 }
if ($status -eq "failed_repeatability") { exit 1 }
exit 2
