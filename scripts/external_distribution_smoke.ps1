param(
  [switch]$RunReleasePerformanceSmoke,
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
  (Test-RequiredPath "src-tauri\target\release\bundle\nsis\PersonaPilot_0.1.0_x64-setup.exe"),
  (Test-RequiredPath "src-tauri\target\release\persona-pilot-desktop.exe"),
  (Test-RequiredPath "docs\24-external-distribution-readiness.md"),
  (Test-RequiredPath "docs\02-current-state.md"),
  (Test-RequiredPath "README.md")
)

$readinessDoc = Get-Content (Join-Path $projectRoot "docs\24-external-distribution-readiness.md") -Raw
$requiredPhrases = @(
  "Provider closure",
  "Profile portability",
  "Runtime measurement",
  "AdsPower boundary",
  "450+"
)
$phraseChecks = $requiredPhrases | ForEach-Object {
  [ordered]@{ phrase = $_; present = $readinessDoc.Contains($_) }
}

$releasePerformanceExitCode = $null
if ($RunReleasePerformanceSmoke) {
  & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "release_performance_smoke.ps1")
  $releasePerformanceExitCode = $LASTEXITCODE
}

$missing = @($checks | Where-Object { -not $_.present })
$missingPhrases = @($phraseChecks | Where-Object { -not $_.present })
$status = if ($missing.Count -eq 0 -and $missingPhrases.Count -eq 0 -and ($null -eq $releasePerformanceExitCode -or $releasePerformanceExitCode -eq 0)) {
  "passed"
} else {
  "failed"
}

$report = [ordered]@{
  schemaVersion = "external_distribution_smoke_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  checks = $checks
  readinessPhraseChecks = $phraseChecks
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
if ($status -ne "passed") { exit 1 }
