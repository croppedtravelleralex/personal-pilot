param(
  [string]$OutputDir = "data/reports/m4-monitor-facade"
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

$monitorPagePath = Join-Path $projectRoot "src/modules/monitor/EventMonitorPage.tsx"
$desktopServicePath = Join-Path $projectRoot "src/services/desktop.ts"
$dashboardPagePath = Join-Path $projectRoot "src/modules/dashboard/DashboardPage.tsx"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"

$monitorPageText = Read-Text $monitorPagePath
$desktopServiceText = Read-Text $desktopServicePath
$dashboardPageText = Read-Text $dashboardPagePath
$m4GateText = Read-Text $m4GatePath

$checks = @(
  (New-GateResult "monitor_page_uses_desktop_facade" (Test-ContainsAll $monitorPageText @("desktopRuntimeListen", "queryEventLog", "countEventLog", "exportEventLog", "pruneEventLog", "DesktopEventLogQueryInput", "DesktopEventLogEntry")) "EventMonitorPage must call runtime subscriptions and event-log history through src/services/desktop.ts"),
  (New-GateResult "monitor_page_no_direct_wails_event_log" (Test-ContainsNone $monitorPageText @("../../wailsjs/go/main/App", "../../wailsjs/go/models", "EventLogQuery(q)", "EventLogCount(q)", "EventLogPrune(before)", "EventLogExport(q)", "(window as any).runtime", "runtime.EventsOn", "catch (e: any)")) "EventMonitorPage must not import Wails App/models, read window.runtime directly, or use any-catch event-log errors"),
  (New-GateResult "desktop_service_monitor_wrappers_present" (Test-ContainsAll $desktopServiceText @("export function desktopRuntimeListen", "export interface DesktopEventLogQueryInput", "export interface DesktopEventLogEntry", "export const queryEventLog", "export const countEventLog", "export const pruneEventLog", "export const exportEventLog")) "src/services/desktop.ts must expose typed monitor runtime and event-log wrappers"),
  (New-GateResult "dashboard_evidence_row_registered" (Test-ContainsAll $dashboardPageText @("m4_monitor_facade", "M4 Monitor")) "Dashboard must list the M4 monitor facade report row"),
  (New-GateResult "m4_gate_monitor_contract_registered" (Test-ContainsAll $m4GateText @("monitor_facade_contract", "Test-MonitorFacadeContract")) "M4 acceptance gate must include the monitor facade source contract")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_monitor_facade_contract" } else { "failed_monitor_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "Monitor facade contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_monitor_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    monitorFacadeUsage = if (($checks | Where-Object { $_.id -eq "monitor_page_uses_desktop_facade" }).status -eq "passed") { "present" } else { "missing" }
    directWailsImportsRemoved = if (($checks | Where-Object { $_.id -eq "monitor_page_no_direct_wails_event_log" }).status -eq "passed") { "yes" } else { "no" }
    desktopWrappers = if (($checks | Where-Object { $_.id -eq "desktop_service_monitor_wrappers_present" }).status -eq "passed") { "present" } else { "missing" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining browser/workbench/core bridge APIs without claiming tauriWailsBridge removal."
    } else {
      "Route EventMonitorPage runtime and history calls through src/services/desktop.ts, remove direct Wails imports/runtime access, and rerun scripts/m4_monitor_facade_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates EventMonitorPage monitor source-level facade usage only.",
    "It does not prove tauriWailsBridge removal, full browser/workbench/core facade closure, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full 450 observed/replay coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-monitor-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 monitor facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
