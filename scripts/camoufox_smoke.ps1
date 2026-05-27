param(
  [string]$ProjectRoot = "",
  [string]$OutputDir = "data/reports/camoufox-smoke",
  [switch]$NoReport,
  [switch]$AllowBlocked,
  [switch]$RunRustTests,
  [switch]$RunReleasePerformanceSmoke
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  if (-not [string]::IsNullOrWhiteSpace($ProjectRoot)) {
    return (Resolve-Path $ProjectRoot).Path
  }
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Read-TextFile([string]$Path) {
  if (-not (Test-Path $Path)) {
    return $null
  }
  return Get-Content -Path $Path -Raw
}

function New-Check([string]$Id, [bool]$Passed, [string]$Evidence, [bool]$Required = $true) {
  return [ordered]@{
    id = $Id
    status = if ($Passed) { "passed" } elseif ($Required) { "failed" } else { "not_run" }
    required = $Required
    evidence = $Evidence
  }
}

function Test-ContainsAll([string]$Text, [string[]]$Needles) {
  if ($null -eq $Text) {
    return $false
  }
  foreach ($needle in $Needles) {
    if (-not $Text.Contains($needle)) {
      return $false
    }
  }
  return $true
}

$root = Resolve-ProjectRoot
$runnerModPath = Join-Path $root "src\runner\mod.rs"
$camoufoxRunnerPath = Join-Path $root "src\runner\camoufox.rs"
$statePath = Join-Path $root "src-tauri\src\state.rs"
$commandsPath = Join-Path $root "src-tauri\src\commands.rs"
$desktopServicePath = Join-Path $root "src\services\desktop.ts"
$desktopTypesPath = Join-Path $root "src\types\desktop.ts"

$runnerMod = Read-TextFile $runnerModPath
$camoufoxRunner = Read-TextFile $camoufoxRunnerPath
$state = Read-TextFile $statePath
$commands = Read-TextFile $commandsPath
$desktopService = Read-TextFile $desktopServicePath
$desktopTypes = Read-TextFile $desktopTypesPath

$checks = @()
$checks += New-Check "runner_kind_registered" (Test-ContainsAll $runnerMod @("pub mod camoufox;", "Camoufox", '"camoufox" => RunnerKind::Camoufox')) "src/runner/mod.rs registers RunnerKind::Camoufox and PERSONA_PILOT_RUNNER=camoufox mapping."
$checks += New-Check "tauri_state_selects_camoufox_runner" (Test-ContainsAll $state @("camoufox::CamoufoxRunner", "RunnerKind::Camoufox", "Arc::new(CamoufoxRunner)")) "src-tauri/src/state.rs can select CamoufoxRunner without changing the default runner."
$checks += New-Check "camoufox_skeleton_blocks_unconfigured_launch" (Test-ContainsAll $camoufoxRunner @("PERSONA_PILOT_CAMOUFOX_ENABLED", "PERSONA_PILOT_CAMOUFOX_CONFIG", "runner_disabled", "runner_config_missing", "runner_not_implemented")) "src/runner/camoufox.rs returns explicit configuration failures before any browser launch."
$checks += New-Check "camoufox_skeleton_reports_no_browser_launch" (Test-ContainsAll $camoufoxRunner @('"real_browser_execution": false', '"browser_launch_attempted": false', "supports_artifacts: false")) "src/runner/camoufox.rs marks current skeleton as non-launching and no artifact runtime yet."
$checks += New-Check "tauri_commands_available" (Test-ContainsAll $commands @("read_camoufox_settings", "apply_camoufox_settings", "check_camoufox_capability", "camoufox_path_empty", "camoufox_path_not_found", "camoufox_path_available")) "src-tauri/src/commands.rs exposes settings and path capability checks with normalized codes."
$checks += New-Check "desktop_wrapper_available" (Test-ContainsAll $desktopService @("readCamoufoxSettings", "applyCamoufoxSettings", "checkCamoufoxCapability", "invokeDesktop(`"read_camoufox_settings`"", "invokeDesktop(`"check_camoufox_capability`"")) "src/services/desktop.ts exports typed wrappers; invoke remains centralized."
$checks += New-Check "desktop_types_available" (Test-ContainsAll $desktopTypes @("DesktopCamoufoxSettings", "DesktopCamoufoxSettingsDraft", "DesktopCamoufoxCapability", "DesktopCamoufoxCapabilityRequest")) "src/types/desktop.ts defines typed Camoufox request/response contracts."

$optionalResults = @()
if ($RunRustTests) {
  Push-Location $root
  try {
    & cargo test --quiet camoufox
    $optionalResults += [ordered]@{ id = "cargo_test_camoufox"; status = "passed"; command = "cargo test --quiet camoufox" }
  } catch {
    $optionalResults += [ordered]@{ id = "cargo_test_camoufox"; status = "failed"; command = "cargo test --quiet camoufox"; error = $_.Exception.Message }
  } finally {
    Pop-Location
  }
} else {
  $optionalResults += [ordered]@{ id = "cargo_test_camoufox"; status = "not_run"; command = "cargo test --quiet camoufox" }
}

if ($RunReleasePerformanceSmoke) {
  try {
    & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "release_performance_smoke.ps1")
    $optionalResults += [ordered]@{ id = "release_performance_smoke"; status = "passed"; command = "powershell -ExecutionPolicy Bypass -File scripts/release_performance_smoke.ps1" }
  } catch {
    $optionalResults += [ordered]@{ id = "release_performance_smoke"; status = "failed"; command = "powershell -ExecutionPolicy Bypass -File scripts/release_performance_smoke.ps1"; error = $_.Exception.Message }
  }
} else {
  $optionalResults += [ordered]@{ id = "release_performance_smoke"; status = "not_run"; command = "powershell -ExecutionPolicy Bypass -File scripts/release_performance_smoke.ps1" }
}

$failedRequired = @($checks | Where-Object { $_.required -and $_.status -ne "passed" })
$failedOptional = @($optionalResults | Where-Object { $_.status -eq "failed" })
$runtimeImplemented = ($null -ne $camoufoxRunner) -and (-not $camoufoxRunner.Contains("runner_not_implemented"))

$status = if ($failedRequired.Count -gt 0 -or $failedOptional.Count -gt 0) {
  "failed"
} elseif (-not $runtimeImplemented) {
  "contract_ready_runtime_smoke_required"
} else {
  "passed"
}

$report = [ordered]@{
  schemaVersion = "camoufox_smoke_v1"
  generatedAt = [DateTimeOffset]::Now.ToString("o")
  projectRoot = $root
  status = $status
  checks = $checks
  optionalResults = $optionalResults
  requiredRuntimeProof = @(
    "run a Camoufox task that opens https://example.com",
    "record summary/stdout/stderr/screenshot artifact refs",
    "verify timeout/cancel leaves no Camoufox/Python child process",
    "run release_performance_smoke.ps1 and enforce-win11-tauri.ps1 against personal-pilot-tauri.exe"
  )
  notes = @(
    "This script is non-destructive: it does not edit settings, kill processes, or launch Camoufox by default.",
    "Current skeleton proof is not runtime completion; blocked status is expected until browser launch and cleanup are implemented."
  )
}

if (-not $NoReport) {
  $absoluteOutputDir = Join-Path $root $OutputDir
  New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null
  $reportPath = Join-Path $absoluteOutputDir ("camoufox-smoke-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
  $report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
  Write-Host "Camoufox smoke report: $reportPath"
}

Write-Host "Camoufox smoke status: $status"
if ($status -eq "failed") { exit 1 }
if ($status -eq "contract_ready_runtime_smoke_required" -and -not $AllowBlocked) { exit 2 }
