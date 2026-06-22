param(
  [switch]$RunReleasePerformanceSmoke,
  [string]$ManualOperatorSmokeStatus = "not_run",
  [string]$CleanWin11InstallStatus = "not_run",
  [string]$PageNavigationSmokeStatus = "not_run",
  [string]$ProviderReadinessSmokeStatus = "not_run",
  [string]$SessionBundleSmokeStatus = "not_run",
  [string]$OutputDir = "data/reports/external-distribution"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

function Test-RequiredPath([string]$RelativePath) {
  $path = Join-Path $projectRoot $RelativePath
  return [ordered]@{
    path = $RelativePath
    present = Test-Path $path
  }
}

$checks = @(
  (Test-RequiredPath "personal-pilot-tauri.exe"),
  (Test-RequiredPath "docs\24-external-distribution-readiness.md"),
  (Test-RequiredPath "docs\02-current-state.md"),
  (Test-RequiredPath "README.md")
)

$readinessDoc = Get-Content (Join-Path $projectRoot "docs\24-external-distribution-readiness.md") -Raw
$requiredPhrases = @(
  [ordered]@{ id = "provider_closure"; alternatives = @("Provider closure") },
  [ordered]@{ id = "historical_only_scope"; alternatives = @("historical-only", "CANCELLED", "local-only") },
  [ordered]@{ id = "runtime_measurement"; alternatives = @("Runtime measurement") },
  [ordered]@{ id = "adspower_boundary"; alternatives = @("AdsPower boundary") },
  [ordered]@{ id = "taxonomy_boundary"; alternatives = @("450+", "450 taxonomy", "450` taxonomy", "450` event", "450` fingerprint") }
)
$phraseChecks = $requiredPhrases | ForEach-Object {
  $present = $false
  foreach ($phrase in $_.alternatives) {
    if ($readinessDoc.Contains($phrase)) { $present = $true; break }
  }
  if (-not $present -and $_.id -eq "taxonomy_boundary") {
    $present = $readinessDoc.Contains("450") -and $readinessDoc.Contains("taxonomy seed")
  }
  [ordered]@{ id = $_.id; alternatives = $_.alternatives; present = $present }
}

$releasePerformanceExitCode = $null
if ($RunReleasePerformanceSmoke) {
  & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "release_performance_smoke.ps1")
  $releasePerformanceExitCode = $LASTEXITCODE
}

$missing = @($checks | Where-Object { -not $_.present })
$missingPhrases = @($phraseChecks | Where-Object { -not $_.present })
$externalGates = @(
  [ordered]@{ id = "manual_operator_smoke"; status = "cancelled_local_only"; providedStatus = $ManualOperatorSmokeStatus; requiredForExternalDistribution = $false },
  [ordered]@{ id = "clean_win11_install_start_uninstall"; status = "cancelled_local_only"; providedStatus = $CleanWin11InstallStatus; requiredForExternalDistribution = $false },
  [ordered]@{ id = "page_navigation_smoke"; status = "cancelled_local_only"; providedStatus = $PageNavigationSmokeStatus; requiredForExternalDistribution = $false },
  [ordered]@{ id = "provider_readiness_smoke"; status = "cancelled_local_only"; providedStatus = $ProviderReadinessSmokeStatus; requiredForExternalDistribution = $false },
  [ordered]@{ id = "session_bundle_smoke"; status = "cancelled_local_only"; providedStatus = $SessionBundleSmokeStatus; requiredForExternalDistribution = $false }
)
$assetStatus = if ($missing.Count -eq 0 -and $missingPhrases.Count -eq 0 -and ($null -eq $releasePerformanceExitCode -or $releasePerformanceExitCode -eq 0)) { "passed" } else { "failed" }
$status = if ($assetStatus -eq "passed") {
  "cancelled_local_only"
} else {
  "failed"
}
$failureReason = if ($status -eq "cancelled_local_only") {
  ""
} else {
  "distribution assets or limitation wording failed local checks"
}

$report = [ordered]@{
  schemaVersion = "external_distribution_smoke_v2"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  checks = $checks
  readinessPhraseChecks = $phraseChecks
  externalGates = $externalGates
  assetStatus = $assetStatus
  failureReason = $failureReason
  releasePerformanceSmokeRan = [bool]$RunReleasePerformanceSmoke
  releasePerformanceExitCode = $releasePerformanceExitCode
  notes = @(
    "This smoke verifies historical distribution preflight assets and limitation wording.",
    "External distribution, manual operator smoke, and clean Win11 install/start/uninstall checks are cancelled for the current local-only scope."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("external-distribution-smoke-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "External distribution smoke report: $reportPath"
Write-Host "Status: $status"
if ($status -eq "failed") { exit 1 }
