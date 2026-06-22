param(
  [string]$OutputDir = "data/reports/m4-automation-facade"
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

$automationPagePath = Join-Path $projectRoot "src/modules/browser/pages/AutomationPage.tsx"
$browserApiPath = Join-Path $projectRoot "src/modules/browser/api.ts"
$dashboardPagePath = Join-Path $projectRoot "src/modules/dashboard/DashboardPage.tsx"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"

$automationPageText = Read-Text $automationPagePath
$browserApiText = Read-Text $browserApiPath
$dashboardPageText = Read-Text $dashboardPagePath
$m4GateText = Read-Text $m4GatePath

$checks = @(
  (New-GateResult "automation_page_uses_browser_api_facade" (Test-ContainsAll $automationPageText @("fetchSchedulerTasks", "createSchedulerTask", "deleteSchedulerTask", "runSchedulerTaskNow", "fetchAutomationRules", "createAutomationRule", "deleteAutomationRule", "toggleAutomationRule", "testFireAutomationRule")) "AutomationPage must call scheduler/rule operations through src/modules/browser/api.ts"),
  (New-GateResult "automation_page_no_direct_wails_scheduler_rule" (Test-ContainsNone $automationPageText @("../../../wailsjs/go/main/App", "../../../wailsjs/go/models", "SchedulerListTasks", "SchedulerAddTask", "SchedulerRemoveTask", "SchedulerRunTaskNow", "AutomationRuleList", "AutomationRuleCreate", "AutomationRuleDelete", "AutomationRuleToggle", "AutomationRuleTestFire", "new backend.")) "AutomationPage must not import Wails App/models or call scheduler/rule bindings directly"),
  (New-GateResult "browser_api_scheduler_rule_facade_present" (Test-ContainsAll $browserApiText @("export async function fetchSchedulerTasks", "export async function createSchedulerTask", "export async function deleteSchedulerTask", "export async function runSchedulerTaskNow", "export async function fetchAutomationRules", "export async function createAutomationRule", "export async function deleteAutomationRule", "export async function toggleAutomationRule", "export async function testFireAutomationRule")) "src/modules/browser/api.ts must expose scheduler/rule facade functions"),
  (New-GateResult "browser_api_scheduler_rule_types_present" (Test-ContainsAll $browserApiText @("export interface SchedulerTaskInput", "export interface SchedulerTaskInfo", "export interface AutomationRuleInput", "export interface AutomationRuleInfo", "SchedulerListTasks: () => Promise", "AutomationRuleCreate: (input: AutomationRuleInput) => Promise")) "src/modules/browser/api.ts must keep typed DTOs for scheduler/rule calls"),
  (New-GateResult "dashboard_evidence_row_registered" (Test-ContainsAll $dashboardPageText @("m4_automation_facade", "M4 Automation")) "Dashboard must list the M4 automation facade report row"),
  (New-GateResult "m4_gate_automation_contract_registered" (Test-ContainsAll $m4GateText @("automation_facade_contract", "Test-AutomationFacadeContract")) "M4 acceptance gate must include the automation facade source contract")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_automation_facade_contract" } else { "failed_automation_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "Automation facade contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_automation_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    pageFacadeUsage = if (($checks | Where-Object { $_.id -eq "automation_page_uses_browser_api_facade" }).status -eq "passed") { "present" } else { "missing" }
    directWailsImportRemoved = if (($checks | Where-Object { $_.id -eq "automation_page_no_direct_wails_scheduler_rule" }).status -eq "passed") { "yes" } else { "no" }
    browserApiFacade = if (($checks | Where-Object { $_.id -eq "browser_api_scheduler_rule_facade_present" }).status -eq "passed") { "present" } else { "missing" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining monitor/app shell/browser runtime bridge APIs without claiming tauriWailsBridge removal."
    } else {
      "Route AutomationPage scheduler/rule calls through src/modules/browser/api.ts, remove direct Wails imports/models, and rerun scripts/m4_automation_facade_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates AutomationPage scheduler/rule source-level facade usage only.",
    "It does not prove tauriWailsBridge removal, full browser API closure, monitor/app shell runtime closure, workbench/core facade closure, provider credentials, remote proxy/TLS, SessionBundle local restore beyond its own gate, AdsPower refresh, or full 450 observed/replay coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-automation-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 automation facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
