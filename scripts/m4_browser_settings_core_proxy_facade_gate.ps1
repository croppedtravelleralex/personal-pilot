param(
  [string]$OutputDir = "data/reports/m4-browser-settings-core-proxy-facade"
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

function Test-ContainsAll([string]$Text, [string[]]$Needles) {
  if ($null -eq $Text) { return $false }
  foreach ($needle in $Needles) {
    if (-not $Text.Contains($needle)) { return $false }
  }
  return $true
}

function Test-ContainsNone([string]$Text, [string[]]$Needles) {
  if ($null -eq $Text) { return $false }
  foreach ($needle in $Needles) {
    if ($Text.Contains($needle)) { return $false }
  }
  return $true
}

function Get-Section([string]$Text, [string]$StartMarker, [string]$EndMarker) {
  $start = $Text.IndexOf($StartMarker, [System.StringComparison]::Ordinal)
  if ($start -lt 0) { return "" }
  $end = $Text.IndexOf($EndMarker, $start + $StartMarker.Length, [System.StringComparison]::Ordinal)
  if ($end -lt 0) { return $Text.Substring($start) }
  return $Text.Substring($start, $end - $start)
}

$projectRoot = Resolve-ProjectRoot
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$apiPath = Join-Path $projectRoot "src\modules\browser\api.ts"
$desktopPath = Join-Path $projectRoot "src\services\desktop.ts"
$bridgePath = Join-Path $projectRoot "src\services\tauriWailsBridge.ts"
$dashboardPath = Join-Path $projectRoot "src\modules\dashboard\DashboardPage.tsx"

$apiText = if (Test-Path $apiPath) { Get-Content -LiteralPath $apiPath -Raw -Encoding UTF8 } else { "" }
$desktopText = if (Test-Path $desktopPath) { Get-Content -LiteralPath $desktopPath -Raw -Encoding UTF8 } else { "" }
$bridgeText = if (Test-Path $bridgePath) { Get-Content -LiteralPath $bridgePath -Raw -Encoding UTF8 } else { "" }
$dashboardText = if (Test-Path $dashboardPath) { Get-Content -LiteralPath $dashboardPath -Raw -Encoding UTF8 } else { "" }

$settingsSection = Get-Section $apiText "// Settings API" "// Core API"
$coreSection = Get-Section $apiText "// Core API" "// Proxy API"
$proxySection = Get-Section $apiText "// Proxy API" "// Cookie API"
$browserBindingBlock = Get-Section $apiText "type BrowserNativeBindings = Partial<{" "}>"

$desktopWrapperMarkers = @(
  "readBrowserSettingsFromDesktop",
  "saveBrowserSettingsFromDesktop",
  "listBrowserCoresFromDesktop",
  "saveBrowserCoreFromDesktop",
  "deleteBrowserCoreFromDesktop",
  "setDefaultBrowserCoreFromDesktop",
  "validateBrowserCoreForKindFromDesktop",
  "listBrowserProxiesFromDesktop",
  "saveBrowserProxiesFromDesktop",
  "browserProxyCheckIPHealthFromDesktop",
  "openUserDataDirFromDesktop",
  "openCorePathFromDesktop"
)

$apiWrapperMarkers = @(
  "tryDesktop(() => readBrowserSettingsFromDesktop())",
  "tryDesktopVoid(() => saveBrowserSettingsFromDesktop(settings))",
  "tryDesktop(() => listBrowserCoresFromDesktop())",
  "tryDesktopVoid(() => saveBrowserCoreFromDesktop(input))",
  "tryDesktop(() => listBrowserProxiesFromDesktop())",
  "tryDesktopVoid(() => saveBrowserProxiesFromDesktop(proxies))",
  "tryDesktop(() => browserProxyCheckIPHealthFromDesktop(proxyId))"
)

$retiredBindingMarkers = @(
  "GetBrowserSettings:",
  "SaveBrowserSettings:",
  "BrowserCoreList:",
  "BrowserCoreSave:",
  "BrowserCoreDelete:",
  "BrowserCoreSetDefault:",
  "BrowserCoreValidateForKind:",
  "BrowserCoreValidate:",
  "BrowserCoreExtendedInfo:",
  "BrowserCoreScan:",
  "BrowserCoreDownload:",
  "BrowserProxyList:",
  "BrowserProxyListGroups:",
  "BrowserProxyListByGroup:",
  "SaveBrowserProxies:",
  "ValidateProxyConfig:",
  "TestProxyConnectivity:",
  "TestProxyRealConnectivity:",
  "BrowserProxyTestSpeed:",
  "BrowserProxyBatchTestSpeed:",
  "BrowserProxyCheckIPHealth:",
  "BrowserProxyBatchCheckIPHealth:",
  "OpenUserDataDir:",
  "OpenCorePath:"
)

$checks = @(
  (New-GateResult "desktop_settings_core_proxy_wrappers_present" (Test-ContainsAll $desktopText $desktopWrapperMarkers) "src/services/desktop.ts must expose named settings/core/proxy wrappers"),
  (New-GateResult "browser_api_uses_desktop_wrappers" (Test-ContainsAll $apiText $apiWrapperMarkers) "src/modules/browser/api.ts must call the named desktop wrappers"),
  (New-GateResult "settings_section_no_dynamic_binding" (Test-ContainsNone $settingsSection @("getBindings()", "bindings?.")) "Settings section must not use dynamic Wails bindings"),
  (New-GateResult "core_section_no_dynamic_binding" (Test-ContainsNone $coreSection @("getBindings()", "bindings?.")) "Core section must not use dynamic Wails bindings"),
  (New-GateResult "proxy_section_no_dynamic_binding" (Test-ContainsNone $proxySection @("getBindings()", "bindings?.")) "Proxy section must not use dynamic Wails bindings"),
  (New-GateResult "browser_native_bindings_shrunk" (Test-ContainsNone $browserBindingBlock $retiredBindingMarkers) "BrowserNativeBindings must not advertise settings/core/proxy Wails methods"),
  (New-GateResult "tauri_wails_bridge_explicit_allowlist" (Test-ContainsAll $bridgeText @("const BRIDGE_RPC_METHOD_NAMES = new Set<string>", "BRIDGE_RPC_METHOD_NAMES.has(property)", "if (property === 'then') return undefined", "return undefined")) "tauriWailsBridge must use an explicit App RPC allowlist instead of forwarding every property"),
  (New-GateResult "dashboard_evidence_row_present" (Test-ContainsAll $dashboardText @("m4_browser_settings_core_proxy_facade", "M4 Browser Facade")) "Dashboard evidence list must include the new facade report kind")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_browser_settings_core_proxy_facade_contract" } else { "failed_browser_settings_core_proxy_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "settings/core/proxy browser facade source contract is incomplete" }

$report = [ordered]@{
  schemaVersion = "m4_browser_settings_core_proxy_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    desktopWrappers = if (Test-ContainsAll $desktopText $desktopWrapperMarkers) { "present" } else { "missing" }
    browserApiFacadeUsage = if (Test-ContainsAll $apiText $apiWrapperMarkers) { "present" } else { "missing" }
    dynamicBindingsRemoved = if (
      (Test-ContainsNone $settingsSection @("getBindings()", "bindings?.")) -and
      (Test-ContainsNone $coreSection @("getBindings()", "bindings?.")) -and
      (Test-ContainsNone $proxySection @("getBindings()", "bindings?."))
    ) { "yes" } else { "no" }
    bridgeAllowlist = if ($bridgeText.Contains("BRIDGE_RPC_METHOD_NAMES.has(property)")) { "present" } else { "missing" }
    nextAction = if ($failed.Count -eq 0) {
      "Keep shrinking remaining browser/profile/session bridge calls in small source-gated batches; do not claim tauriWailsBridge removal yet."
    } else {
      "Move remaining settings/core/proxy calls to desktop wrappers and rerun this gate."
    }
  }
  failureReason = $failureReason
  liveTruthBoundary = @(
    "This is source-level M4 facade evidence for browser settings/core/proxy calls.",
    "The transitional tauriWailsBridge still exists, but its App RPC surface is now explicitly allowlisted.",
    "This does not prove provider credentials, remote proxy/TLS, full headed runtime, or full 450 observed/replay coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-browser-settings-core-proxy-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $reportPath -Encoding UTF8

Write-Host "M4 browser settings/core/proxy facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "passed_browser_settings_core_proxy_facade_contract") { exit 0 }
exit 1
