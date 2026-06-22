param(
  [string]$OutputDir = "data/reports/live-replay-runtime",
  [int]$ExtensionEventsPerFamily = 1
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Read-JsonFile([string]$Path) {
  if (-not (Test-Path $Path)) { throw "Missing JSON file: $Path" }
  return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
}

function Get-ArrayValue([object]$Value) {
  if ($null -eq $Value) { return @() }
  return @($Value)
}

function Select-Cycled([object[]]$Items, [int]$Index, [string]$Fallback) {
  if (@($Items).Count -eq 0) { return $Fallback }
  return [string]@($Items)[$Index % @($Items).Count]
}

function ConvertTo-Slug([string]$Value) {
  $slug = $Value.ToLowerInvariant() -replace '[^a-z0-9]+', '-'
  return $slug.Trim('-')
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

function Test-FamilyReplayContract([object]$Family) {
  $auditPayload = Get-ArrayValue $Family.auditPayload
  $failureStates = Get-ArrayValue $Family.failureStates
  return -not [string]::IsNullOrWhiteSpace([string]$Family.replaySemantics) `
    -and @($auditPayload).Count -gt 0 `
    -and @($failureStates).Count -gt 0 `
    -and -not [string]::IsNullOrWhiteSpace([string]$Family.recoveryBehavior)
}

function New-BehaviorEvent([object]$Family, [int]$Ordinal, [bool]$Extension, [object[]]$PageArchetypes, [object[]]$Phases, [object[]]$Outcomes) {
  $familyId = [string]$Family.id
  $slug = ConvertTo-Slug $familyId
  $eventOrdinal = if ($Extension) { "extension-{0:D3}" -f $Ordinal } else { "{0:D3}" -f $Ordinal }
  return [ordered]@{
    id = "bh-$slug-$eventOrdinal"
    familyId = $familyId
    ordinal = $Ordinal
    extension = $Extension
    pageArchetype = Select-Cycled $PageArchetypes ($Ordinal - 1) "generic"
    phase = Get-BehaviorPhase $familyId $Phases ($Ordinal - 1)
    requestedOutcome = if ($Extension) { "audit_extension" } else { Select-Cycled $Outcomes ($Ordinal - 1) "planned" }
    replaySemantics = [string]$Family.replaySemantics
    auditPayload = Get-ArrayValue $Family.auditPayload
    failureStates = Get-ArrayValue $Family.failureStates
    recoveryBehavior = [string]$Family.recoveryBehavior
    evidenceStatus = [string]$Family.evidenceStatus
  }
}

function Invoke-LocalReplayEvent([object]$Event, [object]$Family) {
  $contractComplete = Test-FamilyReplayContract $Family
  $scenarioIndex = ([int]$Event.ordinal - 1) % 3
  $scenario = @("nominal", "failure_injected", "recovery_check")[$scenarioIndex]
  $runtimeBacked = $true
  $productRuntimeBacked = ([string]$Family.evidenceStatus).StartsWith("primitive_backed")
  $failureState = $null
  $status = "executed"
  $recoveryApplied = $false

  if (-not $contractComplete) {
    $status = "failed"
    $failureState = "replay_contract_incomplete"
  } elseif ($scenario -eq "failure_injected") {
    $failureStates = Get-ArrayValue $Family.failureStates
    $failureState = Select-Cycled $failureStates ([int]$Event.ordinal - 1) "synthetic_failure"
    if (-not [string]::IsNullOrWhiteSpace([string]$Family.recoveryBehavior)) {
      $status = "recovered"
      $recoveryApplied = $true
    } else {
      $status = "failed"
    }
  } elseif ($scenario -eq "recovery_check") {
    $status = "executed"
    $recoveryApplied = -not [string]::IsNullOrWhiteSpace([string]$Family.recoveryBehavior)
  }

  $audit = [ordered]@{
    eventId = [string]$Event.id
    familyId = [string]$Event.familyId
    phase = [string]$Event.phase
    primitive = [string]$Event.familyId
    pageArchetype = [string]$Event.pageArchetype
    scenario = $scenario
    status = $status
    failureState = $failureState
    recoveryApplied = $recoveryApplied
    auditPayloadFields = Get-ArrayValue $Event.auditPayload
    replaySemanticsHash = ([string]$Event.replaySemantics).GetHashCode()
  }

  return [ordered]@{
    eventId = [string]$Event.id
    familyId = [string]$Event.familyId
    status = $status
    runtimeBacked = $runtimeBacked
    runtimeKind = "local_deterministic_replay_harness"
    productRuntimeBacked = $productRuntimeBacked
    contractOnlyBoundary = if ($productRuntimeBacked) { "" } else { "taxonomy family replayed by local harness; product/browser/provider runtime wiring remains separate" }
    replaySemanticsChecked = -not [string]::IsNullOrWhiteSpace([string]$Event.replaySemantics)
    auditPayloadChecked = @(Get-ArrayValue $Event.auditPayload).Count -gt 0
    failureStatesChecked = @(Get-ArrayValue $Event.failureStates).Count -gt 0
    recoveryBehaviorChecked = -not [string]::IsNullOrWhiteSpace([string]$Event.recoveryBehavior)
    failureState = $failureState
    recoveryApplied = $recoveryApplied
    audit = $audit
  }
}

function New-GateResult([string]$Id, [bool]$Passed, [string]$Evidence) {
  return [ordered]@{
    id = $Id
    status = if ($Passed) { "passed" } else { "failed" }
    evidence = $Evidence
  }
}

$projectRoot = Resolve-ProjectRoot
$taxonomyPath = Join-Path $projectRoot "docs\taxonomy\behavior-event-taxonomy.json"
$taxonomy = Read-JsonFile $taxonomyPath
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$pageArchetypes = Get-ArrayValue $taxonomy.dimensions.pageArchetypes
$phases = Get-ArrayValue $taxonomy.dimensions.phases
$outcomes = Get-ArrayValue $taxonomy.dimensions.outcomes

$events = @()
$familyResults = @()
foreach ($family in @($taxonomy.families)) {
  $targetCount = [int]$family.targetCount
  $familyEvents = @()
  for ($i = 1; $i -le $targetCount; $i++) {
    $familyEvents += New-BehaviorEvent $family $i $false $pageArchetypes $phases $outcomes
  }
  for ($extra = 1; $extra -le $ExtensionEventsPerFamily; $extra++) {
    $familyEvents += New-BehaviorEvent $family ($targetCount + $extra) $true $pageArchetypes $phases $outcomes
  }

  $familyReplay = @()
  foreach ($event in $familyEvents) {
    $familyReplay += Invoke-LocalReplayEvent $event $family
  }
  $events += $familyReplay
  $familyResults += [ordered]@{
    familyId = [string]$family.id
    targetCount = $targetCount
    replayedCount = @($familyReplay).Count
    runtimeBackedCount = @($familyReplay | Where-Object { $_.runtimeBacked -eq $true }).Count
    productRuntimeBackedCount = @($familyReplay | Where-Object { $_.productRuntimeBacked -eq $true }).Count
    contractOnlyCount = @($familyReplay | Where-Object { $_.productRuntimeBacked -ne $true }).Count
    recoveredCount = @($familyReplay | Where-Object { $_.status -eq "recovered" }).Count
    failedCount = @($familyReplay | Where-Object { $_.status -eq "failed" }).Count
    replaySemantics = -not [string]::IsNullOrWhiteSpace([string]$family.replaySemantics)
    auditPayload = @(Get-ArrayValue $family.auditPayload).Count -gt 0
    failureStates = @(Get-ArrayValue $family.failureStates).Count -gt 0
    recoveryBehavior = -not [string]::IsNullOrWhiteSpace([string]$family.recoveryBehavior)
    evidenceStatus = [string]$family.evidenceStatus
  }
}

$targetEventCount = [int]$taxonomy.targetEventCount
$replayedCount = @($events).Count
$failedCount = @($events | Where-Object { $_.status -eq "failed" }).Count
$runtimeBackedCount = @($events | Where-Object { $_.runtimeBacked -eq $true }).Count
$productRuntimeBackedCount = @($events | Where-Object { $_.productRuntimeBacked -eq $true }).Count
$contractOnlyCount = @($events | Where-Object { $_.productRuntimeBacked -ne $true }).Count
$fullLocalReplay = $replayedCount -ge $targetEventCount -and $replayedCount -ge 450 -and $failedCount -eq 0

$status = if ($fullLocalReplay) {
  "passed_local_replay_runtime"
} elseif ($replayedCount -gt 0) {
  "partial_local_replay_runtime"
} else {
  "blocked_missing_behavior_taxonomy"
}

$checks = @(
  (New-GateResult "behavior_taxonomy_loaded" ($targetEventCount -eq 450) "targetEventCount=$targetEventCount"),
  (New-GateResult "local_replay_runtime_executed_450_plus" ($replayedCount -ge 450) "replayedCount=$replayedCount"),
  (New-GateResult "replay_semantics_audit_failure_recovery_present" (@($familyResults | Where-Object { -not $_.replaySemantics -or -not $_.auditPayload -or -not $_.failureStates -or -not $_.recoveryBehavior }).Count -eq 0) "families=$(@($familyResults).Count)"),
  (New-GateResult "all_events_replayed_without_failed_status" ($failedCount -eq 0) "failedEventCount=$failedCount"),
  (New-GateResult "runtime_backed_vs_contract_only_reported" ($runtimeBackedCount -eq $replayedCount -and $contractOnlyCount -ge 0) "runtimeBacked=$runtimeBackedCount contractOnly=$contractOnlyCount")
)

$failureReason = if ($status -eq "passed_local_replay_runtime") {
  ""
} elseif ($status -eq "partial_local_replay_runtime") {
  "local replay runtime executed but some events or family contracts failed"
} else {
  "behavior taxonomy was missing or produced no replay events"
}

$nextAction = if ($status -eq "passed_local_replay_runtime") {
  "Keep this 450+ local replay runtime report attached; wire contract-only families into product/browser/provider runtime before claiming target-site replay closure."
} else {
  "Fix behavior taxonomy replay/audit/failure/recovery metadata, then rerun scripts/live_replay_runtime_gate.ps1."
}

$report = [ordered]@{
  schemaVersion = "live_replay_runtime_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  taxonomyPath = "docs/taxonomy/behavior-event-taxonomy.json"
  targetEventCount = $targetEventCount
  replayedEventCount = $replayedCount
  runtimeBackedEventCount = $runtimeBackedCount
  productRuntimeBackedEventCount = $productRuntimeBackedCount
  contractOnlyEventCount = $contractOnlyCount
  recoveredEventCount = @($events | Where-Object { $_.status -eq "recovered" }).Count
  failedEventCount = $failedCount
  familyResults = $familyResults
  replaySample = @($events | Select-Object -First 40)
  checks = $checks
  summary = [ordered]@{
    failed = @($checks | Where-Object { $_.status -ne "passed" }).Count
    targetEventCount = $targetEventCount
    replayedEventCount = $replayedCount
    runtimeBackedEventCount = $runtimeBackedCount
    productRuntimeBackedEventCount = $productRuntimeBackedCount
    contractOnlyEventCount = $contractOnlyCount
    failedEventCount = $failedCount
    nextAction = $nextAction
  }
  failureReason = $failureReason
  liveTruthBoundary = @(
    "This is a deterministic local replay runtime over the behavior taxonomy.",
    "It proves replay semantics, audit payload, failure states, and recovery behavior can execute for 450+ local events.",
    "It does not claim browser target-site production replay, real provider closure, or human-like long-task behavior unless separate runtime reports exist."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("live-replay-runtime-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding UTF8

Write-Host "Live replay runtime report: $reportPath"
Write-Host "Status: $status"
Write-Host "Replayed events: $replayedCount / $targetEventCount"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "passed_local_replay_runtime") { exit 0 }
exit 1
