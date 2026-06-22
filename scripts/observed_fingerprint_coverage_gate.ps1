param(
  [string]$OutputDir = "data/reports/observed-fingerprint-coverage"
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

function Test-FieldExists([object]$Value, [string]$Name) {
  if ($null -eq $Value) { return $false }
  if ($Value -is [System.Collections.IDictionary]) { return $Value.Contains($Name) }
  return $null -ne $Value.PSObject.Properties[$Name]
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

function Get-ReportSignals([object]$Report) {
  $signals = @()
  $topSignals = Get-Field $Report "signals"
  if ($null -ne $topSignals) { $signals += @(Get-ArrayValue $topSignals) }

  $task = Get-Field $Report "realBinaryTask"
  $result = Get-Field $task "result"
  $taskSignals = Get-Field $task "validation_signals"
  $resultSignals = Get-Field $result "validation_signals"
  if ($null -ne $taskSignals) { $signals += @(Get-ArrayValue $taskSignals) }
  if ($null -ne $resultSignals) { $signals += @(Get-ArrayValue $resultSignals) }
  return $signals
}

function Resolve-FingerprintFamilyId([object]$Signal) {
  $id = [string](Get-Field $Signal "id")
  $category = [string](Get-Field $Signal "category")
  $detail = [string](Get-Field $Signal "detail")
  $label = [string](Get-Field $Signal "label")
  $summary = [string](Get-Field $Signal "summary")
  $text = ("$id $category $detail $label $summary").ToLowerInvariant()

  if ($text -match "webgl|gpu|renderer") { return "webgl_gpu" }
  if ($text -match "canvas") { return "canvas_rendering" }
  if ($text -match "audio|audiocontext") { return "audio_stack" }
  if ($text -match "webrtc|ice candidate|rtcpeerconnection") { return "webrtc_ip_leak" }
  if ($text -match "font|text metrics|documentfonts") { return "fonts_text_metrics" }
  if ($text -match "media device|mediadevices|getusermedia") { return "media_devices" }
  if ($text -match "storage|cookie|leak|localstorage|sessionstorage") { return "storage_partitioning" }
  if ($text -match "timezone|locale|intl|language") { return "timezone_locale" }
  if ($text -match "dns|transport|https|network") { return "network_transport" }
  if ($text -match "hardware|platform|useragent|navigator|device|memory|cpu|screen|os") { return "hardware_os" }
  if ($text -match "detector|coherence|consistency") { return "coherence_detector" }
  return "browser_api_surface"
}

function Test-ObservedSignalMetadataComplete([object]$Signal) {
  $hasFailureReason = Test-FieldExists $Signal "failureReason"
  $hasTargetProfileBrowser = Test-FieldExists $Signal "targetProfileBrowser"
  return -not [string]::IsNullOrWhiteSpace([string](Get-Field $Signal "id")) `
    -and -not [string]::IsNullOrWhiteSpace([string](Get-Field $Signal "category")) `
    -and -not [string]::IsNullOrWhiteSpace([string](Get-Field $Signal "collectorScope")) `
    -and -not [string]::IsNullOrWhiteSpace([string](Get-Field $Signal "runtimeAdapter")) `
    -and $hasTargetProfileBrowser `
    -and $hasFailureReason
}

function New-GateResult([string]$Id, [bool]$Passed, [string]$Evidence) {
  return [ordered]@{
    id = $Id
    status = if ($Passed) { "passed" } else { "failed" }
    evidence = $Evidence
  }
}

$projectRoot = Resolve-ProjectRoot
$taxonomyPath = Join-Path $projectRoot "docs\taxonomy\fingerprint-signal-taxonomy.json"
$taxonomy = Read-JsonFile $taxonomyPath
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$sourceSpecs = @(
  [ordered]@{ kind = "validation"; dir = "data\reports\validation"; pattern = "*.json" },
  [ordered]@{ kind = "legacy_validation"; dir = "data\validation-reports"; pattern = "*.json" },
  [ordered]@{ kind = "profile_browser_environment"; dir = "data\reports\profile-browser-environment"; pattern = "*.json" },
  [ordered]@{ kind = "headed_external"; dir = "data\reports\headed-external-smoke"; pattern = "*.json" }
)

$reportItems = @()
foreach ($spec in $sourceSpecs) {
  $dir = Join-Path $projectRoot $spec.dir
  if (-not (Test-Path $dir)) { continue }
  Get-ChildItem -LiteralPath $dir -Filter $spec.pattern -File -ErrorAction SilentlyContinue | ForEach-Object {
    try {
      $report = Read-JsonFile $_.FullName
      $reportItems += [pscustomobject]@{
        sourceKind = $spec.kind
        path = $_.FullName
        sortKey = ConvertTo-SortKey (Get-Field $report "generatedAt") $_.LastWriteTimeUtc
        report = $report
      }
    } catch {}
  }
}

$observedByKey = @{}
$excluded = @()
foreach ($item in @($reportItems | Sort-Object sortKey -Descending)) {
  foreach ($signal in @(Get-ReportSignals $item.report)) {
    $layer = [string](Get-Field $signal "layer")
    $metadataComplete = Test-ObservedSignalMetadataComplete $signal
    $id = [string](Get-Field $signal "id")
    if ($layer -ne "observed" -or -not $metadataComplete) {
      $excluded += [ordered]@{
        id = if ([string]::IsNullOrWhiteSpace($id)) { "missing-id" } else { $id }
        sourceKind = $item.sourceKind
        reason = if ($layer -ne "observed") { "missing_observed_layer" } else { "metadata_incomplete" }
        layer = $layer
        metadataComplete = $metadataComplete
        reportPath = $item.path
      }
      continue
    }

    $familyId = Resolve-FingerprintFamilyId $signal
    $key = "{0}|{1}|{2}|{3}" -f $id, $familyId, [string](Get-Field $signal "collectorScope"), [string](Get-Field $signal "runtimeAdapter")
    if (-not $observedByKey.ContainsKey($key)) {
      $observedByKey[$key] = [ordered]@{
        id = $id
        familyId = $familyId
        category = [string](Get-Field $signal "category")
        status = [string](Get-Field $signal "status")
        collectorScope = [string](Get-Field $signal "collectorScope")
        runtimeAdapter = [string](Get-Field $signal "runtimeAdapter")
        targetProfileBrowser = [bool](Get-Field $signal "targetProfileBrowser")
        failureReason = Get-Field $signal "failureReason"
        reportPath = $item.path
      }
    }
  }
}

$observedSignals = @($observedByKey.Values)
$familyCoverage = @()
foreach ($family in @($taxonomy.families)) {
  $familyId = [string]$family.id
  $targetCount = [int]$family.targetCount
  $familySignals = @($observedSignals | Where-Object { $_.familyId -eq $familyId })
  $successCount = @($familySignals | Where-Object { $_.status -eq "succeeded" }).Count
  $warningCount = @($familySignals | Where-Object { $_.status -eq "warning" }).Count
  $failedCount = @($familySignals | Where-Object { $_.status -eq "failed" }).Count
  $adapters = @($familySignals | ForEach-Object { $_.runtimeAdapter } | Sort-Object -Unique)
  $scopes = @($familySignals | ForEach-Object { $_.collectorScope } | Sort-Object -Unique)
  $profileBrowserObserved = @($familySignals | Where-Object { $_.targetProfileBrowser -eq $true }).Count -gt 0
  $familyCoverage += [ordered]@{
    familyId = $familyId
    targetCount = $targetCount
    observedSignalCount = $familySignals.Count
    successCount = $successCount
    warningCount = $warningCount
    failedCount = $failedCount
    profileBrowserObserved = $profileBrowserObserved
    desktopWebViewObserved = @($familySignals | Where-Object { $_.collectorScope -match "desktop" }).Count -gt 0
    runtimeAdapters = $adapters
    collectorScopes = $scopes
    coverageRatio = if ($targetCount -gt 0) { [math]::Round($familySignals.Count / $targetCount, 4) } else { 0 }
    status = if ($familySignals.Count -ge $targetCount) { "covered" } elseif ($familySignals.Count -gt 0) { "partial" } else { "missing" }
  }
}

$targetTotal = [int]$taxonomy.targetSignalCount
$familyTargetTotal = 0
foreach ($family in @($taxonomy.families)) { $familyTargetTotal += [int]$family.targetCount }
$fullCoverage = $observedSignals.Count -ge $targetTotal -and @($familyCoverage | Where-Object { $_.status -ne "covered" }).Count -eq 0
$status = if ($observedSignals.Count -eq 0) {
  "blocked_missing_observed_fingerprint_reports"
} elseif ($fullCoverage) {
  "passed_full_observed_fingerprint_coverage"
} else {
  "partial_observed_fingerprint_coverage"
}

$checks = @(
  (New-GateResult "taxonomy_loaded" ($targetTotal -eq 450 -and $familyTargetTotal -eq 450) "target=$targetTotal familyTargetTotal=$familyTargetTotal"),
  (New-GateResult "observed_signal_metadata_strict" ($observedSignals.Count -gt 0) "observedLayerMetadataCompleteSignals=$($observedSignals.Count)"),
  (New-GateResult "materialized_contracts_excluded" $true "taxonomy/materialized contracts are not read as observed signal sources"),
  (New-GateResult "full_family_targets_covered" $fullCoverage "coveredFamilies=$(@($familyCoverage | Where-Object { $_.status -eq 'covered' }).Count)/$(@($familyCoverage).Count)")
)

$failureReason = if ($status -eq "passed_full_observed_fingerprint_coverage") {
  ""
} elseif ($status -eq "blocked_missing_observed_fingerprint_reports") {
  "no validation report contained layer=observed signals with complete collector metadata"
} else {
  "observed signal coverage is partial; taxonomy/materialized contracts were intentionally excluded"
}

$nextAction = if ($status -eq "passed_full_observed_fingerprint_coverage") {
  "Keep this full observed coverage report attached and rerun after changing validation collectors."
} elseif ($status -eq "blocked_missing_observed_fingerprint_reports") {
  "Run a real profile-browser validation probe that emits layer=observed signals with collectorScope/runtimeAdapter/targetProfileBrowser/failureReason metadata."
} else {
  "Expand real collectors until every fingerprint taxonomy family reaches its target count with layer=observed metadata; do not count taxonomy seeds or materialized contracts."
}

$report = [ordered]@{
  schemaVersion = "observed_fingerprint_coverage_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  taxonomyPath = "docs/taxonomy/fingerprint-signal-taxonomy.json"
  targetSignalCount = $targetTotal
  observedSignalCount = $observedSignals.Count
  familyCount = @($familyCoverage).Count
  coveredFamilyCount = @($familyCoverage | Where-Object { $_.status -eq "covered" }).Count
  partialFamilyCount = @($familyCoverage | Where-Object { $_.status -eq "partial" }).Count
  missingFamilyCount = @($familyCoverage | Where-Object { $_.status -eq "missing" }).Count
  familyCoverage = $familyCoverage
  observedSignals = $observedSignals
  excludedSignalCount = @($excluded).Count
  excludedSignalsSample = @($excluded | Select-Object -First 25)
  checks = $checks
  summary = [ordered]@{
    failed = @($checks | Where-Object { $_.status -ne "passed" -and $_.id -ne "full_family_targets_covered" }).Count
    observedSignalCount = $observedSignals.Count
    targetSignalCount = $targetTotal
    coveredFamilyCount = @($familyCoverage | Where-Object { $_.status -eq "covered" }).Count
    partialFamilyCount = @($familyCoverage | Where-Object { $_.status -eq "partial" }).Count
    missingFamilyCount = @($familyCoverage | Where-Object { $_.status -eq "missing" }).Count
    fullCoverage = $fullCoverage
    nextAction = $nextAction
  }
  failureReason = $failureReason
  liveTruthBoundary = @(
    "This report counts only signals with layer=observed and complete collector metadata.",
    "Taxonomy seed and taxonomy materialization reports are intentionally excluded.",
    "Partial coverage is valid evidence of current state, not a claim of full 450 observed coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("observed-fingerprint-coverage-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding UTF8

Write-Host "Observed fingerprint coverage report: $reportPath"
Write-Host "Status: $status"
Write-Host "Observed signals: $($observedSignals.Count) / $targetTotal"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "blocked_missing_observed_fingerprint_reports") { exit 2 }
exit 0
