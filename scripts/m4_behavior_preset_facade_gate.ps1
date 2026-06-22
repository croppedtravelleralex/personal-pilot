param(
  [string]$OutputDir = "data/reports/m4-behavior-preset-facade"
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

function Read-Text([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path)) { return "" }
  return Get-Content -LiteralPath $Path -Raw -Encoding UTF8
}

function Test-ContainsAll([string]$Text, [string[]]$Tokens) {
  foreach ($token in $Tokens) {
    if ($Text.IndexOf($token, [System.StringComparison]::OrdinalIgnoreCase) -lt 0) { return $false }
  }
  return $true
}

function Test-ContainsNone([string]$Text, [string[]]$Tokens) {
  foreach ($token in $Tokens) {
    if ($Text.IndexOf($token, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) { return $false }
  }
  return $true
}

$projectRoot = Resolve-ProjectRoot
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$fingerprintPanelPath = Join-Path $projectRoot "src/modules/browser/components/FingerprintPanel.tsx"
$browserApiPath = Join-Path $projectRoot "src/modules/browser/api.ts"
$dashboardPagePath = Join-Path $projectRoot "src/modules/dashboard/DashboardPage.tsx"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"

$fingerprintPanelText = Read-Text $fingerprintPanelPath
$browserApiText = Read-Text $browserApiPath
$dashboardPageText = Read-Text $dashboardPagePath
$m4GateText = Read-Text $m4GatePath

$checks = @(
  (New-GateResult "fingerprint_panel_uses_browser_api_facade" (Test-ContainsAll $fingerprintPanelText @("import { fetchBehaviorPresets } from '../api'", "fetchBehaviorPresets().then(setBehaviorPresets)")) "FingerprintPanel must load behavior presets through the browser module API facade"),
  (New-GateResult "fingerprint_panel_no_direct_wails_behavior_preset" (Test-ContainsNone $fingerprintPanelText @("../../../wailsjs/go/main/App", "wailsjs/go/main/App", "import { BehaviorPresetList", "BehaviorPresetList()")) "FingerprintPanel must not import or call BehaviorPresetList directly from Wails"),
  (New-GateResult "browser_api_behavior_preset_facade_present" (Test-ContainsAll $browserApiText @("BehaviorPresetList: () => Promise", "export async function fetchBehaviorPresets", "bindings?.BehaviorPresetList", "bindings.BehaviorPresetList()")) "src/modules/browser/api.ts must keep the typed BehaviorPresetList facade"),
  (New-GateResult "dashboard_evidence_row_registered" (Test-ContainsAll $dashboardPageText @("m4_behavior_preset_facade", "M4 Behavior")) "Dashboard must list the M4 behavior preset facade report row"),
  (New-GateResult "m4_gate_behavior_preset_contract_registered" (Test-ContainsAll $m4GateText @("behavior_preset_facade_contract", "Test-BehaviorPresetFacadeContract")) "M4 acceptance gate must include the behavior preset facade source contract")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_behavior_preset_facade_contract" } else { "failed_behavior_preset_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "Behavior preset facade contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_behavior_preset_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    componentFacadeUsage = if (($checks | Where-Object { $_.id -eq "fingerprint_panel_uses_browser_api_facade" }).status -eq "passed") { "present" } else { "missing" }
    directWailsImportRemoved = if (($checks | Where-Object { $_.id -eq "fingerprint_panel_no_direct_wails_behavior_preset" }).status -eq "passed") { "yes" } else { "no" }
    browserApiFacade = if (($checks | Where-Object { $_.id -eq "browser_api_behavior_preset_facade_present" }).status -eq "passed") { "present" } else { "missing" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining browser/workbench/core bridge APIs without claiming tauriWailsBridge removal."
    } else {
      "Route FingerprintPanel behavior preset loading through src/modules/browser/api.ts, remove direct Wails imports, and rerun scripts/m4_behavior_preset_facade_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates FingerprintPanel behavior preset source-level facade usage only.",
    "It does not prove tauriWailsBridge removal, full browser API closure, workbench/core facade closure, provider credentials, remote proxy/TLS, SessionBundle local restore beyond its own gate, AdsPower refresh, or full 450 observed/replay coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-behavior-preset-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 behavior preset facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
