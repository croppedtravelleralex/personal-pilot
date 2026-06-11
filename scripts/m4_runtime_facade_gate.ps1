param(
  [string]$OutputDir = "data/reports/m4-runtime-facade"
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

$desktopServicePath = Join-Path $projectRoot "src/services/desktop.ts"
$settingsPagePath = Join-Path $projectRoot "src/modules/settings/SettingsPage.tsx"
$corePagePath = Join-Path $projectRoot "src/modules/browser/pages/CoreManagementPage.tsx"
$proxyPickerPath = Join-Path $projectRoot "src/modules/browser/components/ProxyPickerModal.tsx"
$proxyPoolPath = Join-Path $projectRoot "src/modules/browser/pages/ProxyPoolPage.tsx"
$launchDocsPath = Join-Path $projectRoot "src/modules/browser/pages/LaunchApiDocsPage.tsx"
$tutorialPath = Join-Path $projectRoot "src/modules/browser/pages/UsageTutorialPage.tsx"
$dashboardPagePath = Join-Path $projectRoot "src/modules/dashboard/DashboardPage.tsx"
$desktopRustPath = Join-Path $projectRoot "src/desktop/mod.rs"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"

$desktopServiceText = Read-Text $desktopServicePath
$settingsText = Read-Text $settingsPagePath
$coreText = Read-Text $corePagePath
$proxyPickerText = Read-Text $proxyPickerPath
$proxyPoolText = Read-Text $proxyPoolPath
$launchDocsText = Read-Text $launchDocsPath
$tutorialText = Read-Text $tutorialPath
$dashboardText = Read-Text $dashboardPagePath
$desktopRustText = Read-Text $desktopRustPath
$m4GateText = Read-Text $m4GatePath

$directRuntimeMarkers = @(
  "wailsjs/runtime/runtime",
  "import { EventsOn",
  "import { EventsOff",
  "import { BrowserOpenURL",
  "EventsOn(",
  "EventsOff(",
  "BrowserOpenURL("
)

$runtimePageTexts = @(
  [pscustomobject]@{ name = "SettingsPage"; text = $settingsText },
  [pscustomobject]@{ name = "CoreManagementPage"; text = $coreText },
  [pscustomobject]@{ name = "ProxyPickerModal"; text = $proxyPickerText },
  [pscustomobject]@{ name = "ProxyPoolPage"; text = $proxyPoolText },
  [pscustomobject]@{ name = "LaunchApiDocsPage"; text = $launchDocsText },
  [pscustomobject]@{ name = "UsageTutorialPage"; text = $tutorialText }
)

$runtimePagesHaveNoRawImports = $true
foreach ($item in $runtimePageTexts) {
  if (-not (Test-ContainsNone $item.text $directRuntimeMarkers)) {
    $runtimePagesHaveNoRawImports = $false
  }
}

$checks = @(
  (New-GateResult "desktop_runtime_facade_wrappers_present" (Test-ContainsAll $desktopServiceText @("export function desktopRuntimeListen", "export function desktopOpenExternalUrl", "desktopWindow.runtime?.EventsOn", "desktopWindow.runtime?.BrowserOpenURL")) "src/services/desktop.ts must expose runtime event and external URL wrappers"),
  (New-GateResult "settings_runtime_progress_uses_facade" ((Test-ContainsAll $settingsText @("desktopRuntimeListen<BackupExportProgress>", "backup:export:progress", "backup:import:progress")) -and (Test-ContainsNone $settingsText $directRuntimeMarkers)) "Settings backup import/export progress must use desktopRuntimeListen and no raw Wails runtime imports"),
  (New-GateResult "core_runtime_progress_and_url_use_facade" ((Test-ContainsAll $coreText @("desktopRuntimeListen", "download:progress", "desktopOpenExternalUrl")) -and (Test-ContainsNone $coreText $directRuntimeMarkers)) "Core management download progress and external release links must use desktop facade wrappers"),
  (New-GateResult "proxy_runtime_events_use_facade" ((Test-ContainsAll $proxyPickerText @("desktopRuntimeListen", "proxy:speed:result")) -and (Test-ContainsAll $proxyPoolText @("desktopRuntimeListen", "proxy:speed:result", "proxy:iphealth:result")) -and (Test-ContainsNone $proxyPickerText $directRuntimeMarkers) -and (Test-ContainsNone $proxyPoolText $directRuntimeMarkers)) "Proxy picker and proxy pool runtime result events must use desktopRuntimeListen"),
  (New-GateResult "docs_external_links_use_facade" ((Test-ContainsAll $launchDocsText @("desktopOpenExternalUrl")) -and (Test-ContainsAll $tutorialText @("desktopOpenExternalUrl")) -and (Test-ContainsNone $launchDocsText $directRuntimeMarkers) -and (Test-ContainsNone $tutorialText $directRuntimeMarkers)) "Launch API docs and tutorial external links must use desktopOpenExternalUrl"),
  (New-GateResult "runtime_pages_no_direct_wails_runtime_imports" $runtimePagesHaveNoRawImports "Runtime-facing pages must not import wailsjs/runtime/runtime or call raw EventsOn/EventsOff/BrowserOpenURL"),
  (New-GateResult "dashboard_evidence_row_registered" (Test-ContainsAll $dashboardText @("m4_runtime_facade", "M4 Runtime")) "Dashboard must list the M4 runtime facade report row"),
  (New-GateResult "desktop_evidence_history_registered" (Test-ContainsAll $desktopRustText @("m4-runtime-facade", "m4_runtime_facade", "passed_runtime_facade_contract", "runtimeFacadeUsage")) "Desktop evidence history must read and summarize M4 runtime facade reports"),
  (New-GateResult "m4_gate_runtime_contract_registered" (Test-ContainsAll $m4GateText @("runtime_facade_contract", "Test-RuntimeFacadeContract")) "M4 acceptance gate must include the runtime facade source contract")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_runtime_facade_contract" } else { "failed_runtime_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "Runtime facade contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_runtime_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    runtimeFacadeUsage = if ($runtimePagesHaveNoRawImports) { "present" } else { "missing" }
    settingsRuntimeEvents = if (($checks | Where-Object { $_.id -eq "settings_runtime_progress_uses_facade" }).status -eq "passed") { "present" } else { "missing" }
    coreRuntimeEvents = if (($checks | Where-Object { $_.id -eq "core_runtime_progress_and_url_use_facade" }).status -eq "passed") { "present" } else { "missing" }
    proxyRuntimeEvents = if (($checks | Where-Object { $_.id -eq "proxy_runtime_events_use_facade" }).status -eq "passed") { "present" } else { "missing" }
    externalUrlWrapper = if (($checks | Where-Object { $_.id -eq "docs_external_links_use_facade" }).status -eq "passed") { "present" } else { "missing" }
    directRuntimeImportsRemoved = if ($runtimePagesHaveNoRawImports) { "yes" } else { "no" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining core/proxy/settings bridge APIs and tauriWailsBridge compatibility paths separately."
    } else {
      "Route page-level runtime EventsOn/EventsOff/BrowserOpenURL usage through src/services/desktop.ts, register evidence history, and rerun scripts/m4_runtime_facade_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates selected page-level runtime EventsOn/EventsOff/BrowserOpenURL source-level facade usage only.",
    "It does not prove tauriWailsBridge removal, every core/proxy/settings API closure, real provider credentials, remote proxy/TLS, cross-machine SessionBundle, AdsPower refresh, or full 450 observed/replay coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-runtime-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 runtime facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
