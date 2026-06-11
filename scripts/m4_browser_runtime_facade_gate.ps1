param(
  [string]$OutputDir = "data/reports/m4-browser-runtime-facade"
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

$apiPath = Join-Path $projectRoot "src/modules/browser/api.ts"
$typesPath = Join-Path $projectRoot "src/modules/browser/types.ts"
$listPagePath = Join-Path $projectRoot "src/modules/browser/pages/BrowserListPage.tsx"
$detailPagePath = Join-Path $projectRoot "src/modules/browser/pages/BrowserDetailPage.tsx"
$dashboardPagePath = Join-Path $projectRoot "src/modules/dashboard/DashboardPage.tsx"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"

$apiText = Read-Text $apiPath
$typesText = Read-Text $typesPath
$listText = Read-Text $listPagePath
$detailText = Read-Text $detailPagePath
$dashboardPageText = Read-Text $dashboardPagePath
$m4GateText = Read-Text $m4GatePath

$directRuntimeMarkers = @("../../../wailsjs/runtime/runtime", "../../wailsjs/runtime", "import { EventsOn }", "EventsOn(")

$checks = @(
  (New-GateResult "browser_api_uses_desktop_runtime_facade" (Test-ContainsAll $apiText @("desktopRuntimeListen", "onBrowserInstanceRuntimeEvents", "BROWSER_INSTANCE_RUNTIME_EVENT_NAMES", "normalizeBrowserRuntimeEventPayload")) "src/modules/browser/api.ts must expose browser runtime subscriptions through desktopRuntimeListen"),
  (New-GateResult "browser_api_no_direct_runtime_import" (Test-ContainsNone $apiText @("import { EventsOn } from '../../wailsjs/runtime'", "EventsOn(")) "src/modules/browser/api.ts must not import or call raw Wails EventsOn for browser runtime events"),
  (New-GateResult "browser_runtime_event_types_exported" (Test-ContainsAll $typesText @("BrowserInstanceRuntimeEventName", "BrowserInstanceRuntimeEvent", "BrowserRuntimeEventPayload", "rawPayload")) "src/modules/browser/types.ts must define typed browser instance runtime event contracts"),
  (New-GateResult "browser_pages_use_browser_runtime_facade" ((Test-ContainsAll $listText @("onBrowserInstanceRuntimeEvents")) -and (Test-ContainsAll $detailText @("onBrowserInstanceRuntimeEvents"))) "Browser list/detail pages must subscribe via src/modules/browser/api.ts facade"),
  (New-GateResult "browser_pages_no_direct_runtime_events" ((Test-ContainsNone $listText $directRuntimeMarkers) -and (Test-ContainsNone $detailText $directRuntimeMarkers)) "Browser list/detail pages must not import Wails runtime or call EventsOn directly"),
  (New-GateResult "dashboard_evidence_row_registered" (Test-ContainsAll $dashboardPageText @("m4_browser_runtime_facade", "M4 Browser Runtime")) "Dashboard must list the M4 browser runtime facade report row"),
  (New-GateResult "m4_gate_browser_runtime_contract_registered" (Test-ContainsAll $m4GateText @("browser_runtime_facade_contract", "Test-BrowserRuntimeFacadeContract")) "M4 acceptance gate must include the browser runtime facade source contract")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_browser_runtime_facade_contract" } else { "failed_browser_runtime_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "Browser runtime facade contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_browser_runtime_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    browserRuntimeFacadeUsage = if (($checks | Where-Object { $_.id -eq "browser_pages_use_browser_runtime_facade" }).status -eq "passed") { "present" } else { "missing" }
    directRuntimeImportsRemoved = if (($checks | Where-Object { $_.id -eq "browser_pages_no_direct_runtime_events" }).status -eq "passed") { "yes" } else { "no" }
    desktopRuntimeWrapper = if (($checks | Where-Object { $_.id -eq "browser_api_uses_desktop_runtime_facade" }).status -eq "passed") { "present" } else { "missing" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining settings/core/proxy runtime bridge APIs without claiming tauriWailsBridge removal."
    } else {
      "Route Browser List/Detail runtime subscriptions through src/modules/browser/api.ts and desktopRuntimeListen, remove direct EventsOn imports, and rerun scripts/m4_browser_runtime_facade_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates Browser List/Detail runtime subscription source-level facade usage only.",
    "It does not prove tauriWailsBridge removal, full settings/core/proxy runtime facade closure, provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full 450 observed/replay coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-browser-runtime-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 browser runtime facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
