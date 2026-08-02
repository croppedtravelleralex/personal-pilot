# Full local workflow smoke (tab-bound).
# Creates a temporary profile, starts headed Chromium, opens a dedicated tab,
# navigates/scrolls/fingerprints on that tab, then cleans up.
param(
  [int]$ReadyTimeoutSec = 120,
  [string]$NavigateUrl = "https://example.com/",
  [switch]$KeepInstance
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")

$root = Get-CapabilityProjectRoot
Set-Location $root
$env:PERSONAL_PILOT_APP_ROOT = $root

$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$runId = "wf-smoke-$stamp"
$reportDir = Join-Path $root "data/reports/full-workflow-smoke"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null
$reportPath = Join-Path $reportDir "full-workflow-smoke-$stamp.json"

$checks = New-Object System.Collections.Generic.List[object]
function Add-Check([string]$id, [bool]$ok, [string]$detail) {
  $checks.Add([ordered]@{ id = $id; ok = $ok; detail = $detail }) | Out-Null
  $tone = if ($ok) { "Green" } else { "Red" }
  Write-Host ("[{0}] {1}: {2}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $id, $detail) -ForegroundColor $tone
}

Write-Host "=== Full Workflow Smoke (tab-bound) ===" -ForegroundColor Cyan

$old = Get-CimInstance Win32_Process -Filter "Name='personal-pilot-core.exe'" -ErrorAction SilentlyContinue |
  Where-Object { $_.CommandLine -like "*$root*" }
foreach ($p in @($old)) {
  Write-Host "Stopping prior core pid=$($p.ProcessId)" -ForegroundColor Yellow
  Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Seconds 1

$harness = $null
$profileId = ""
$lastPid = 0
$boundTabId = ""
try {
  $harness = Get-OrStartCapabilityHarness -Root $root -ReadyTimeoutSec 90 -ForceNewCore
  Add-Check "core_ready" ($null -ne $harness -and $harness.BridgeUrl -ne "") ("bridge=$($harness.BridgeUrl)")

  $cores = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserCoreList" -RpcArgList @())
  $core = @($cores) | Where-Object { $_.isDefault -eq $true } | Select-Object -First 1
  if (-not $core) { $core = @($cores) | Select-Object -First 1 }
  Add-Check "core_available" ($null -ne $core -and $core.coreId) ("coreId=$($core.coreId)")

  $profileInput = @{
    profileName       = "wf-smoke-$runId"
    userDataDir       = "browser/user-data/wf-smoke-$runId"
    coreId            = [string]$core.coreId
    fingerprintArgs   = @()
    launchArgs        = @("--window-size=1280,800", "--disable-popup-blocking")
    tags              = @("workflow-smoke", $runId, "skip-host-resolver-rules")
    keywords          = @($runId)
    humanizeSeed      = "wf-smoke-$runId"
    behaviorProfileId = ""
    proxyConfig       = ""
    proxyId           = ""
  }
  $created = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserProfileCreate" -RpcArgList @($profileInput)
  $profileId = [string]$created.profileId
  if (-not $profileId) { $profileId = [string]$created.ProfileId }
  Add-Check "profile_create" (-not [string]::IsNullOrWhiteSpace($profileId)) ("profileId=$profileId")

  $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStart" -RpcArgList @($profileId) -TimeoutSec 240
  $ready = $false
  $status = $null
  $deadline = (Get-Date).AddSeconds($ReadyTimeoutSec)
  while ((Get-Date) -lt $deadline) {
    $status = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStatus" -RpcArgList @($profileId)
    if ($status.running -and $status.debugReady -and [int]$status.debugPort -gt 0 -and $status.injectionReady) {
      $ready = $true
      $lastPid = [int]$status.pid
      break
    }
    Start-Sleep -Seconds 2
  }
  Add-Check "instance_ready" $ready ("pid=$($status.pid) port=$($status.debugPort) injectionReady=$($status.injectionReady)")

  if ($ready) {
    $boundTabId = [string](Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchNewTab" -RpcArgList @($profileId, $NavigateUrl) -TimeoutSec 60)
    Add-Check "tab_create" (-not [string]::IsNullOrWhiteSpace($boundTabId)) ("tabId=$boundTabId")
    if ($boundTabId) {
      $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchSwitchTab" -RpcArgList @($profileId, $boundTabId) -TimeoutSec 30
    }

    $actions = @(
      @{ type = "navigate"; url = $NavigateUrl; tabId = $boundTabId; postWaitMs = 2000; humanizationLevel = "low" },
      @{ type = "scroll"; distance = 300; tabId = $boundTabId; humanizationLevel = "low" }
    )
    $actionRes = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchExecuteActions" -RpcArgList @($profileId, $actions) -TimeoutSec 90)
    $actionsOk = $true
    $detailParts = @()
    foreach ($r in $actionRes) {
      if ($r.ok -eq $false) { $actionsOk = $false }
      $detailParts += ("{0}:{1}" -f $r.type, $(if ($r.ok) { "ok" } else { $r.error }))
    }
    Add-Check "tab_bound_actions" $actionsOk ("count=$(@($actionRes).Count) $($detailParts -join '; ')")

    $tabs = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchListTabs" -RpcArgList @($profileId) -TimeoutSec 30)
    # Match the actual bound tab (tabId) OR the URL whose host equals the navigation target's host.
    # Do NOT treat any new https tab as a target — that would pass with unrelated tabs.
    $targetHost = ([uri]$NavigateUrl).Host
    $expectedUrl = [regex]::Escape($NavigateUrl)
    $matched = @($tabs | Where-Object {
      [string]$_.tabId -eq $boundTabId -or [string]$_.url -match ("^" + $expectedUrl) -or [string]$_.url -match ("//" + [regex]::Escape($targetHost) + "([/?#]|$)")
    })
    Add-Check "tab_list_contains_target" ($matched.Count -gt 0) ("tabs=$(@($tabs).Count) bound=$boundTabId targetHost=$targetHost")

    $fp = $null
    try {
      $fp = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchFingerprintProfile" -RpcArgList @($profileId) -TimeoutSec 90
    } catch {}
    Add-Check "fingerprint_snapshot" ($null -ne $fp) ("ua=$([string]$fp.userAgent)")

    $scrollOk = $false
    try {
      $scrollActions = @(@{ type = "scroll"; distance = 200; tabId = $boundTabId; humanizationLevel = "low" })
      $scrollRes = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchExecuteActions" -RpcArgList @($profileId, $scrollActions) -TimeoutSec 45)
      $scrollOk = (@($scrollRes).Count -gt 0 -and $scrollRes[0].ok -ne $false)
    } catch {
      $scrollOk = $false
    }
    Add-Check "scroll_on_bound_tab" $scrollOk "tab-bound scroll via ExecuteActions"
  }
}
catch {
  Add-Check "fatal" $false $_.Exception.Message
}
finally {
  if (-not $KeepInstance) {
    if ($profileId -and $lastPid -gt 0) {
      Stop-Process -Id $lastPid -Force -ErrorAction SilentlyContinue
    }
    Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" -ErrorAction SilentlyContinue |
      Where-Object { $_.CommandLine -like "*$profileId*" -or $_.CommandLine -like "*wf-smoke*" } |
      ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    if ($harness -and $harness.Process -and -not $harness.Process.HasExited) {
      try { $harness.Process.Kill() } catch {}
    }
  }
}

$failed = @($checks | Where-Object { -not $_.ok })
$status = if ($failed.Count -eq 0) { "passed_full_workflow_smoke" } else { "failed_full_workflow_smoke" }
$report = [ordered]@{
  schema      = "full_workflow_smoke_v1"
  generatedAt = (Get-Date).ToString("o")
  status      = $status
  runId       = $runId
  profileId   = $profileId
  boundTabId  = $boundTabId
  navigateUrl = $NavigateUrl
  checks      = $checks
  failedCount = $failed.Count
  evidenceBoundary = "Local core RPC + headed Chromium tab-bound navigate/scroll/fingerprint smoke. Not platform/provider live."
}
$report | ConvertTo-Json -Depth 10 | Set-Content -Encoding UTF8 $reportPath
Write-Host ""
Write-Host ("Report: {0} status={1} failed={2}" -f $reportPath, $status, $failed.Count) -ForegroundColor $(if ($status -like "passed*") { "Green" } else { "Red" })
if ($status -notlike "passed*") { exit 1 }
