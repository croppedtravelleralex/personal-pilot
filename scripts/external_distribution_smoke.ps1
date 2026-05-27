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
  (Test-RequiredPath "src-tauri\target\release\bundle\nsis\personal-pilot_1.1.0_x64-setup.exe"),
  (Test-RequiredPath "src-tauri\target\release\personal-pilot-tauri.exe"),
  (Test-RequiredPath "personal-pilot-tauri.exe"),
  (Test-RequiredPath "docs\24-external-distribution-readiness.md"),
  (Test-RequiredPath "docs\02-current-state.md"),
  (Test-RequiredPath "README.md")
)

$readinessDoc = Get-Content (Join-Path $projectRoot "docs\24-external-distribution-readiness.md") -Raw
$requiredPhrases = @(
  [ordered]@{ id = "provider_closure"; alternatives = @("Provider closure") },
  [ordered]@{ id = "profile_portability"; alternatives = @("Profile portability") },
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
  [ordered]@{ id = "manual_operator_smoke"; status = $ManualOperatorSmokeStatus; requiredForExternalDistribution = $true },
  [ordered]@{ id = "clean_win11_install_start_uninstall"; status = $CleanWin11InstallStatus; requiredForExternalDistribution = $true },
  [ordered]@{ id = "page_navigation_smoke"; status = $PageNavigationSmokeStatus; requiredForExternalDistribution = $true },
  [ordered]@{ id = "provider_readiness_smoke"; status = $ProviderReadinessSmokeStatus; requiredForExternalDistribution = $true },
  [ordered]@{ id = "session_bundle_smoke"; status = $SessionBundleSmokeStatus; requiredForExternalDistribution = $true }
)
$blockedExternalGates = @($externalGates | Where-Object { $_.status -ne "passed" })
$assetStatus = if ($missing.Count -eq 0 -and $missingPhrases.Count -eq 0 -and ($null -eq $releasePerformanceExitCode -or $releasePerformanceExitCode -eq 0)) { "passed" } else { "failed" }
$status = if ($assetStatus -eq "passed" -and $blockedExternalGates.Count -eq 0) {
  "passed"
} elseif ($assetStatus -eq "passed") {
  "blocked_external_smoke_required"
} else {
  "failed"
}
$failureReason = if ($status -eq "passed") {
  ""
} elseif ($assetStatus -ne "passed") {
  "distribution assets or limitation wording failed local checks"
} else {
  "external smoke gates not passed: $(@($blockedExternalGates | ForEach-Object { $_.id }) -join ', ')"
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
    "This smoke verifies distribution preflight assets and limitation wording.",
    "It does not replace real human installation/start/navigation smoke on a clean Win11 machine."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("external-distribution-smoke-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "External distribution smoke report: $reportPath"
Write-Host "Status: $status"
if ($status -eq "failed") { exit 1 }
if ($status -eq "blocked_external_smoke_required") { exit 2 }
