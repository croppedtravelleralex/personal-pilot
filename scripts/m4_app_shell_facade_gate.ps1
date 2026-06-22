param(
  [string]$OutputDir = "data/reports/m4-app-shell-facade"
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

$appPath = Join-Path $projectRoot "src/App.tsx"
$desktopServicePath = Join-Path $projectRoot "src/services/desktop.ts"
$dashboardPagePath = Join-Path $projectRoot "src/modules/dashboard/DashboardPage.tsx"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"

$appText = Read-Text $appPath
$desktopServiceText = Read-Text $desktopServicePath
$dashboardPageText = Read-Text $dashboardPagePath
$m4GateText = Read-Text $m4GatePath

$checks = @(
  (New-GateResult "app_shell_uses_desktop_facade" (Test-ContainsAll $appText @("desktopRuntimeListen", "desktopEnvironment", "desktopQuitAppOnly", "desktopQuitFull", "desktopQuit()", "desktopWindowHide", "desktopWindowMinimize")) "App shell close, notification, environment, quit, and tray actions must call src/services/desktop.ts wrappers"),
  (New-GateResult "app_shell_no_direct_wails_imports" (Test-ContainsNone $appText @("./wailsjs/go/main/App", "./wailsjs/runtime/runtime", "ForceQuit as ForceQuitApp", "QuitAppOnly as QuitAppOnlyApp", "Environment, Quit", "(window as any).runtime")) "App shell must not import Wails App/runtime modules or read window.runtime directly"),
  (New-GateResult "desktop_service_app_shell_wrappers_present" (Test-ContainsAll $desktopServiceText @("export function desktopRuntimeListen", "export function desktopEnvironment", "export function desktopQuit", "export function desktopQuitAppOnly", "export function desktopQuitFull", "export function desktopWindowHide", "export function desktopWindowMinimize")) "src/services/desktop.ts must expose typed app shell wrappers"),
  (New-GateResult "dashboard_evidence_row_registered" (Test-ContainsAll $dashboardPageText @("m4_app_shell_facade", "M4 App Shell")) "Dashboard must list the M4 app shell facade report row"),
  (New-GateResult "m4_gate_app_shell_contract_registered" (Test-ContainsAll $m4GateText @("app_shell_facade_contract", "Test-AppShellFacadeContract")) "M4 acceptance gate must include the app shell facade source contract")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_app_shell_facade_contract" } else { "failed_app_shell_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "App shell facade contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_app_shell_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    appShellFacadeUsage = if (($checks | Where-Object { $_.id -eq "app_shell_uses_desktop_facade" }).status -eq "passed") { "present" } else { "missing" }
    directWailsImportsRemoved = if (($checks | Where-Object { $_.id -eq "app_shell_no_direct_wails_imports" }).status -eq "passed") { "yes" } else { "no" }
    desktopWrappers = if (($checks | Where-Object { $_.id -eq "desktop_service_app_shell_wrappers_present" }).status -eq "passed") { "present" } else { "missing" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining monitor/browser runtime bridge APIs without claiming tauriWailsBridge removal."
    } else {
      "Route App.tsx shell runtime usage through src/services/desktop.ts, remove direct Wails App/runtime imports, and rerun scripts/m4_app_shell_facade_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates App.tsx app shell source-level facade usage only.",
    "It does not prove tauriWailsBridge removal, full monitor/browser/workbench/core facade closure, provider credentials, remote proxy/TLS, SessionBundle local restore beyond its own gate, AdsPower refresh, or full 450 observed/replay coverage."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-app-shell-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 app shell facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
