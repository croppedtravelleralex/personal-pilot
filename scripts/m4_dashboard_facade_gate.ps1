param(
  [string]$OutputDir = "data/reports/m4-dashboard-facade"
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

$dashboardApiPath = Join-Path $projectRoot "src/modules/dashboard/api.ts"
$dashboardPagePath = Join-Path $projectRoot "src/modules/dashboard/DashboardPage.tsx"
$desktopServicePath = Join-Path $projectRoot "src/services/desktop.ts"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"

$dashboardApiText = Read-Text $dashboardApiPath
$dashboardPageText = Read-Text $dashboardPagePath
$desktopServiceText = Read-Text $desktopServicePath
$m4GateText = Read-Text $m4GatePath

$checks = @(
  (New-GateResult "dashboard_api_uses_typed_desktop_wrappers" (Test-ContainsAll $dashboardApiText @("readDashboardStats", "readLicenseStatus", "reloadDesktopConfig", "generateDesktopCdKeys")) "src/modules/dashboard/api.ts must call typed desktop service wrappers"),
  (New-GateResult "dashboard_api_no_dynamic_wails_binding" (Test-ContainsNone $dashboardApiText @("const bindings: any", "import('../../wailsjs/go/main/App')", "bindings.", "getBindings")) "src/modules/dashboard/api.ts must not dynamically import Wails bindings or call raw dashboard bindings"),
  (New-GateResult "desktop_service_dashboard_wrappers_present" (Test-ContainsAll $desktopServiceText @("readDashboardStats", "readLicenseStatus", "reloadDesktopConfig", "generateDesktopCdKeys", "DesktopDashboardStatsResponse", "DesktopLicenseStatusResponse")) "src/services/desktop.ts must expose typed dashboard/license/config/key wrappers"),
  (New-GateResult "dashboard_page_evidence_rows_preserved" (Test-ContainsAll $dashboardPageText @("fetchDashboardStats", "fetchEvidenceReportHistory", "fetchReleaseSmokeContract", "M4 Payload", "Runtime Adapter")) "Dashboard page must keep stats, evidence history, and runtime adapter report surface"),
  (New-GateResult "m4_gate_dashboard_facade_contract_registered" (Test-ContainsAll $m4GateText @("dashboard_facade_contract", "Test-DashboardFacadeContract")) "M4 acceptance gate must include the dashboard facade source contract")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_dashboard_facade_contract" } else { "failed_dashboard_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "Dashboard facade contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_dashboard_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    typedDesktopWrappers = if (($checks | Where-Object { $_.id -eq "dashboard_api_uses_typed_desktop_wrappers" }).status -eq "passed") { "present" } else { "missing" }
    dynamicBindingRemoved = if (($checks | Where-Object { $_.id -eq "dashboard_api_no_dynamic_wails_binding" }).status -eq "passed") { "yes" } else { "no" }
    evidenceRowsPreserved = if (($checks | Where-Object { $_.id -eq "dashboard_page_evidence_rows_preserved" }).status -eq "passed") { "yes" } else { "no" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining profile/settings/logs low-frequency dynamic Wails bindings without claiming tauriWailsBridge removal."
    } else {
      "Route Dashboard API through src/services/desktop.ts typed wrappers, remove raw Wails dynamic imports, and rerun scripts/m4_dashboard_facade_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates Dashboard API source-level typed facade usage only.",
    "It does not prove tauriWailsBridge removal, profile/settings/logs facade closure, provider credentials, remote proxy/TLS, SessionBundle local restore beyond its own gate, AdsPower refresh, or full 450 observed/replay coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-dashboard-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 dashboard facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
