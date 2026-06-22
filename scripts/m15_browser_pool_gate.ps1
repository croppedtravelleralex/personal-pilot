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

$projectRoot = Resolve-ProjectRoot
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$enginePath = Join-Path $projectRoot "backend/internal/pool/engine.go"
$prewarmPath = Join-Path $projectRoot "backend/internal/pool/prewarm.go"
$testPath = Join-Path $projectRoot "backend/internal/pool/engine_test.go"

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
  (New-GateResult "prewarm_budget_step_present" (Test-FileContains $prewarmPath 'Name: "budget"') "Prewarm plan must include a budget step"),
  (New-GateResult "cleanup_tests_present" ((Test-FileContains $testPath "TestPoolBudgetAndCleanupExpiredLease") -and (Test-FileContains $testPath "TestPoolCleanupReclaimsStaleIdleAndFailedSlots")) "cleanup/budget tests must exist")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) {
  "passed_pool_lifecycle_harness"
} elseif ($testExitCode -ne 0) {
  "failed_pool_tests"
} else {
  "failed_pool_source_contract"
}

$failureReason = if ($status -eq "passed_pool_lifecycle_harness") {
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
    prewarmBudgetStepStatus = if (Test-FileContains $prewarmPath 'Name: "budget"') { "present" } else { "missing" }
    nextAction = if ($failed.Count -eq 0) {
      "Connect this lifecycle harness to real browser process prewarm/acquire/release and collect process/RSS cleanup proof before claiming M15 complete."
    } else {
      "Fix backend/internal/pool lifecycle tests or missing cleanup/budget markers, then rerun scripts/m15_browser_pool_gate.ps1."
    }
  }
  failureReason = $failureReason
  liveTruthBoundary = @(
    "This gate validates an in-memory M15 browser pool lifecycle harness only.",
    "It proves local acquire/release, resource budget accounting, and cleanup proof contracts.",
    "It does not prove real browser process prewarm, CDP readiness, RSS/process budgets, proxy/TLS behavior, provider closure, SessionBundle local restore beyond its own gate, AdsPower refresh, or full 450 coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m15-browser-pool-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M15 browser pool gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "passed_pool_lifecycle_harness") { exit 0 }
exit 1
