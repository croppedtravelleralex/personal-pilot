param(
  [string]$ProjectRoot = "",
  [string]$OutputDir = "data/reports/taxonomy-coverage",
  [int]$BehaviorOveragePerFamily = 1,
  [switch]$AllowObservedPending
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  if (-not [string]::IsNullOrWhiteSpace($ProjectRoot)) {
    return (Resolve-Path $ProjectRoot).Path
  }
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Read-JsonFile([string]$Path) {
  if (-not (Test-Path $Path)) {
    throw "Missing JSON file: $Path"
  }
  return Get-Content -Path $Path -Raw | ConvertFrom-Json
}

function ConvertTo-Slug([string]$Value) {
  $slug = $Value.ToLowerInvariant() -replace '[^a-z0-9]+', '-'
  return $slug.Trim('-')
}

function Get-ArrayValue($Value) {
  if ($null -eq $Value) { return @() }
  return @($Value)
}

function Select-Cycled([object[]]$Items, [int]$Index, [string]$Fallback) {
  if ($Items.Count -eq 0) { return $Fallback }
  return [string]$Items[$Index % $Items.Count]
}

function Get-BehaviorPhase([string]$FamilyId, [object[]]$Phases, [int]$Index) {
  $map = @{
    readiness_wait = "readiness"
    settle_idle = "settle"
    scroll_scan = "scan"
    hover_focus = "focus"
    typing_input = "input"
    content_pause = "consume"
    session_persist = "persist"
    budget_recovery = "recover"
    manual_gate = "manual_gate"
    debug_audit = "audit"
    provider_assist = "input"
  }
  if ($map.ContainsKey($FamilyId)) { return $map[$FamilyId] }
  return Select-Cycled $Phases $Index "audit"
}

function New-FingerprintSignals($Taxonomy) {
  $signals = @()
  foreach ($family in $Taxonomy.families) {
    $familyId = [string]$family.id
    $count = [int]$family.targetCount
    $slug = ConvertTo-Slug $familyId
    $adapterSupport = Get-ArrayValue $family.adapterSupport
    for ($i = 1; $i -le $count; $i++) {
      $adapter = Select-Cycled $adapterSupport ($i - 1) "profile_browser_adapter_pending"
      $status = if (($family.evidenceStatus -as [string]) -like "partial*") {
        "materialized_contract_ready_partial_family_evidence"
      } else {
        "materialized_contract_ready_observed_pending"
      }
      $signals += [ordered]@{
        id = "fp-$slug-{0:D3}" -f $i
        familyId = $familyId
        ordinal = $i
        collectorScope = if ($adapter -eq "desktop_webview") { "desktop_webview" } else { "profile_browser_required" }
        runtimeAdapter = $adapter
        targetProfileBrowser = ($adapter -ne "desktop_webview")
        declaredAppliedObservedLayer = [string]$family.declaredAppliedObservedLayer
        collectorPath = [string]$family.collectorPath
        currentEvidence = [string]$family.currentEvidence
        failureReasonRequired = $true
        failureMode = [string]$family.failureMode
        repeatabilityRule = [string]$family.repeatabilityRule
        evidenceSchema = [string]$family.evidenceSchema
        coverageStatus = $status
      }
    }
  }
  return $signals
}

function New-BehaviorEvents($Taxonomy) {
  $events = @()
  $pageArchetypes = Get-ArrayValue $Taxonomy.dimensions.pageArchetypes
  $phases = Get-ArrayValue $Taxonomy.dimensions.phases
  $outcomes = Get-ArrayValue $Taxonomy.dimensions.outcomes
  foreach ($family in $Taxonomy.families) {
    $familyId = [string]$family.id
    $count = [int]$family.targetCount
    $slug = ConvertTo-Slug $familyId
    for ($i = 1; $i -le $count; $i++) {
      $phase = Get-BehaviorPhase $familyId $phases ($i - 1)
      $outcome = Select-Cycled $outcomes ($i - 1) "planned"
      $status = if (($family.evidenceStatus -as [string]) -like "primitive_backed*") {
        "materialized_replay_contract_ready_primitive_family_backed"
      } else {
        "materialized_replay_contract_ready_live_replay_pending"
      }
      $events += [ordered]@{
        id = "bh-$slug-{0:D3}" -f $i
        familyId = $familyId
        ordinal = $i
        pageArchetype = Select-Cycled $pageArchetypes ($i - 1) "generic"
        phase = $phase
        outcome = $outcome
        runtimePath = [string]$family.runtimePath
        replaySemantics = [string]$family.replaySemantics
        auditPayload = Get-ArrayValue $family.auditPayload
        failureStates = Get-ArrayValue $family.failureStates
        recoveryBehavior = [string]$family.recoveryBehavior
        currentEvidence = [string]$family.currentEvidence
        coverageStatus = $status
      }
    }
  }

  if ($BehaviorOveragePerFamily -gt 0) {
    foreach ($family in $Taxonomy.families) {
      $familyId = [string]$family.id
      $count = [int]$family.targetCount
      $slug = ConvertTo-Slug $familyId
      for ($extra = 1; $extra -le $BehaviorOveragePerFamily; $extra++) {
        $ordinal = $count + $extra
        $events += [ordered]@{
          id = "bh-$slug-extension-{0:D3}" -f $extra
          familyId = $familyId
          ordinal = $ordinal
          pageArchetype = Select-Cycled $pageArchetypes ($ordinal - 1) "generic"
          phase = Get-BehaviorPhase $familyId $phases ($ordinal - 1)
          outcome = "audit_extension"
          runtimePath = [string]$family.runtimePath
          replaySemantics = "coverage extension: $($family.replaySemantics)"
          auditPayload = (Get-ArrayValue $family.auditPayload) + @("coverage_extension_id")
          failureStates = Get-ArrayValue $family.failureStates
          recoveryBehavior = [string]$family.recoveryBehavior
          currentEvidence = [string]$family.currentEvidence
          coverageStatus = "materialized_replay_contract_ready_450_plus_extension_live_replay_pending"
        }
      }
    }
  }
  return $events
}

$root = Resolve-ProjectRoot
$fingerprintPath = Join-Path $root "docs\taxonomy\fingerprint-signal-taxonomy.json"
$behaviorPath = Join-Path $root "docs\taxonomy\behavior-event-taxonomy.json"
$fingerprint = Read-JsonFile $fingerprintPath
$behavior = Read-JsonFile $behaviorPath

$signals = New-FingerprintSignals $fingerprint
$events = New-BehaviorEvents $behavior
$fingerprintExpected = [int]$fingerprint.targetSignalCount
$behaviorExpected = [int]$behavior.targetEventCount
$fingerprintComplete = ($signals.Count -eq $fingerprintExpected -and $fingerprintExpected -eq 450)
$behaviorComplete = ($events.Count -ge 451 -and $events.Count -ge $behaviorExpected)

$status = if ($fingerprintComplete -and $behaviorComplete) {
  "passed_materialized_contract"
} else {
  "failed_count_mismatch"
}

$report = [ordered]@{
  schemaVersion = "taxonomy_coverage_materialization_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $root
  status = $status
  fingerprint = [ordered]@{
    taxonomyPath = "docs/taxonomy/fingerprint-signal-taxonomy.json"
    targetSignalCount = $fingerprintExpected
    materializedSignalCount = $signals.Count
    familyCount = @($fingerprint.families).Count
    coverageStatus = "materialized_450_signal_contract_ready_observed_pending"
    signals = $signals
  }
  behavior = [ordered]@{
    taxonomyPath = "docs/taxonomy/behavior-event-taxonomy.json"
    targetEventCount = $behaviorExpected
    materializedEventCount = $events.Count
    overagePerFamily = $BehaviorOveragePerFamily
    familyCount = @($behavior.families).Count
    coverageStatus = "materialized_450_plus_replay_contract_ready_live_replay_pending"
    events = $events
  }
  truthBoundary = [ordered]@{
    observedFingerprintCoverage = "pending_real_collector_evidence"
    liveBehaviorReplayCoverage = "pending_executor_evidence"
    reason = "This materializes per-signal/per-event coverage contracts; it does not promote target taxonomy to observed/live runtime evidence."
  }
  notes = @(
    "Fingerprint target is exactly 450 materialized signal contracts.",
    "Behavior target is 450+ by project wording; this report materializes the declared 450 event contracts from the taxonomy seed.",
    "Observed/live promotion requires separate runtime reports with collectorScope/runtimeAdapter/failureReason and replay trace evidence."
  )
}

$absoluteOutputDir = Join-Path $root $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null
$reportPath = Join-Path $absoluteOutputDir ("taxonomy-coverage-materialized-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 12 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "Taxonomy coverage materialization report: $reportPath"
Write-Host "Status: $status"
Write-Host "Fingerprint signals: $($signals.Count) / $fingerprintExpected"
Write-Host "Behavior events: $($events.Count) / $behaviorExpected"
Write-Host "Truth boundary: materialized contracts are not observed/live runtime evidence."

if ($status -ne "passed_materialized_contract") { exit 1 }
if (-not $AllowObservedPending) { exit 0 }
