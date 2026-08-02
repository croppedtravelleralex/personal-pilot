param(
  [string]$OutputDir = "data/reports/m4-profile-facade"
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

$profileApiPath = Join-Path $projectRoot "src/modules/profile/api.ts"
$desktopServicePath = Join-Path $projectRoot "src/services/desktop.ts"
$dashboardPagePath = Join-Path $projectRoot "src/modules/dashboard/DashboardPage.tsx"
$m4GatePath = Join-Path $projectRoot "scripts/m4_acceptance_gate.ps1"

$profileApiText = Read-Text $profileApiPath
$desktopServiceText = Read-Text $desktopServicePath
$dashboardPageText = Read-Text $dashboardPagePath
$m4GateText = Read-Text $m4GatePath

$checks = @(
  (New-GateResult "profile_api_uses_typed_desktop_wrapper" (Test-ContainsAll $profileApiText @("fetchRemoteAuthorProfileFromDesktop", "Record<string, unknown>", "normalizeChannel(value: unknown)", "errorMessage(error)")) "src/modules/profile/api.ts must use the typed desktop profile wrapper and unknown payload guards"),
  (New-GateResult "profile_api_no_dynamic_wails_binding" (Test-ContainsNone $profileApiText @("const bindings: any", "import('../../wailsjs/go/main/App')", "getBindings", "(window as any).go?.main?.App", "Record<string, any>", "normalizeChannel(value: any)")) "src/modules/profile/api.ts must not dynamically import Wails bindings or use any payload records"),
  (New-GateResult "profile_browser_fetch_fallback_preserved" (Test-ContainsAll $profileApiText @("fetchRemoteAuthorPayloadViaBrowser", "fetch(authorURL", "AbortController", "Accept: 'application/json'")) "profile API must keep browser fetch fallback for non-desktop previews"),
  (New-GateResult "desktop_service_profile_wrapper_present" (Test-ContainsAll $desktopServiceText @("fetchRemoteAuthorProfileFromDesktop", "FetchRemoteAuthorProfile", "Promise<Record<string, unknown>>")) "src/services/desktop.ts must expose the remote author profile typed wrapper"),
  (New-GateResult "dashboard_evidence_row_registered" (Test-ContainsAll $dashboardPageText @("m4_profile_facade", "M4 Profile")) "Dashboard must list the M4 profile facade report row"),
  (New-GateResult "m4_gate_profile_contract_registered" (Test-ContainsAll $m4GateText @("profile_facade_contract", "Test-ProfileFacadeContract")) "M4 acceptance gate must include the profile facade source contract")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_profile_facade_contract" } else { "failed_profile_facade_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "Profile facade contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_profile_facade_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    typedDesktopWrapper = if (($checks | Where-Object { $_.id -eq "profile_api_uses_typed_desktop_wrapper" }).status -eq "passed") { "present" } else { "missing" }
    browserFetchFallback = if (($checks | Where-Object { $_.id -eq "profile_browser_fetch_fallback_preserved" }).status -eq "passed") { "preserved" } else { "missing" }
    dynamicBindingRemoved = if (($checks | Where-Object { $_.id -eq "profile_api_no_dynamic_wails_binding" }).status -eq "passed") { "yes" } else { "no" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining workbench/core bridge APIs without claiming tauriWailsBridge removal."
    } else {
      "Route profile remote author loading through src/services/desktop.ts, keep browser fallback, remove raw Wails dynamic imports, and rerun scripts/m4_profile_facade_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates Profile remote author source-level typed facade usage only.",
    "It preserves the browser fetch fallback for non-desktop previews and does not prove tauriWailsBridge removal or workbench/core facade closure; CAPTCHA/SMS/Email credential-backed provider smoke remains separate."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-profile-facade-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 profile facade gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
