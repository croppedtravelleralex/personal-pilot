param(
  [string]$Phone = "19202757042",
  [string]$ProfileId = "a4b108a0-afb3-4e93-843e-f5ef22278b9d",
  [int]$StartTimeoutSec = 300,
  [int]$InjectionTimeoutSec = 120,
  [string]$ViaSSH = "panda",
  [string]$LaunchBase = "http://127.0.0.1:19876",
  [switch]$ForceNewCore,
  [switch]$KeepInstance
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")

$root = Get-CapabilityProjectRoot
Set-Location $root
$proxy = Import-CapabilityProxyConfig -Root $root
if (-not $proxy) { throw 'proxy.local.env missing - cannot run live XHS acceptance' }

$phone = ($Phone -replace '\D', '').Trim()
$userDataDir = "browser/user-data/xhs-$phone"
$results = @()

function Add-LiveResult {
  param([string]$Id, [string]$Name, [bool]$Pass, [string]$Detail)
  $script:results += [pscustomobject]@{ Id = $Id; Name = $Name; Pass = $Pass; Detail = $Detail }
}

function Test-ValidNavigationUrl {
  param([string]$PageUrl)
  if ([string]::IsNullOrWhiteSpace($PageUrl)) { return $false }
  if ($PageUrl -match 'chrome-error|chromewebdata|about:neterror') { return $false }
  return $PageUrl -match '^https://'
}

function Invoke-LaunchInstanceStop {
  param(
    [string]$LaunchBase,
    [string]$ProfileId,
    [int]$TimeoutSec = 120
  )
  return Invoke-RestMethod -Uri "$LaunchBase/api/instances/stop" -Method Post -ContentType "application/json" -Body (@{ profileId = $ProfileId } | ConvertTo-Json -Compress) -TimeoutSec $TimeoutSec
}

function Stop-LaunchInstanceForce {
  param(
    [string]$LaunchBase,
    [string]$ProfileId,
    [int]$TimeoutSec = 90
  )
  try {
    $null = Invoke-LaunchInstanceStop -LaunchBase $LaunchBase -ProfileId $ProfileId -TimeoutSec $TimeoutSec
  } catch {
    Write-Host "WARN: graceful stop failed: $($_.Exception.Message)" -ForegroundColor Yellow
  }
  try {
    $st = Get-LaunchInstanceStatus -LaunchBase $LaunchBase -ProfileId $ProfileId
    $procId = [int]$st.pid
    if ($procId -gt 0) {
      Write-Host "Force killing browser pid=$procId" -ForegroundColor Yellow
      Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
    }
  } catch {}
  $deadline = (Get-Date).AddSeconds(30)
  while ((Get-Date) -lt $deadline) {
    $st = Get-LaunchInstanceStatus -LaunchBase $LaunchBase -ProfileId $ProfileId
    if (-not $st.running) { return $st }
    Start-Sleep -Seconds 2
  }
  return Get-LaunchInstanceStatus -LaunchBase $LaunchBase -ProfileId $ProfileId
}

function Invoke-LaunchInstanceStart {
  param(
    [string]$LaunchBase,
    [string]$ProfileId,
    [int]$TimeoutSec = 600
  )
  return Invoke-RestMethod -Uri "$LaunchBase/api/instances/$ProfileId`?action=start" -Method Post -TimeoutSec $TimeoutSec
}

function Update-LaunchProfile {
  param(
    [string]$LaunchBase,
    [string]$ProfileId,
    [hashtable]$ProfileFields,
    [int]$TimeoutSec = 60
  )
  $body = @{ profile = $ProfileFields } | ConvertTo-Json -Depth 10 -Compress
  return Invoke-RestMethod -Uri "$LaunchBase/api/profiles/$ProfileId" -Method Put -ContentType "application/json" -Body $body -TimeoutSec $TimeoutSec
}

function Invoke-CoreLong {
  param(
    [object]$Harness,
    [string]$Method,
    [object[]]$RpcArgs = @(),
    [int]$TimeoutSec = 120
  )
  if ($Harness.HttpOnly) {
    throw "RPC $Method unavailable in HTTP-only mode (LaunchServer reuse)"
  }
  $encodedArgs = @()
  foreach ($item in $RpcArgs) {
    if ($null -eq $item) {
      $encodedArgs += $null
      continue
    }
    $encodedArgs += ,$item
  }
  $body = @{ method = $Method; args = $encodedArgs } | ConvertTo-Json -Depth 12 -Compress
  $headers = @{
    "Content-Type" = "application/json"
    "X-Personal-Pilot-Bridge-Token" = $Harness.BridgeToken
  }
  $resp = Invoke-RestMethod -Uri "$($Harness.BridgeUrl.TrimEnd('/'))/rpc" -Method Post -Headers $headers -Body $body -TimeoutSec $TimeoutSec
  if (-not $resp.ok) { throw "RPC $Method failed: $($resp.error)" }
  return $resp.result
}

function Invoke-LaunchWorkbenchActions {
  param(
    [string]$LaunchBase,
    [string]$ProfileId,
    [array]$Actions,
    [int]$TimeoutSec = 120
  )
  $body = @{ profileId = $ProfileId; actions = $Actions } | ConvertTo-Json -Depth 8 -Compress
  return Invoke-RestMethod -Uri "$LaunchBase/api/workbench/actions" -Method Post -ContentType "application/json" -Body $body -TimeoutSec $TimeoutSec
}

function Get-LaunchInstanceStatus {
  param(
    [string]$LaunchBase,
    [string]$ProfileId
  )
  return Invoke-RestMethod -Uri "$LaunchBase/api/instances/status?profileId=$ProfileId" -TimeoutSec 15
}

function Wait-ProfileLaunchReady {
  param(
    [string]$LaunchBase,
    [string]$ProfileId,
    [int]$TimeoutSec = 300
  )
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  $st = $null
  $graceUntil = (Get-Date).AddSeconds(15)
  while ((Get-Date) -lt $deadline) {
    $st = Get-LaunchInstanceStatus -LaunchBase $LaunchBase -ProfileId $ProfileId
    $lastErr = ""
    if ($st.profile -and $st.profile.lastError) { $lastErr = [string]$st.profile.lastError }
    Write-Host ([string]::Format(
      '{0:HH:mm:ss} running={1} debugReady={2} injectionReady={3} pid={4} port={5} lastError={6}',
      (Get-Date), $st.running, $st.debugReady, $st.injectionReady, $st.pid, $st.debugPort, $lastErr
    ))
    if ($st.running -and $st.debugReady -and $st.injectionReady) {
      return $st
    }
    if (-not $st.running -and (Get-Date) -gt $graceUntil -and $lastErr -and $lastErr -notmatch 'pending|attach|窗口|后台') {
      throw "instance stopped before ready: $lastErr"
    }
    Start-Sleep -Seconds 3
  }
  $detail = [string]::Format(
    'running={0} debugReady={1} injectionReady={2}',
    $st.running, $st.debugReady, $st.injectionReady
  )
  throw "timeout waiting for debugReady+injectionReady within ${TimeoutSec}s ($detail)"
}

function Wait-ProfileNotRunning {
  param(
    [string]$LaunchBase,
    [string]$ProfileId,
    [int]$TimeoutSec = 90
  )
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    $st = Get-LaunchInstanceStatus -LaunchBase $LaunchBase -ProfileId $ProfileId
    if (-not $st.running) { return $st }
    Start-Sleep -Seconds 2
  }
  throw "profile still running after ${TimeoutSec}s"
}

function Wait-ProfileState {
  param(
    [string]$LaunchBase,
    [string]$ProfileId,
    [scriptblock]$Predicate,
    [int]$TimeoutSec = 120,
    [string]$Label = "condition"
  )
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  $last = $null
  while ((Get-Date) -lt $deadline) {
    $last = Get-LaunchInstanceStatus -LaunchBase $LaunchBase -ProfileId $ProfileId
    if ($last.profile.lastError -and [string]$last.profile.lastError -notmatch 'pending|attach|窗口|后台') {
      throw "profile error while waiting for ${Label}: $($last.profile.lastError)"
    }
    if (& $Predicate $last) { return $last }
    Start-Sleep -Seconds 2
  }
  $detail = "running=$($last.running) debugReady=$($last.debugReady) injectionReady=$($last.injectionReady) pid=$($last.pid) debugPort=$($last.debugPort)"
  throw "timeout waiting for ${Label} within ${TimeoutSec}s ($detail)"
}

Write-Host "=== XHS Live Acceptance (UDEAL + stealth + HTTP workbench) ===" -ForegroundColor Cyan
$proxyServer = $proxy.ProxyServer
if ($ViaSSH) {
  $join = if ($proxyServer -match '\?') { '&' } else { '?' }
  $proxyServer = "$proxyServer${join}pp_via_ssh=$ViaSSH"
  Write-Host "SSH tunnel: pp_via_ssh=$ViaSSH"
}

$harness = $null
$exitCode = 0
$profileId = ""
$launchBase = ""
try {
  $harness = Get-OrStartCapabilityHarness -Root $root -ReadyTimeoutSec 60 -ForceNewCore:$ForceNewCore -LaunchBase $LaunchBase
  $launchBase = [string]$harness.LaunchBase
  if (-not (Test-LaunchServerHealthy -BaseUrl $launchBase)) { throw "LaunchServer not ready at $launchBase" }
  if ($harness.HttpOnly) {
    Write-Host "Reusing existing LaunchServer: $launchBase" -ForegroundColor Green
  } else {
    Write-Host "Started harness core: $($harness.BridgeUrl)"
  }
  Add-LiveResult -Id "L-01" -Name "LaunchServer ready" -Pass $true -Detail $launchBase

  # L-02: proxy bridge smoke via LaunchServer manual test path (bridge managers wired)
  $parseBody = @{ raw = $proxyServer } | ConvertTo-Json -Compress
  $parsed = Invoke-RestMethod -Uri "$launchBase/api/proxy/parse" -Method Post -ContentType "application/json" -Body $parseBody -TimeoutSec 30
  $parseOk = ($parsed.ok -eq $true) -and ($parsed.proxyConfig -match 'pp_via_ssh')
  Add-LiveResult -Id "L-02" -Name "UDEAL proxy parse + pp_via_ssh" -Pass $parseOk -Detail ($parsed.proxyConfig)

  $profile = $null
  if ($ProfileId) {
    try {
      $got = Invoke-RestMethod -Uri "$launchBase/api/profiles/$ProfileId" -TimeoutSec 15
      if ($got.ok -eq $true -and $got.profile) { $profile = $got.profile }
    } catch {}
  }
  if (-not $profile) {
    $listResp = Invoke-RestMethod -Uri "$launchBase/api/profiles" -TimeoutSec 15
    $list = @($listResp.profiles)
    if ($ProfileId) {
      $profile = $list | Where-Object { $_.profileId -eq $ProfileId } | Select-Object -First 1
    }
    if (-not $profile) {
      $suffix = "xhs-$phone"
      $profile = $list | Where-Object { [string]$_.userDataDir -like "*$suffix*" } | Select-Object -First 1
    }
  }
  if (-not $profile) { throw "XHS profile not found for $userDataDir" }
  $profileId = [string]$profile.profileId
  $resolvedUserDataDir = [string]$profile.userDataDir
  if (-not $resolvedUserDataDir) { $resolvedUserDataDir = $userDataDir }
  Write-Host "Profile: $profileId ($($profile.profileName))"

  $fpArgs = @(
    "--fingerprint-brand=Chrome",
    "--fingerprint-platform=windows",
    "--fingerprint-platform-version=10.0.0",
    "--fingerprint-hardware-concurrency=8",
    "--fingerprint-device-memory=8",
    "--fingerprint-canvas-noise=true",
    "--fingerprint-audio-noise=true",
    "--webrtc-ip-handling-policy=disable_non_proxied_udp",
    "--lang=zh-CN",
    "--timezone=Asia/Shanghai",
    "--window-size=1920,1080"
  )
  $launchArgs = @("--disable-blink-features=AutomationControlled")
  $tags = @("xhs", "nurture", "creator", "phone-login", "auto-99")

  $updateBody = @{
    profileName       = "小红书 $phone"
    userDataDir       = $resolvedUserDataDir
    coreId            = "core-fingerprint-chromium-139-0-7258-154"
    fingerprintArgs   = $fpArgs
    launchArgs        = $launchArgs
    tags              = $tags
    keywords          = @("xhs", $phone, "creator-center")
    humanizeSeed      = "xhs-$phone"
    behaviorProfileId = "office-worker"
    proxyId           = ""
    proxyConfig       = $proxyServer
  }
  $null = Update-LaunchProfile -LaunchBase $launchBase -ProfileId $profileId -ProfileFields $updateBody
  Add-LiveResult -Id "L-03" -Name "Profile auto-99 + UDEAL proxy" -Pass $true -Detail $proxyServer

  $null = Stop-LaunchInstanceForce -LaunchBase $launchBase -ProfileId $profileId
  try {
    $null = Wait-ProfileNotRunning -LaunchBase $launchBase -ProfileId $profileId -TimeoutSec 30
  } catch {
    Write-Host "WARN: $($_.Exception.Message)" -ForegroundColor Yellow
  }

  Write-Host "Starting instance (HTTP start + status poll)..."
  try {
    $null = Invoke-LaunchInstanceStart -LaunchBase $launchBase -ProfileId $profileId -TimeoutSec ([Math]::Max($StartTimeoutSec, 420))
  } catch {
    Write-Host "WARN: HTTP start: $($_.Exception.Message)" -ForegroundColor Yellow
  }

  $readyTimeout = $StartTimeoutSec + $InjectionTimeoutSec
  $st = Wait-ProfileLaunchReady -LaunchBase $launchBase -ProfileId $profileId -TimeoutSec $readyTimeout
  $detailL04 = [string]::Format('debugPort={0} injectionReady={1}', $st.debugPort, $st.injectionReady)
  Add-LiveResult -Id 'L-04' -Name 'Instance start sing-box SSH' -Pass $true -Detail $detailL04
  Add-LiveResult -Id 'L-05' -Name 'injectionReady gate' -Pass ([bool]$st.injectionReady) -Detail ([string]::Format('injectionReady={0}', $st.injectionReady))

  # L-06: HTTP workbench navigate - must land on real HTTPS page
  $navResp = Invoke-LaunchWorkbenchActions -LaunchBase $launchBase -ProfileId $profileId -Actions @(
    @{ type = "navigate"; url = "https://www.xiaohongshu.com/explore"; humanizationLevel = "high"; postWaitMs = 4000 }
  )
  $navResult = @($navResp.results)[0]
  $pageUrl = [string]$navResult.pageUrl
  $navOk = ($navResult.ok -eq $true) -and (Test-ValidNavigationUrl $pageUrl) -and ($pageUrl -match 'xiaohongshu\.com')
  Add-LiveResult -Id "L-06" -Name "HTTP navigate explore (real page)" -Pass $navOk -Detail "pageUrl=$pageUrl title=$($navResult.pageTitle)"

  # L-07: humanize scroll via HTTP API
  $scrollResp = Invoke-LaunchWorkbenchActions -LaunchBase $launchBase -ProfileId $profileId -Actions @(
    @{ type = "scroll"; distance = 400; direction = "down"; humanizationLevel = "high"; postWaitMs = 1500 }
  )
  $scrollOk = @($scrollResp.results)[0].ok -eq $true
  Add-LiveResult -Id "L-07" -Name "HTTP humanize scroll" -Pass $scrollOk -Detail ($scrollResp.results | ConvertTo-Json -Compress -Depth 3)

  # L-08: script bypass blocked (HTTP workbench path)
  $scriptBlocked = $false
  try {
    $null = Invoke-LaunchWorkbenchActions -LaunchBase $launchBase -ProfileId $profileId -Actions @(
      @{ type = "script"; script = "1+1"; humanizationLevel = "high" }
    ) -TimeoutSec 30
  } catch {
    if ($_.Exception.Message -match "forbidden|script|403|blocked|denied") { $scriptBlocked = $true }
  }
  if (-not $scriptBlocked -and -not $harness.HttpOnly) {
    try {
      $scriptActions = @(@{ type = "script"; script = "1+1"; humanizationLevel = "high" })
      $null = Invoke-CoreLong -Harness $harness -Method "WorkbenchExecuteActions" -RpcArgs @($profileId, $scriptActions) -TimeoutSec 30
    } catch {
      if ($_.Exception.Message -match "forbidden|script|403|blocked|denied") { $scriptBlocked = $true }
    }
  }
  Add-LiveResult -Id "L-08" -Name "script bypass blocked" -Pass $scriptBlocked -Detail $(if ($scriptBlocked) { "403 as expected" } else { "script not blocked" })

  # L-09: audit logs must contain HTTP workbench traces
  $logs = Invoke-RestMethod -Uri "$launchBase/api/audit/logs?limit=50" -TimeoutSec 10
  $apiHits = @($logs.items | Where-Object { $_.path -like "/api/workbench/*" })
  Add-LiveResult -Id "L-09" -Name "audit HTTP workbench traces" -Pass ($apiHits.Count -gt 0) -Detail "entries=$($apiHits.Count)"

  # L-10: fingerprint health via HTTP (stealth signal)
  $fpOk = $false
  $fpDetail = "skipped"
  try {
    $fpHealth = Invoke-RestMethod -Uri "$launchBase/api/workbench/fingerprint-health" -Method Post -ContentType "application/json" -Body (@{ profileId = $profileId } | ConvertTo-Json -Compress) -TimeoutSec 90
    $fpOk = ($fpHealth.ok -eq $true) -and ($fpHealth.health.overall -ne "critical")
    $fpDetail = [string]$fpHealth.health.overall
  } catch {
    $fpDetail = $_.Exception.Message
  }
  Add-LiveResult -Id "L-10" -Name "fingerprint health (stealth)" -Pass $fpOk -Detail $fpDetail

  # L-11: input plane - scroll + click (CDP first; auto/os when headed window is reachable)
  $inputPlaneOk = $false
  $inputDetail = "skipped: navigate failed"
  if ($navOk) {
    try {
      $null = Invoke-LaunchWorkbenchActions -LaunchBase $launchBase -ProfileId $profileId -TimeoutSec 45 -Actions @(
        @{ type = "scroll"; distance = 200; direction = "down"; humanizationLevel = "high"; inputMode = "cdp"; postWaitMs = 600 }
      )
      $inputResp = Invoke-LaunchWorkbenchActions -LaunchBase $launchBase -ProfileId $profileId -TimeoutSec 45 -Actions @(
        @{ type = "click"; selector = "body"; humanizationLevel = "high"; inputMode = "cdp"; postWaitMs = 400 }
      )
      $clickResult = @($inputResp.results | Where-Object { $_.type -eq "click" } | Select-Object -First 1)
      if ($clickResult) {
        $inputPlaneOk = ($clickResult.ok -eq $true) -and ($clickResult.inputPlane -in @("os", "cdp"))
        $inputDetail = "inputPlane=$($clickResult.inputPlane) ok=$($clickResult.ok)"
        if (-not $inputPlaneOk -and $clickResult.error) { $inputDetail += " err=$($clickResult.error)" }
      } else {
        $inputDetail = "no click result"
      }
    } catch {
      $inputDetail = $_.Exception.Message
    }
  }
  Add-LiveResult -Id "L-11" -Name "input plane click (auto/os)" -Pass $inputPlaneOk -Detail $inputDetail

  # L-12: CreepJS stealth probe (live; threshold 85 for 99+)
  $creepOk = $false
  $creepDetail = "skipped: injection not ready"
  if ($st -and $st.injectionReady) {
    Start-Sleep -Seconds 3
    for ($attempt = 1; $attempt -le 3; $attempt++) {
      try {
        $probe = Invoke-RestMethod -Uri "$launchBase/api/workbench/stealth-probe" -Method Post -ContentType "application/json" -Body (@{ profileId = $profileId } | ConvertTo-Json -Compress) -TimeoutSec 180
        $creepTrust = [double]$probe.creepTrust
        $creepOk = ($probe.ok -eq $true) -and ($creepTrust -ge 85)
        $creepDetail = "creepTrust=$creepTrust target99Plus=$($probe.target99Plus)"
        break
      } catch {
        $creepDetail = $_.Exception.Message
        if ($_.Exception.Message -match '409|冲突|ownership|busy' -and $attempt -lt 3) {
          Write-Host "WARN: stealth probe busy, retry $attempt..." -ForegroundColor Yellow
          Start-Sleep -Seconds 5
          continue
        }
      }
    }
  }
  Add-LiveResult -Id "L-12" -Name "CreepJS stealth probe >= 85" -Pass $creepOk -Detail $creepDetail

  $sessionFile = Join-Path $root "data/scenarios/fixtures/xhs-login.session.json"
  New-Item -ItemType Directory -Force -Path (Split-Path $sessionFile) | Out-Null
  @{
    schema         = "xhs_live_acceptance_v2"
    createdAt      = (Get-Date).ToString("o")
    phone          = $phone
    profileId      = $profileId
    launchBase     = $launchBase
    bridgeUrl      = $harness.BridgeUrl
    bridgeToken    = $harness.BridgeToken
    httpOnly       = [bool]$harness.HttpOnly
    injectionReady = [bool]$st.injectionReady
    debugReady     = [bool]$st.debugReady
    pageUrl        = $pageUrl
    proxy          = "$($proxy.Host):$($proxy.Port)"
    status         = $(if ($navOk) { "live_acceptance_pass" } else { "live_acceptance_fail" })
  } | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 $sessionFile

  Write-Host ""
  Write-Host "Session: $sessionFile" -ForegroundColor Green
  Write-Host "Launch API: $launchBase" -ForegroundColor Green
}
catch {
  $exitCode = 1
  Write-Host "LIVE ACCEPTANCE ERROR: $($_.Exception.Message)" -ForegroundColor Red
}
finally {
  if ($profileId -and $launchBase -and -not $KeepInstance) {
    try {
      Write-Host "Acceptance cleanup: stopping browser instance..." -ForegroundColor DarkGray
      $null = Stop-LaunchInstanceForce -LaunchBase $launchBase -ProfileId $profileId
    } catch {
      Write-Host "WARN: acceptance cleanup stop: $($_.Exception.Message)" -ForegroundColor Yellow
    }
  } elseif ($KeepInstance -and $profileId) {
    Write-Host "KeepInstance: browser left running (profileId=$profileId)." -ForegroundColor Yellow
  }
  if ($harness) {
    try { Stop-CapabilityCoreHarness -Harness $harness } catch {}
  }
  Write-Host ""
  Write-Host "=== Live Results ===" -ForegroundColor Cyan
  $results | Format-Table -AutoSize
  $failed = @($results | Where-Object { -not $_.Pass })
  if ($failed.Count -gt 0 -or $exitCode -ne 0) {
    Write-Host "FAIL: $($failed.Count) check(s)" -ForegroundColor Red
    exit 1
  }
  Write-Host ('ALL PASS ({0} checks)' -f $results.Count) -ForegroundColor Green
}
