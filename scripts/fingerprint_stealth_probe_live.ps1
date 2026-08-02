# Live fingerprint + CreepJS/WebRTC stealth probe for a headed instance.
param(
  [string]$ProxyId = "",
  [string]$ProxyConfig = "",
  [int]$ReadyTimeoutSec = 120,
  [double]$MinScore = 50,
  [switch]$NoProxy,
  [switch]$KeepInstance
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")

$root = Get-CapabilityProjectRoot
Set-Location $root
$env:PERSONAL_PILOT_APP_ROOT = $root

$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$reportDir = Join-Path $root "data\reports\app-instance-live"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null
$reportPath = Join-Path $reportDir ("fingerprint-stealth-probe-{0}.json" -f $stamp)

$harness = $null
$profileId = ""
$last = $null
$gateFailures = [System.Collections.Generic.List[string]]::new()

function Select-HealthyProxyId {
  param([object[]]$Proxies)
  foreach ($p in $Proxies) {
    $id = [string]$p.proxyId
    if ([string]::IsNullOrWhiteSpace($id) -or $id.StartsWith("__")) { continue }
    $json = [string]$p.lastIPHealthJson
    if ([string]::IsNullOrWhiteSpace($json)) { continue }
    if ($json -match '"ok"\s*:\s*true') {
      return $id
    }
  }
  return ""
}

try {
  Write-Host "=== Fingerprint Stealth Probe Live ===" -ForegroundColor Cyan
  $harness = Get-OrStartCapabilityHarness -Root $root -ReadyTimeoutSec 90 -ForceNewCore

  $cores = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserCoreList" -RpcArgList @())
  $defaultCore = @($cores) | Where-Object { $_.isDefault -eq $true } | Select-Object -First 1
  if (-not $defaultCore) { $defaultCore = @($cores) | Select-Object -First 1 }
  if (-not $defaultCore) { throw "no browser core available" }

  if ($NoProxy) {
    $ProxyId = ""
    $ProxyConfig = ""
  } elseif ([string]::IsNullOrWhiteSpace($ProxyId) -and [string]::IsNullOrWhiteSpace($ProxyConfig)) {
    $proxies = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserProxyList" -RpcArgList @())
    $ProxyId = Select-HealthyProxyId -Proxies $proxies
  }
  Write-Host ("SELECTED_PROXY_ID={0}" -f $(if ($ProxyId) { $ProxyId } else { "<none>" }))
  Write-Host ("SELECTED_PROXY_CONFIG={0}" -f $(if ($ProxyConfig) { $ProxyConfig } else { "<none>" }))

  $tags = @("fp-stealth", "$stamp", "auto-99")
  # Only skip host-resolver when there is no proxy; with proxy keep leak hardening.
  if ([string]::IsNullOrWhiteSpace($ProxyId) -and [string]::IsNullOrWhiteSpace($ProxyConfig)) {
    $tags += "skip-host-resolver-rules"
  }

  $profileInput = @{
    profileName       = "fp-stealth-$stamp"
    userDataDir       = "browser/user-data/fp-stealth-$stamp"
    coreId            = [string]$defaultCore.coreId
    fingerprintArgs   = @()
    launchArgs        = @("--window-size=1280,800", "--disable-popup-blocking")
    tags              = $tags
    keywords          = @("fp-stealth")
    humanizeSeed      = "fp-stealth-$stamp"
    behaviorProfileId = "balanced-human"
    proxyConfig       = $ProxyConfig
    proxyId           = $ProxyId
  }

  $created = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserProfileCreate" -RpcArgList @($profileInput)
  $profileId = [string]$created.profileId
  if ([string]::IsNullOrWhiteSpace($profileId)) { $profileId = [string]$created.ProfileId }
  if ([string]::IsNullOrWhiteSpace($profileId)) { throw "profile create returned empty id" }
  Write-Host ("PROFILE={0}" -f $profileId)

  $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStart" -RpcArgList @($profileId) -TimeoutSec 180

  $ready = $false
  $deadline = (Get-Date).AddSeconds($ReadyTimeoutSec)
  while ((Get-Date) -lt $deadline) {
    $last = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStatus" -RpcArgList @($profileId) -TimeoutSec 30
    if ($last.running -and $last.debugReady -and [int]$last.debugPort -gt 0) {
      # Prefer injectionReady, but don't hard-fail if only debugReady is up.
      if ($last.injectionReady) {
        $ready = $true
        break
      }
      $ready = $true
    }
    Start-Sleep -Seconds 2
  }
  if (-not $ready) {
    throw ("instance not ready: " + ($last | ConvertTo-Json -Compress -Depth 6))
  }
  Write-Host ("READY pid={0} port={1} injectionReady={2}" -f $last.pid, $last.debugPort, $last.injectionReady)
  if (-not $last.injectionReady) {
    Write-Host "WARN injectionReady=false; continuing with debugReady only" -ForegroundColor Yellow
    Start-Sleep -Seconds 3
  }

  $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceOpenUrl" -RpcArgList @($profileId, "https://example.com/")
  Start-Sleep -Seconds 2

  $fp = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchFingerprintProfile" -RpcArgList @($profileId)
  $health = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchFingerprintHealthProfile" -RpcArgList @($profileId)
  $identity = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "IdentityReportProfile" -RpcArgList @($profileId)

  Write-Host "Running WorkbenchRunStealthProbeSuite (WebRTC + CreepJS)..."
  $probe = $null
  $probeError = ""
  try {
    $probe = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchRunStealthProbeSuite" -RpcArgList @($profileId)
  } catch {
    $probeError = $_.Exception.Message
    Write-Host ("PROBE_ERR={0}" -f $probeError) -ForegroundColor Yellow
  }

  $stealth = $null
  $stealthError = ""
  try {
    $stealth = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "AsymmetricStealthReportV2" -RpcArgList @($profileId)
  } catch {
    $stealthError = $_.Exception.Message
    $stealth = $null
  }

  $accountHealth = $null
  try {
    $accountHealth = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchAccountHealthReport" -RpcArgList @($profileId)
  } catch {
    $accountHealth = @{ error = $_.Exception.Message }
  }

  $report = [ordered]@{
    schema           = "instance_fingerprint_stealth_probe_v1"
    generatedAt      = (Get-Date).ToString("o")
    profileId        = $profileId
    proxyId          = $ProxyId
    proxyConfig      = $ProxyConfig
    pid              = $last.pid
    debugPort        = $last.debugPort
    injectionReady   = $last.injectionReady
    fingerprintSummary = [ordered]@{
      userAgent               = $fp.userAgent
      platform                = $fp.platform
      webdriver               = $fp.webdriver
      timezone                = $fp.timezone
      language                = $fp.language
      hardwareConcurrency     = $fp.hardwareConcurrency
      deviceMemory            = $fp.deviceMemory
      screen                  = ("{0}x{1}" -f $fp.screenWidth, $fp.screenHeight)
      canvasHash              = $fp.canvasHash
      fontHash                = $fp.fontHash
      webglVendor             = $fp.webglVendor
      webglRenderer           = $fp.webglRenderer
      uaDataPlatform          = $fp.uaDataPlatform
      uaDataBrands            = $fp.uaDataBrands
      uaDataFullVersionList   = $fp.uaDataFullVersionList
    }
    health = [ordered]@{
      score      = $health.score
      level      = $health.level
      checkCount = @($health.checks).Count
      checks     = $health.checks
    }
    identity = [ordered]@{
      score     = $identity.score
      level     = $identity.level
      subscores = $identity.subscores
      summary   = $identity.summary
    }
    probe        = $probe
    probeError   = $probeError
    stealth      = $stealth
    stealthError = $stealthError
    accountHealth = $accountHealth
  }
  $report | ConvertTo-Json -Depth 14 | Set-Content -Encoding UTF8 $reportPath

  Write-Host ""
  Write-Host ("REPORT={0}" -f $reportPath) -ForegroundColor Green
  Write-Host ("HEALTH={0}/{1}" -f $health.score, $health.level)
  Write-Host ("IDENTITY={0}/{1}" -f $identity.score, $identity.level)
  if ($identity.subscores) {
    Write-Host ("SUBSCORES={0}" -f ($identity.subscores | ConvertTo-Json -Compress))
  }
  if ($probe) {
    $creep = $probe.creepjs
    $webrtc = $probe.webrtc
    Write-Host ("PROBE stealthScore={0} grade={1}" -f $probe.stealthScore, $probe.grade)
    if ($creep) {
      Write-Host ("CREEPJS trust={0} lies={1} message={2}" -f $creep.trustScore, $creep.lieCount, $creep.message)
    }
    if ($webrtc) {
      Write-Host ("WEBRTC={0}" -f ($webrtc | ConvertTo-Json -Compress -Depth 6))
    }
  } elseif ($probeError) {
    Write-Host ("PROBE failed: {0}" -f $probeError) -ForegroundColor Yellow
  }
  if ($stealth) {
    Write-Host ("STEALTH score={0} grade={1} strategy={2}" -f $stealth.stealthScore, $stealth.displayGrade, $stealth.strategy)
    if ($stealth.gaps) {
      Write-Host ("GAPS={0}" -f (($stealth.gaps | ForEach-Object { "$_" }) -join " | "))
    }
    foreach ($dim in @($stealth.dimensions)) {
      Write-Host ("DIM {0}={1} w={2}" -f $dim.id, $dim.score, $dim.weight)
    }
  } elseif ($stealthError) {
    Write-Host ("STEALTH failed: {0}" -f $stealthError) -ForegroundColor Yellow
  }

  # --- Gate evaluation (fail honestly, exit nonzero on missing/low probe data) ---
  if ($probeError) { $gateFailures.Add("probe request failed: $probeError") }
  elseif (-not $probe -or $null -eq $probe.stealthScore) {
    $gateFailures.Add("probe returned no usable stealthScore")
  } elseif ($probe.stealthScore -lt $MinScore) {
    $gateFailures.Add("probe stealthScore=$($probe.stealthScore) below MinScore=$MinScore")
  }
  if ($stealthError) { $gateFailures.Add("stealth report failed: $stealthError") }
  elseif (-not $stealth -or $null -eq $stealth.stealthScore) {
    $gateFailures.Add("stealth report returned no usable score")
  } elseif ($stealth.stealthScore -lt $MinScore) {
    $gateFailures.Add("stealth score=$($stealth.stealthScore) below MinScore=$MinScore")
  }
  if ($null -eq $health.score -or [int]$health.score -lt $MinScore) {
    $gateFailures.Add("health score unacceptable: $($health.score)")
  }

  if ($gateFailures.Count -gt 0) {
    Write-Host ""
    Write-Host "=== Gate FAILED ($($gateFailures.Count)) ===" -ForegroundColor Red
    foreach ($f in $gateFailures) { Write-Host ("  - {0}" -f $f) -ForegroundColor Red }
    exit 1
  }
  Write-Host ""
  Write-Host "=== Gate PASSED ===" -ForegroundColor Green
  exit 0
}
catch {
  Write-Host ("FATAL: {0}" -f $_.Exception.Message) -ForegroundColor Red
  exit 1
}
finally {
  if (-not $KeepInstance) {
    if ($profileId -and $last -and [int]$last.pid -gt 0) {
      Stop-Process -Id ([int]$last.pid) -Force -ErrorAction SilentlyContinue
    }
    Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" -ErrorAction SilentlyContinue |
      Where-Object { $_.CommandLine -like "*fp-stealth*" } |
      ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    if ($harness -and $harness.Process -and -not $harness.Process.HasExited) {
      try { $harness.Process.Kill() } catch {}
    }
  } else {
    Write-Host ("KeepInstance profileId={0}" -f $profileId)
  }
}
