param(
  [string]$HeadedReportsDir = "data/reports/headed-external-smoke",
  [string]$OutputDir = "data/reports/m10-headed-stability",
  [int]$RepeatValidationProbeCount = 3,
  [int]$TimeoutSeconds = 120,
  [switch]$SkipRefresh
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Get-Field([object]$Value, [string]$Name) {
  if ($null -eq $Value) { return $null }
  if ($Value -is [System.Collections.IDictionary]) { return $Value[$Name] }
  $property = $Value.PSObject.Properties[$Name]
  if ($null -eq $property) { return $null }
  return $property.Value
}

function Get-ArrayValue([object]$Value) {
  if ($null -eq $Value) { return @() }
  return @($Value)
}

function Read-JsonFile([string]$Path) {
  return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
}

function ConvertTo-SortKey([object]$Value, [datetime]$Fallback) {
  if ($null -ne $Value) {
    $text = [string]$Value
    $epochMs = 0L
    if ([Int64]::TryParse($text, [ref]$epochMs)) {
      if ($text.Length -le 10) {
        return [DateTimeOffset]::FromUnixTimeSeconds($epochMs).UtcDateTime
      }
      return [DateTimeOffset]::FromUnixTimeMilliseconds($epochMs).UtcDateTime
    }
    try {
      return ([DateTimeOffset]::Parse($text)).UtcDateTime
    } catch {}
  }
  return $Fallback.ToUniversalTime()
}

function Get-ValidationSignals([object]$TaskOrReport) {
  $task = Get-Field $TaskOrReport "realBinaryTask"
  if ($null -eq $task) { $task = $TaskOrReport }
  $result = Get-Field $task "result"
  $signals = Get-Field $result "validation_signals"
  if ($null -eq $signals) { $signals = Get-Field $task "validation_signals" }
  if ($null -eq $signals) { return @() }
  return @(Get-ArrayValue $signals)
}

function Get-SignalSignature([object[]]$Signals, [string]$FieldMode) {
  $items = @()
  foreach ($signal in @(Get-ArrayValue $Signals)) {
    $id = [string](Get-Field $signal "id")
    $category = [string](Get-Field $signal "category")
    $status = [string](Get-Field $signal "status")
    if ($FieldMode -eq "category") {
      if (-not [string]::IsNullOrWhiteSpace($category)) { $items += $category }
    } elseif ($FieldMode -eq "id") {
      if (-not [string]::IsNullOrWhiteSpace($id)) { $items += $id }
    } else {
      if (-not [string]::IsNullOrWhiteSpace($id) -or -not [string]::IsNullOrWhiteSpace($category)) {
        $items += ("{0}|{1}|{2}" -f $id, $category, $status)
      }
    }
  }
  return (@($items | Sort-Object -Unique) -join ";")
}

function Count-SignalStatus([object[]]$Signals, [string]$Status) {
  return @(@($Signals) | Where-Object { [string](Get-Field $_ "status") -eq $Status }).Count
}

function Get-HeadedReportRank([object]$Report) {
  $status = [string](Get-Field $Report "status")
  $task = Get-Field $Report "realBinaryTask"
  $taskStatus = [string](Get-Field $task "status")
  $repeatability = Get-Field $Report "repeatability"
  if ($taskStatus -ne "passed") { return 0 }
  if ($status -eq "passed_real_binary_repeatability" -and $null -ne $repeatability) { return 30 }
  if ($status -eq "passed_real_binary_validation_probe") { return 20 }
  if ($status -eq "passed_real_binary_task") { return 10 }
  return 0
}

function Read-HeadedReports([string]$Directory) {
  if (-not (Test-Path $Directory)) { return @() }
  $items = @()
  Get-ChildItem -LiteralPath $Directory -Filter *.json -File -ErrorAction SilentlyContinue | ForEach-Object {
    try {
      $report = Read-JsonFile $_.FullName
      $items += [pscustomobject]@{
        path = $_.FullName
        report = $report
        sortKey = ConvertTo-SortKey (Get-Field $report "generatedAt") $_.LastWriteTimeUtc
        rank = Get-HeadedReportRank $report
      }
    } catch {}
  }
  return @($items | Sort-Object -Property @{Expression = { $_.rank }; Descending = $true }, @{Expression = { $_.sortKey }; Descending = $true })
}

function Read-AttemptSignals([object]$Attempt, [object]$FallbackReport) {
  $reportPath = [string](Get-Field $Attempt "reportPath")
  if (-not [string]::IsNullOrWhiteSpace($reportPath) -and (Test-Path $reportPath)) {
    try {
      return Get-ValidationSignals (Read-JsonFile $reportPath)
    } catch {}
  }
  return Get-ValidationSignals $FallbackReport
}

function New-GateResult([string]$Id, [bool]$Passed, [string]$Evidence) {
  return [ordered]@{
    id = $Id
    status = if ($Passed) { "passed" } else { "failed" }
    evidence = $Evidence
  }
}

$projectRoot = Resolve-ProjectRoot
$absoluteHeadedReportsDir = Join-Path $projectRoot $HeadedReportsDir
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$refresh = [ordered]@{
  skipped = [bool]$SkipRefresh
  command = ""
  exitCode = $null
  outputTail = @()
}

if (-not $SkipRefresh) {
  $smokePath = Join-Path $projectRoot "scripts\headed_external_smoke.ps1"
  $refresh.command = "powershell -ExecutionPolicy Bypass -File scripts\headed_external_smoke.ps1 -Action validation_probe -RepeatValidationProbeCount $RepeatValidationProbeCount -TimeoutSeconds $TimeoutSeconds"
  Push-Location $projectRoot
  try {
    $output = & powershell -ExecutionPolicy Bypass -File $smokePath -Action validation_probe -RepeatValidationProbeCount $RepeatValidationProbeCount -TimeoutSeconds $TimeoutSeconds 2>&1
    $refresh.exitCode = $LASTEXITCODE
    $refresh.outputTail = @($output | Select-Object -Last 30)
  } finally {
    Pop-Location
  }
}

$items = Read-HeadedReports $absoluteHeadedReportsDir
$best = if ($items.Count -gt 0) { $items[0] } else { $null }
$report = if ($null -ne $best) { $best.report } else { $null }
$repeatability = Get-Field $report "repeatability"
$attempts = @(Get-ArrayValue (Get-Field $repeatability "attempts"))
$matrix = @()
foreach ($attempt in $attempts) {
  $signals = Read-AttemptSignals $attempt $report
  $categories = @((Get-SignalSignature $signals "category").Split(";") | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
  $signalIds = @((Get-SignalSignature $signals "id").Split(";") | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
  $matrix += [ordered]@{
    attempt = [int](Get-Field $attempt "attempt")
    status = [string](Get-Field $attempt "status")
    action = [string](Get-Field $attempt "action")
    durationMs = Get-Field $attempt "durationMs"
    finalUrl = Get-Field $attempt "finalUrl"
    signalCount = @($signals).Count
    warningCount = Count-SignalStatus $signals "warning"
    failedSignalCount = Count-SignalStatus $signals "failed"
    categories = $categories
    signalIds = $signalIds
    statusSignature = Get-SignalSignature $signals "status"
    reportPath = Get-Field $attempt "reportPath"
    failureReason = Get-Field $attempt "failureReason"
  }
}

$executedCount = $matrix.Count
$passedAttempts = @($matrix | Where-Object { $_.status -eq "passed" }).Count
$allPassed = $executedCount -ge $RepeatValidationProbeCount -and $passedAttempts -eq $executedCount
$categorySignatures = @($matrix | ForEach-Object { (@($_.categories) -join "|") } | Sort-Object -Unique)
$signalIdSignatures = @($matrix | ForEach-Object { (@($_.signalIds) -join "|") } | Sort-Object -Unique)
$statusSignatures = @($matrix | ForEach-Object { [string]$_.statusSignature } | Sort-Object -Unique)
$warningCounts = @($matrix | ForEach-Object { [int]$_.warningCount } | Sort-Object -Unique)
$failedCounts = @($matrix | ForEach-Object { [int]$_.failedSignalCount } | Sort-Object -Unique)
$durations = @($matrix | ForEach-Object { if ($null -ne $_.durationMs) { [double]$_.durationMs } })
$durationStats = if ($durations.Count -gt 0) {
  [ordered]@{
    minMs = ($durations | Measure-Object -Minimum).Minimum
    maxMs = ($durations | Measure-Object -Maximum).Maximum
    averageMs = [math]::Round(($durations | Measure-Object -Average).Average, 2)
    spreadMs = (($durations | Measure-Object -Maximum).Maximum - ($durations | Measure-Object -Minimum).Minimum)
  }
} else {
  [ordered]@{ minMs = $null; maxMs = $null; averageMs = $null; spreadMs = $null }
}

$stableCategories = $executedCount -gt 0 -and $categorySignatures.Count -eq 1
$stableSignalIds = $executedCount -gt 0 -and $signalIdSignatures.Count -eq 1
$stableStatuses = $executedCount -gt 0 -and $statusSignatures.Count -eq 1
$stableWarnings = $executedCount -gt 0 -and $warningCounts.Count -eq 1
$stableFailures = $executedCount -gt 0 -and $failedCounts.Count -eq 1
$coherenceParts = @($stableCategories, $stableSignalIds, $stableStatuses, $stableWarnings, $stableFailures)
$coherenceScore = [math]::Round((@($coherenceParts | Where-Object { $_ -eq $true }).Count / [double]$coherenceParts.Count), 4)
$passRatio = if ($executedCount -gt 0) { $passedAttempts / [double]$executedCount } else { 0 }
$durationSpreadOk = $durations.Count -gt 1 -and $durationStats.spreadMs -le ($TimeoutSeconds * 1000)
$stabilityScore = [math]::Round((($passRatio * 0.7) + ($(if ($durationSpreadOk) { 1 } else { 0 }) * 0.3)), 4)

$status = if ($null -eq $report) {
  "blocked_missing_headed_report"
} elseif ($executedCount -lt $RepeatValidationProbeCount) {
  "partial_long_task_stability_matrix"
} elseif ($allPassed -and $coherenceScore -eq 1 -and $stabilityScore -ge 0.95) {
  "passed_long_task_stability_coherence"
} elseif ($passedAttempts -gt 0) {
  "partial_long_task_stability_or_coherence"
} else {
  "failed_long_task_stability"
}

$checks = @(
  (New-GateResult "headed_external_report_exists" ($null -ne $report) $(if ($null -ne $best) { $best.path } else { "missing" })),
  (New-GateResult "attempt_matrix_complete" ($executedCount -ge $RepeatValidationProbeCount) "executed=$executedCount requested=$RepeatValidationProbeCount"),
  (New-GateResult "attempts_passed" $allPassed "passed=$passedAttempts executed=$executedCount"),
  (New-GateResult "signal_categories_stable" $stableCategories "uniqueCategorySignatures=$($categorySignatures.Count)"),
  (New-GateResult "signal_ids_stable" $stableSignalIds "uniqueSignalIdSignatures=$($signalIdSignatures.Count)"),
  (New-GateResult "signal_statuses_stable" $stableStatuses "uniqueStatusSignatures=$($statusSignatures.Count)"),
  (New-GateResult "warning_failure_counts_stable" ($stableWarnings -and $stableFailures) "uniqueWarningCounts=$($warningCounts.Count) uniqueFailedCounts=$($failedCounts.Count)")
)

$failureReason = if ($status -eq "passed_long_task_stability_coherence") {
  ""
} elseif ($status -eq "blocked_missing_headed_report") {
  "no headed_external report exists"
} elseif ($status -eq "partial_long_task_stability_matrix") {
  "headed_external attempts were fewer than requested"
} elseif ($status -eq "partial_long_task_stability_or_coherence") {
  "headed_external attempts passed partially or produced unstable signal/category/status matrix"
} else {
  "headed_external long-task stability attempts failed"
}

$nextAction = if ($status -eq "passed_long_task_stability_coherence") {
  "Keep this M10 stability matrix attached; continue with remote proxy/TLS and product replay wiring as separate evidence."
} else {
  "Inspect the attempt matrix, fix headed_external instability, then rerun scripts/m10_headed_stability_gate.ps1."
}

$reportOut = [ordered]@{
  schemaVersion = "m10_headed_stability_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  refresh = $refresh
  headedExternalReportPath = if ($null -ne $best) { $best.path } else { $null }
  headedExternalReportStatus = [string](Get-Field $report "status")
  requestedCount = $RepeatValidationProbeCount
  executedCount = $executedCount
  passedCount = $passedAttempts
  coherence = [ordered]@{
    score = $coherenceScore
    stableCategories = $stableCategories
    stableSignalIds = $stableSignalIds
    stableSignalStatuses = $stableStatuses
    stableWarningCounts = $stableWarnings
    stableFailedCounts = $stableFailures
  }
  stability = [ordered]@{
    score = $stabilityScore
    passRatio = [math]::Round($passRatio, 4)
    durationSpreadOk = $durationSpreadOk
    durationStats = $durationStats
  }
  matrix = $matrix
  checks = $checks
  summary = [ordered]@{
    failed = @($checks | Where-Object { $_.status -ne "passed" }).Count
    requestedCount = $RepeatValidationProbeCount
    executedCount = $executedCount
    passedCount = $passedAttempts
    coherenceScore = $coherenceScore
    stabilityScore = $stabilityScore
    signalCount = if ($matrix.Count -gt 0) { [int]$matrix[0].signalCount } else { 0 }
    categories = if ($matrix.Count -gt 0) { @($matrix[0].categories) } else { @() }
    nextAction = $nextAction
  }
  failureReason = $failureReason
  liveTruthBoundary = @(
    "This gate validates local headed_external long-task repeatability and coherence matrix only.",
    "It does not prove provider production closure, remote proxy/TLS, AdsPower refresh, or full 450 observed/replay coverage.",
    "The refresh command may start real local browser processes through scripts/headed_external_smoke.ps1."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m10-headed-stability-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$reportOut | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding UTF8

Write-Host "M10 headed stability gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "passed_long_task_stability_coherence") { exit 0 }
if ($status -eq "blocked_missing_headed_report") { exit 2 }
exit 1
