param(
  [string]$OutputDir = "data/reports/taxonomy-audit"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

function Read-Taxonomy([string]$RelativePath) {
  $path = Join-Path $projectRoot $RelativePath
  if (-not (Test-Path $path)) { throw "Missing taxonomy file: $RelativePath" }
  return Get-Content $path -Raw | ConvertFrom-Json
}

function Measure-Taxonomy($taxonomy, [string]$kind) {
  $familyCount = @($taxonomy.families).Count
  $targetTotal = 0
  foreach ($family in $taxonomy.families) {
    $targetTotal += [int]$family.targetCount
  }
  $declaredTarget = if ($null -ne $taxonomy.targetSignalCount) {
    [int]$taxonomy.targetSignalCount
  } else {
    [int]$taxonomy.targetEventCount
  }
  return [ordered]@{
    kind = $kind
    schemaVersion = $taxonomy.schemaVersion
    status = $taxonomy.status
    familyCount = $familyCount
    declaredTargetCount = $declaredTarget
    familyTargetTotal = $targetTotal
    targetTotalMatches = ($targetTotal -eq $declaredTarget -and $declaredTarget -eq 450)
    evidenceBackedFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.evidenceStatus }).Count
    replaySemanticsFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.replaySemantics }).Count
    auditPayloadFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.auditPayload }).Count
    failureStatesFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.failureStates }).Count
    recoveryBehaviorFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.recoveryBehavior }).Count
    collectorPathFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.collectorPath }).Count
    layerFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.declaredAppliedObservedLayer }).Count
    adapterSupportFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.adapterSupport }).Count
    failureModeFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.failureMode }).Count
    repeatabilityRuleFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.repeatabilityRule }).Count
    evidenceSchemaFamilyCount = @($taxonomy.families | Where-Object { $null -ne $_.evidenceSchema }).Count
  }
}

$fingerprint = Read-Taxonomy "docs\taxonomy\fingerprint-signal-taxonomy.json"
$behavior = Read-Taxonomy "docs\taxonomy\behavior-event-taxonomy.json"
$items = @(
  (Measure-Taxonomy $fingerprint "fingerprint"),
  (Measure-Taxonomy $behavior "behavior")
)
$failed = @($items | Where-Object { -not $_.targetTotalMatches })
$status = if ($failed.Count -eq 0) { "passed" } else { "failed" }

$report = [ordered]@{
  schemaVersion = "taxonomy_audit_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  items = $items
  notes = @(
    "This validates taxonomy seed accounting only.",
    "A 450 target count is not shipped runtime coverage until replay/observed evidence exists."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("taxonomy-audit-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Taxonomy audit report: $reportPath"
Write-Host "Status: $status"
if ($status -ne "passed") { exit 1 }
