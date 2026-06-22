param(
  [string]$OutputDir = "data/reports/m4-browser-payload-schema"
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

$projectRoot = Resolve-ProjectRoot
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$apiPath = Join-Path $projectRoot "src/modules/browser/api.ts"
$typesPath = Join-Path $projectRoot "src/modules/browser/types.ts"
$listPagePath = Join-Path $projectRoot "src/modules/browser/pages/BrowserListPage.tsx"
$detailPagePath = Join-Path $projectRoot "src/modules/browser/pages/BrowserDetailPage.tsx"

$apiText = Read-Text $apiPath
$typesText = Read-Text $typesPath
$listText = Read-Text $listPagePath
$detailText = Read-Text $detailPagePath

$checks = @(
  (New-GateResult "browser_runtime_event_payload_type_exported" (Test-ContainsAll $typesText @("BrowserRuntimeEventPayload", "profileId", "error")) "src/modules/browser/types.ts must define a shared BrowserRuntimeEventPayload contract"),
  (New-GateResult "browser_runtime_event_normalizer_exported" (Test-ContainsAll $apiText @("normalizeBrowserRuntimeEventPayload", "BrowserRuntimeEventPayload", "profile_id", "lastError")) "src/modules/browser/api.ts must export a runtime event payload normalizer with camel/snake compatibility"),
  (New-GateResult "browser_pages_use_normalizer" ((Test-ContainsAll $listText @("normalizeBrowserRuntimeEventPayload")) -and (Test-ContainsAll $detailText @("normalizeBrowserRuntimeEventPayload"))) "Browser list/detail runtime event handlers must use the shared normalizer"),
  (New-GateResult "browser_pages_no_event_any_payload" (($listText.IndexOf("payload: any", [System.StringComparison]::OrdinalIgnoreCase) -lt 0) -and ($detailText.IndexOf("payload: any", [System.StringComparison]::OrdinalIgnoreCase) -lt 0)) "Browser list/detail runtime event handlers must not use payload:any"),
  (New-GateResult "browser_api_no_any_normalizer_inputs" (($apiText.IndexOf("normalizeLaunchServerInfo(payload: any)", [System.StringComparison]::OrdinalIgnoreCase) -lt 0) -and ($apiText.IndexOf("normalizeRecordingDetail(payload: any", [System.StringComparison]::OrdinalIgnoreCase) -lt 0)) "browser api normalizers must accept unknown/typed inputs instead of any")
)

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed_browser_payload_schema_contract" } else { "failed_browser_payload_schema_contract" }
$failureReason = if ($failed.Count -eq 0) { "" } else { "Browser payload schema contract failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }

$report = [ordered]@{
  schemaVersion = "m4_browser_payload_schema_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  failureReason = $failureReason
  checks = $checks
  summary = [ordered]@{
    failed = $failed.Count
    normalizedEventPayload = if (($checks | Where-Object { $_.id -eq "browser_runtime_event_normalizer_exported" }).status -eq "passed") { "present" } else { "missing" }
    pageUsage = if (($checks | Where-Object { $_.id -eq "browser_pages_use_normalizer" }).status -eq "passed") { "present" } else { "missing" }
    anyPayloadRemoved = if (($checks | Where-Object { $_.id -eq "browser_pages_no_event_any_payload" }).status -eq "passed") { "yes" } else { "no" }
    nextAction = if ($failed.Count -eq 0) {
      "Continue shrinking remaining low-frequency workbench/core bridge payloads without claiming tauriWailsBridge removal."
    } else {
      "Add shared browser payload types and normalizers, replace browser page any payload handlers, then rerun scripts/m4_browser_payload_schema_gate.ps1."
    }
  }
  liveTruthBoundary = @(
    "This gate validates browser module source-level payload schema normalization only.",
    "It does not prove CAPTCHA/SMS/Email credential-backed provider smoke.",
    "It does not remove every tauriWailsBridge/core bridge compatibility path."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-browser-payload-schema-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "M4 browser payload schema gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($failed.Count -gt 0) { exit 1 }
exit 0
