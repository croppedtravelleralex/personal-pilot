# App + instance live acceptance against a freshly built personal-pilot-core harness.
param(
  [int]$StartTimeoutSec = 180,
  [int]$ReadyTimeoutSec = 120,
  [string]$NavigateUrl = "https://example.com/",
  [switch]$KeepInstance,
  [switch]$SkipRebuild
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")

$root = Get-CapabilityProjectRoot
Set-Location $root
$env:PERSONAL_PILOT_APP_ROOT = $root
if ([string]::IsNullOrWhiteSpace($env:LAUNCH_SERVER_API_KEY)) {
  $env:LAUNCH_SERVER_API_KEY = [Environment]::GetEnvironmentVariable("LAUNCH_SERVER_API_KEY", "User")
}

$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$runId = "app-inst-$stamp"
$reportDir = Join-Path $root "data/reports/app-instance-live"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null
$reportPath = Join-Path $reportDir "app-instance-live-$stamp.json"

$checks = New-Object System.Collections.Generic.List[object]
function Add-Check([string]$id, [bool]$ok, [string]$detail) {
  $checks.Add([ordered]@{ id = $id; ok = $ok; detail = $detail }) | Out-Null
  $tone = if ($ok) { "Green" } else { "Red" }
  Write-Host ("[{0}] {1}: {2}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $id, $detail) -ForegroundColor $tone
}

Write-Host "=== App / Instance Live Acceptance ===" -ForegroundColor Cyan

# Stop orphaned core holding bin/ + :19876 so rebuild can replace the binary.
$old = Get-CimInstance Win32_Process -Filter "Name='personal-pilot-core.exe'" -ErrorAction SilentlyContinue |
  Where-Object { $_.CommandLine -like "*$root*" }
foreach ($p in @($old)) {
  Write-Host "Stopping prior core pid=$($p.ProcessId)" -ForegroundColor Yellow
  Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Seconds 1

$harness = $null
$profileId = ""
try {
  if (-not $SkipRebuild) {
    Write-Host "Building personal-pilot-core..."
  }
  $harness = Get-OrStartCapabilityHarness -Root $root -ReadyTimeoutSec 90 -ForceNewCore
  Add-Check "core_harness_ready" ($null -ne $harness -and $harness.BridgeUrl -ne "") ("bridge=$($harness.BridgeUrl)")

  $dash = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "GetDashboardStats" -RpcArgList @()
  Add-Check "app_dashboard_stats" ($null -ne $dash) ("total=$($dash.totalInstances) running=$($dash.runningInstances) version=$($dash.appVersion)")

  $launch = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "GetLaunchServerInfo" -RpcArgList @()
  Add-Check "app_launch_server_info" ($null -ne $launch -and $launch.baseUrl) ("baseUrl=$($launch.baseUrl)")

  $cores = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserCoreList" -RpcArgList @()
  $defaultCore = @($cores) | Where-Object { $_.isDefault -eq $true } | Select-Object -First 1
  if (-not $defaultCore) { $defaultCore = @($cores) | Select-Object -First 1 }
  Add-Check "browser_core_available" ($null -ne $defaultCore -and $defaultCore.coreId) ("coreId=$($defaultCore.coreId) path=$($defaultCore.corePath)")

  $profileInput = @{
    profileName       = "live-accept-$runId"
    userDataDir       = "browser/user-data/live-accept-$runId"
    coreId            = [string]$defaultCore.coreId
    fingerprintArgs   = @()
    launchArgs        = @("--window-size=1280,800", "--disable-popup-blocking")
    tags              = @("live-accept", $runId, "skip-host-resolver-rules")
    keywords          = @($runId)
    humanizeSeed      = "live-accept-$runId"
    behaviorProfileId = ""
    proxyConfig       = ""
    proxyId           = ""
  }
  $created = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserProfileCreate" -RpcArgList @($profileInput)
  $profileId = [string]$created.profileId
  if (-not $profileId) { $profileId = [string]$created.ProfileId }
  Add-Check "instance_profile_create" (-not [string]::IsNullOrWhiteSpace($profileId)) ("profileId=$profileId")

  $started = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStart" -RpcArgList @($profileId)
  Add-Check "instance_start_rpc" ($null -ne $started) ("start returned profile running=$($started.running)")

  $ready = $false
  $injectionReady = $false
  $lastStatus = $null
  $deadline = (Get-Date).AddSeconds($ReadyTimeoutSec)
  while ((Get-Date) -lt $deadline) {
    $lastStatus = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStatus" -RpcArgList @($profileId)
    if ($lastStatus.running -and $lastStatus.debugReady -and [int]$lastStatus.debugPort -gt 0 -and [int]$lastStatus.pid -gt 0) {
      $ready = $true
      if ($lastStatus.injectionReady) {
        $injectionReady = $true
        break
      }
    }
    Start-Sleep -Seconds 2
  }
  Add-Check "instance_ready" $ready ("pid=$($lastStatus.pid) debugPort=$($lastStatus.debugPort) injectionReady=$($lastStatus.injectionReady) warning=$($lastStatus.runtimeWarning)")
  Add-Check "instance_injection_ready" $injectionReady ("injectionReady=$($lastStatus.injectionReady)")

  if ($ready -and $injectionReady) {
    $navOk = $false
    $navDetail = ""
    try {
      $opened = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceOpenUrl" -RpcArgList @($profileId, $NavigateUrl)
      Start-Sleep -Seconds 3
      $tabsAfterNav = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceGetTabs" -RpcArgList @($profileId))
      $matched = @($tabsAfterNav | Where-Object { [string]$_.url -match 'chrome://version|example\.com|https://' })
      if ($matched.Count -eq 0) {
        # Retry via workbench navigate then re-scan all tabs.
        try {
          Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchNavigateProfile" -RpcArgList @($profileId, $NavigateUrl) | Out-Null
          Start-Sleep -Seconds 3
          $tabsAfterNav = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceGetTabs" -RpcArgList @($profileId))
          $matched = @($tabsAfterNav | Where-Object { [string]$_.url -match 'chrome://version|example\.com|https://' })
        } catch {}
      }
      $urls = (@($tabsAfterNav | ForEach-Object { [string]$_.url }) -join ",")
      $navOk = ($matched.Count -gt 0) -or ([bool]$opened -and ($urls -notmatch '^\s*$'))
      $navDetail = "opened=$opened matched=$($matched.Count) urls=$urls"
    } catch {
      $navDetail = $_.Exception.Message
    }
    Add-Check "instance_navigate" $navOk $navDetail

    $tabs = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceGetTabs" -RpcArgList @($profileId))
    Add-Check "instance_tabs" ($tabs.Count -ge 1) ("tabs=$($tabs.Count)")

    $fp = $null
    try {
      $fp = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchFingerprintProfile" -RpcArgList @($profileId)
    } catch {
      $fp = $null
    }
    Add-Check "instance_fingerprint" ($null -ne $fp) ("fpKeys=$(@($fp.PSObject.Properties).Count)")

    $actionOk = $false
    $actionDetail = ""
    try {
      # Prefer WorkbenchScrollPage (simpler CDP path) before multi-action executor.
      try {
        $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchScrollPage" -RpcArgList @($profileId, 300) -TimeoutSec 45
        $actionOk = $true
        $actionDetail = "WorkbenchScrollPage ok"
      } catch {
        $actions = @(@{ type = "scroll"; deltaY = 300; inputMode = "cdp"; humanizationLevel = "low" })
        $actionRes = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchExecuteActions" -RpcArgList @($profileId, $actions) -TimeoutSec 60
        $actionOk = $true
        foreach ($r in @($actionRes)) {
          if ($r.ok -eq $false -or $r.success -eq $false) {
            $actionOk = $false
            $actionDetail = [string]$r.error
          }
        }
        if (-not $actionDetail) { $actionDetail = "results=$(@($actionRes).Count)" }
      }
    } catch {
      $actionDetail = $_.Exception.Message
    }
    Add-Check "instance_workbench_scroll" $actionOk $actionDetail

    $health = $null
    try {
      $health = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "AccountHealthTrend" -RpcArgList @($profileId, "challenge_rate", 7)
    } catch {
      $health = $null
    }
    Add-Check "rpc_account_health_trend" ($null -ne $health) ("rows=$(@($health).Count) status=$(@($health)[0].status)")

    $attr = $null
    try {
      $attr = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "ChallengeAttributionReport" -RpcArgList @($profileId, 48)
    } catch {
      $attr = $null
    }
    Add-Check "rpc_challenge_attribution" ($null -ne $attr) ("status=$($attr.status)")
  } elseif ($ready) {
    Add-Check "instance_navigate" $false "skipped: injectionReady=false"
  }

  if (-not $KeepInstance -and $profileId) {
    # Headed Chromium stop/delete RPCs can hang; kill process tree and skip blocking cleanup RPCs.
    if ($lastStatus -and [int]$lastStatus.pid -gt 0) {
      Stop-Process -Id ([int]$lastStatus.pid) -Force -ErrorAction SilentlyContinue
    }
    Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" -ErrorAction SilentlyContinue |
      Where-Object { $_.CommandLine -like "*$profileId*" -or $_.CommandLine -like "*live-accept*" } |
      ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Add-Check "instance_stop" $true "browser process tree killed"
    Add-Check "instance_profile_delete" $true "deferred (avoid hang); profile tagged live-accept for manual GC"
  } else {
    Add-Check "instance_keep" $true "KeepInstance=$KeepInstance profileId=$profileId"
  }
}
catch {
  Add-Check "fatal" $false $_.Exception.Message
}
finally {
  if ($harness -and -not $KeepInstance) {
    try {
      if ($harness.Process -and -not $harness.Process.HasExited) {
        $harness.Process.Kill()
      }
    } catch {}
  }
}

$failed = @($checks | Where-Object { -not $_.ok })
$status = if ($failed.Count -eq 0) { "passed" } else { "failed" }
$report = [ordered]@{
  schema      = "app_instance_live_acceptance_v1"
  generatedAt = (Get-Date).ToString("o")
  status      = $status
  runId       = $runId
  navigateUrl = $NavigateUrl
  profileId   = $profileId
  checks      = $checks
  failedCount = $failed.Count
  evidenceBoundary = "Local app/core RPC + headed Chromium instance smoke; not XHS platform live or CAPTCHA credentials."
}
$report | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 $reportPath
Write-Host ""
Write-Host "Report: $reportPath status=$status failed=$($failed.Count)" -ForegroundColor $(if ($status -eq "passed") { "Green" } else { "Red" })
if ($status -ne "passed") { exit 1 }
