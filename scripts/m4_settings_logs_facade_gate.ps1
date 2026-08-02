param(
  [string]$OutputDir = "data/reports/m4-settings-logs-facade"
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

$settingsApiPath = Join-Path $projectRoot "src/modules/settings/api.ts"
$logsPagePath = Join-Path $projectRoot "src/modules/browser/pages/BrowserLogsPage.tsx"
$desktopServicePath = Join-Path $projectRoot "src/services/desktop.ts"
$bridgePath = Join-Path $projectRoot "src/services/tauriWailsBridge.ts"
$dashboardPagePath = Join-Path $projectRoot "src/modules/dashboard/DashboardPage.tsx"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"

$settingsApiText = Read-Text $settingsApiPath
$logsPageText = Read-Text $logsPagePath
$desktopServiceText = Read-Text $desktopServicePath
$bridgeText = Read-Text $bridgePath
$dashboardPageText = Read-Text $dashboardPagePath
$m4GateText = Read-Text $m4GatePath

$checks = @(
  (New-GateResult "settings_api_uses_typed_backup_wrappers" (Test-ContainsAll $settingsApiText @("initializeSystemDataFromDesktop", "exportSystemConfigFromDesktop", "importSystemConfigFromDesktop", "DesktopBackupActionResult")) "src/modules/settings/api.ts must call typed desktop backup wrappers"),
  (New-GateResult "settings_api_no_dynamic_wails_binding" (Test-ContainsNone $settingsApiText @("const bindings: any", "import('../../wailsjs/go/main/App')", "bindings.", "getBindings")) "src/modules/settings/api.ts must not dynamically import Wails bindings"),
  (New-GateResult "logs_page_uses_typed_wrappers" (Test-ContainsAll $logsPageText @("getAppLogs", "clearAppLogs", "DesktopJsonValue")) "BrowserLogsPage must call typed desktop log wrappers"),
  (New-GateResult "logs_page_no_dynamic_wails_binding" (Test-ContainsNone $logsPageText @("const bindings: any", "import('../../../wailsjs/go/main/App')", "bindings.")) "BrowserLogsPage must not dynamically import Wails bindings"),
  (New-GateResult "desktop_service_settings_logs_wrappers_present" (Test-ContainsAll $desktopServiceText @("DesktopBackupActionResult", "DesktopDestructivePreflight", "initializeSystemData", "exportSystemConfig", "importSystemConfig", "getAppLogs", "clearAppLogs", "confirmDestructivePreflight")) "src/services/desktop.ts must expose typed settings backup and log wrappers"),
  (New-GateResult "bridge_compatibility_proxy_preserved" (Test-ContainsAll $bridgeText @("BackupInitializeSystem", "BackupExportPackage", "BackupImportPackage", "GetAppLogs", "ClearAppLogs")) "tauriWailsBridge must keep Wails-compatible proxy methods while reusing typed wrappers"),
  (New-GateResult "dashboard_evidence_row_registered" (Test-ContainsAll $dashboardPageText @("m4_settings_logs_facade", "M4 Settings/Logs")) "Dashboard must list the M4 settings/logs facade report row"),
  (New-GateResult "m4_gate_settings_logs_contract_registered" (Test-ContainsAll $m4GateText @("settings_logs_facade_contract", "Test-SettingsLogsFacadeContract")) "M4 acceptance gate must include the settings/logs facade source contract")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_settings_logs_facade_contract" } else { "failed_settings_logs_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "Settings/logs facade contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_settings_logs_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    settingsBackupWrappers = if (($checks | Where-Object { $_.id -eq "settings_api_uses_typed_backup_wrappers" }).status -eq "passed") { "present" } else { "missing" }
    logWrappers = if (($checks | Where-Object { $_.id -eq "logs_page_uses_typed_wrappers" }).status -eq "passed") { "present" } else { "missing" }
    dynamicBindingRemoved = if (
      (($checks | Where-Object { $_.id -eq "settings_api_no_dynamic_wails_binding" }).status -eq "passed") -and
      (($checks | Where-Object { $_.id -eq "logs_page_no_dynamic_wails_binding" }).status -eq "passed")
    ) { "yes" } else { "no" }
    bridgeCompatibilityProxy = if (($checks | Where-Object { $_.id -eq "bridge_compatibility_proxy_preserved" }).status -eq "passed") { "preserved" } else { "missing" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining profile/workbench/core bridge APIs without claiming tauriWailsBridge removal."
    } else {
      "Route Settings backup and Browser logs through src/services/desktop.ts typed wrappers, remove raw dynamic imports, and rerun scripts/m4_settings_logs_facade_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates Settings backup API and Browser logs source-level typed facade usage only.",
    "It preserves the transitional tauriWailsBridge compatibility proxy and does not prove full bridge removal or profile/workbench/core facade closure; CAPTCHA/SMS/Email credential-backed provider smoke remains separate."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-settings-logs-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 settings/logs facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
