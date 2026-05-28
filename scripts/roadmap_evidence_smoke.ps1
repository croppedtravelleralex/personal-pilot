param(
  [string]$OutputDir = "data/reports/roadmap-evidence"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

function Read-Text([string]$RelativePath) {
  $path = Join-Path $projectRoot $RelativePath
  if (-not (Test-Path $path)) { return "" }
  return Get-Content -Path $path -Raw -Encoding UTF8
}

function Test-AllNeedles([string]$Text, [string[]]$Needles) {
  foreach ($needle in $Needles) {
    if (-not $Text.Contains($needle)) { return $false }
  }
  return $true
}

function New-Check([string]$Id, [string]$Title, [bool]$Passed, [string]$Evidence, [string]$Boundary = "") {
  [ordered]@{
    id = $Id
    title = $Title
    status = if ($Passed) { "passed" } else { "failed" }
    evidence = $Evidence
    boundary = $Boundary
  }
}

$environmentInjector = Read-Text "backend\internal\behavior\environment_injector.go"
$environmentTests = Read-Text "backend\internal\behavior\environment_injector_test.go"
$environmentAudit = Read-Text "backend\internal\behavior\environment_audit.go"
$lifecycleEngine = Read-Text "backend\internal\behavior\lifecycle\engine.go"
$lifecycleState = Read-Text "backend\internal\behavior\lifecycle\state.go"
$lifecycleTests = (Read-Text "backend\internal\behavior\lifecycle\engine_test.go") + "`n" + (Read-Text "backend\internal\behavior\lifecycle\state_test.go")

$checks = @()
$checks += New-Check "phase_1_2_browser_api_surface" "browser_api_surface implementation scan" `
  (Test-AllNeedles $environmentInjector @("Navigator.prototype, 'webdriver'", "Navigator.prototype, 'languages'", "Navigator.prototype, 'platform'", "Navigator.prototype, 'vendor'", "Navigator.prototype, 'userAgent'", "Navigator.prototype, 'hardwareConcurrency'", "Navigator.prototype, 'deviceMemory'", "Navigator.prototype, 'plugins'", "Navigator.prototype, 'mimeTypes'", "navigator.mediaDevices.enumerateDevices", "navigator.mediaDevices.getUserMedia")) `
  "environment_injector.go contains the browser API hooks currently implemented for the roadmap family." `
  "This is implementation coverage, not a 44-field observed profile-browser proof."

$checks += New-Check "phase_1_3_canvas_rendering" "canvas_rendering implementation scan" `
  (Test-AllNeedles $environmentInjector @("HTMLCanvasElement.prototype.toDataURL", "CanvasRenderingContext2D.prototype.getImageData", "CanvasRenderingContext2D.prototype.fillText", "stableNoise", "canvasNoise")) `
  "environment_injector.go contains canvas toDataURL/getImageData/fillText seeded-noise hooks." `
  "This is implementation coverage, not a 38-field observed profile-browser proof."

$checks += New-Check "phase_1_4_timezone_locale" "timezone_locale implementation scan" `
  (Test-AllNeedles $environmentInjector @("Intl.DateTimeFormat.prototype.resolvedOptions", "Date.prototype.getTimezoneOffset", "Accept-Language", "Network.setExtraHTTPHeaders")) `
  "environment_injector.go coordinates timezone, offset, locale and Accept-Language header injection." `
  "This is implementation coverage, not a 34-field observed profile-browser proof."

$checks += New-Check "environment_audit_contract" "environment audit contract" `
  (Test-AllNeedles ($environmentAudit + $environmentTests) @("AuditEnvironmentInjection", "browser_api_surface", "canvas_rendering", "timezone_locale", "supported_pending_observed")) `
  "environment audit code preserves supported-vs-observed distinction for the partially completed target-count families." `
  "Observed proof still requires a real profile browser validation report."

$checks += New-Check "phase_3_1_lifecycle_runtime" "lifecycle local runtime implementation" `
  (Test-AllNeedles ($lifecycleEngine + $lifecycleState + $lifecycleTests) @("PlanDailySession", "EvolveInterest", "AdvanceSocialStage", "AssessGeoCoherence", "behavior_lifecycle_state", "IncrementSession", "TestLifecycleStorePersistsState")) `
  "lifecycle engine, SQLite store, risk/geo/session strategy and tests are present." `
  "This is local runtime/store evidence, not proof of scheduled production activity against real accounts."

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_with_external_evidence_pending" } else { "failed" }

$report = [ordered]@{
  schemaVersion = "roadmap_evidence_smoke_v1"
  generatedAt = [DateTimeOffset]::Now.ToString("o")
  projectRoot = $projectRoot
  status = $status
  checks = $checks
  remainingExternalEvidence = @(
    "profile-browser observed proof for 44/38/34 target families",
    "real provider credentials and smoke",
    "second-machine SessionBundle portability",
    "real Camoufox task runtime page/artifact/cancel/performance smoke",
    "real browser pool prewarm and Xray/Sing-Box outbound runtime proof"
  )
  notes = @(
    "This smoke intentionally separates code implementation evidence from external/runtime observed proof.",
    "Do not convert partial target-count families to full observed completion without profile-browser reports."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("roadmap-evidence-smoke-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Roadmap evidence smoke report: $reportPath"
Write-Host "Status: $status"
if ($status -eq "failed") { exit 1 }
