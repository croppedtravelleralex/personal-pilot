param(
  [string]$OutputDir = "data/reports/m15-browser-pool"
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

function Test-FileContains([string]$Path, [string]$Pattern) {
  if (-not (Test-Path $Path)) { return $false }
  $text = Get-Content -LiteralPath $Path -Raw -Encoding UTF8
  return $text.Contains($Pattern)
}

function Get-Field([object]$Value, [string]$Name) {
  if ($null -eq $Value) { return $null }
  if ($Value -is [System.Collections.IDictionary]) { return $Value[$Name] }
  $property = $Value.PSObject.Properties[$Name]
  if ($null -eq $property) { return $null }
  return $property.Value
}

function ConvertTo-SortKey([object]$Value, [datetime]$Fallback) {
  if ($null -ne $Value) {
    $text = [string]$Value
    $epochMs = 0L
    if ([Int64]::TryParse($text, [ref]$epochMs)) {
      if ($text.Length -le 10) { return [DateTimeOffset]::FromUnixTimeSeconds($epochMs).UtcDateTime }
      return [DateTimeOffset]::FromUnixTimeMilliseconds($epochMs).UtcDateTime
    }
    try {
      return ([DateTimeOffset]::Parse($text)).UtcDateTime
    } catch {}
  }
  return $Fallback.ToUniversalTime()
}

function Get-LatestJsonReport([string]$Dir, [string]$Pattern) {
  if (-not (Test-Path $Dir)) { return $null }
  $items = @()
  Get-ChildItem -LiteralPath $Dir -Filter $Pattern -File -ErrorAction SilentlyContinue | ForEach-Object {
    try {
      $value = Get-Content -LiteralPath $_.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
      $items += [pscustomobject]@{
        path = $_.FullName
        value = $value
        sortKey = ConvertTo-SortKey (Get-Field $value "generatedAt") $_.LastWriteTimeUtc
      }
    } catch {}
  }
  $sorted = @($items | Sort-Object sortKey -Descending)
  if ($sorted.Count -eq 0) { return $null }
  return $sorted[0]
}

$projectRoot = Resolve-ProjectRoot
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$enginePath = Join-Path $projectRoot "backend/internal/pool/engine.go"
$prewarmPath = Join-Path $projectRoot "backend/internal/pool/prewarm.go"
$testPath = Join-Path $projectRoot "backend/internal/pool/engine_test.go"
$latestProcessItem = Get-LatestJsonReport (Join-Path $projectRoot "data\reports\m15-browser-process") "m15-browser-process-gate-*.json"
$latestProcessReport = if ($null -ne $latestProcessItem) { Get-Field $latestProcessItem "value" } else { $null }
$latestProcessStatus = [string](Get-Field $latestProcessReport "status")
$latestProcessSummary = Get-Field $latestProcessReport "summary"
$latestProcessCdpReady = (Get-Field $latestProcessSummary "cdpReady") -eq $true
$latestProcessCleanupComplete = (Get-Field $latestProcessSummary "cleanupComplete") -eq $true

$testOutput = ""
$testExitCode = 0
Push-Location $projectRoot
try {
  $testOutput = & go test ./backend/internal/pool -count=1 2>&1 | Out-String
  $testExitCode = $LASTEXITCODE
} finally {
  Pop-Location
}

$checks = @(
  (New-GateResult "pool_go_tests_pass" ($testExitCode -eq 0) "go test ./backend/internal/pool -count=1 exitCode=$testExitCode"),
  (New-GateResult "cleanup_proof_source_present" (Test-FileContains $enginePath "type CleanupProof struct") "CleanupProof struct must exist"),
  (New-GateResult "release_cleanup_source_present" (Test-FileContains $enginePath "ReleaseWithCleanup") "ReleaseWithCleanup must emit cleanup proof"),
  (New-GateResult "cleanup_report_source_present" (Test-FileContains $enginePath "type CleanupReport struct") "CleanupReport struct must exist"),
  (New-GateResult "resource_budget_source_present" (Test-FileContains $enginePath "type BudgetStatus struct") "BudgetStatus must exist"),
  (New-GateResult "usage_source_present" (Test-FileContains $enginePath "func (e *Engine) Usage") "Usage accounting must exist"),
  (New-GateResult "process_attachment_source_present" ((Test-FileContains $enginePath "type ProcessAttachment struct") -and (Test-FileContains $enginePath "func (e *Engine) AttachProcess")) "Pool must attach real browser process metadata to a slot"),
  (New-GateResult "process_cleanup_source_present" ((Test-FileContains $enginePath "type ProcessCleanupProof struct") -and (Test-FileContains $enginePath "ReleaseProcessWithCleanup")) "Pool release must emit process cleanup proof"),
  (New-GateResult "prewarm_budget_step_present" (Test-FileContains $prewarmPath 'Name: "budget"') "Prewarm plan must include a budget step"),
  (New-GateResult "cleanup_tests_present" ((Test-FileContains $testPath "TestPoolBudgetAndCleanupExpiredLease") -and (Test-FileContains $testPath "TestPoolCleanupReclaimsStaleIdleAndFailedSlots")) "cleanup/budget tests must exist"),
  (New-GateResult "process_cleanup_tests_present" (Test-FileContains $testPath "TestPoolProcessAttachmentReleaseCleanupProof") "process attach/release cleanup test must exist"),
  (New-GateResult "real_browser_process_report_passed" ($latestProcessStatus -eq "passed_real_browser_process_prewarm_cleanup") "latestProcessStatus=$latestProcessStatus reportPath=$($latestProcessItem.path)"),
  (New-GateResult "real_browser_process_cdp_cleanup_ready" ($latestProcessCdpReady -and $latestProcessCleanupComplete) "cdpReady=$latestProcessCdpReady cleanupComplete=$latestProcessCleanupComplete")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) {
  "passed_real_pool_process_integration"
} elseif ($testExitCode -ne 0) {
  "failed_pool_tests"
} else {
  "failed_pool_source_contract"
}

$failureReason = if ($status -eq "passed_real_pool_process_integration") {
  ""
} elseif ($testExitCode -ne 0) {
  "backend/internal/pool tests failed"
} else {
  "pool lifecycle source contract is incomplete"
}

$report = [ordered]@{
  schemaVersion = "m15_browser_pool_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  goTest = [ordered]@{
    command = "go test ./backend/internal/pool -count=1"
    exitCode = $testExitCode
    status = if ($testExitCode -eq 0) { "passed" } else { "failed" }
    outputTail = (($testOutput -split "`r?`n") | Select-Object -Last 20) -join "`n"
  }
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    cleanupProofStatus = if (Test-FileContains $enginePath "type CleanupProof struct") { "present" } else { "missing" }
    resourceBudgetStatus = if (Test-FileContains $enginePath "type BudgetStatus struct") { "present" } else { "missing" }
    processAttachmentStatus = if (Test-FileContains $enginePath "type ProcessAttachment struct") { "present" } else { "missing" }
    processCleanupStatus = if (Test-FileContains $enginePath "type ProcessCleanupProof struct") { "present" } else { "missing" }
    realBrowserProcessStatus = $latestProcessStatus
    realBrowserProcessReportPath = if ($null -ne $latestProcessItem) { $latestProcessItem.path } else { $null }
    prewarmBudgetStepStatus = if (Test-FileContains $prewarmPath 'Name: "budget"') { "present" } else { "missing" }
    nextAction = if ($failed.Count -eq 0) {
      "Keep this M15 pool/process integration report attached; rerun after changing pool acquire/release, process launch, proxy binding, or cleanup behavior."
    } else {
      "Fix backend/internal/pool lifecycle tests or missing cleanup/budget markers, then rerun scripts/m15_browser_pool_gate.ps1."
    }
  }
  failureReason = $failureReason
  liveTruthBoundary = @(
    "This gate validates the local M15 pool lifecycle, process attach/release cleanup contract, and latest real browser process CDP/RSS/cleanup proof together.",
    "Proxy/session cleanup proof is local slot binding cleanup proof; remote proxy provider egress is not required for local-only use.",
    "It does not prove CAPTCHA/SMS/Email credential-backed provider smoke."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m15-browser-pool-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M15 browser pool gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "passed_real_pool_process_integration") { exit 0 }
exit 1
