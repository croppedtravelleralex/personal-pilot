param(
  [string]$ProfileId = "a4b108a0-afb3-4e93-843e-f5ef22278b9d",
  [string]$ViaSSH = "panda",
  [int]$PollSec = 360,
  [switch]$FullProfileUpdate
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")

$root = Get-CapabilityProjectRoot
Set-Location $root
$proxy = Import-CapabilityProxyConfig -Root $root
if (-not $proxy) { throw "proxy.local.env missing" }

$proxyServer = $proxy.ProxyServer
if ($ViaSSH) {
  $join = if ($proxyServer -match '\?') { '&' } else { '?' }
  $proxyServer = "$proxyServer${join}pp_via_ssh=$ViaSSH"
}

$harness = $null
try {
  $harness = Start-CapabilityCoreHarness -Root $root -ReadyTimeoutSec 90
  Write-Host "Bridge: $($harness.BridgeUrl)"

  $launchInfo = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "GetLaunchServerInfo"
  $launchBase = [string]$launchInfo.baseUrl
  Write-Host "Launch: $launchBase"

  function Get-DiagInstanceStatus {
    return Invoke-RestMethod -Uri "$launchBase/api/instances/status?profileId=$ProfileId" -TimeoutSec 15
  }

  try {
    $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStop" -RpcArgList @($ProfileId)
    Start-Sleep -Seconds 2
  } catch {}

  $update = @{
    proxyConfig = $proxyServer
    tags        = @("xhs", "auto-99")
  }
  if ($FullProfileUpdate) {
    $update.coreId = "core-fingerprint-chromium-139-0-7258-154"
    $update.fingerprintArgs = @(
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
    $update.launchArgs = @("--disable-blink-features=AutomationControlled")
    $update.humanizeSeed = "xhs-19202757042"
    $update.behaviorProfileId = "office-worker"
  }
  $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserProfileUpdate" -RpcArgList @($ProfileId, $update)

  Write-Host "Starting instance (sync RPC, timeout 600s)..."
  $startParams = @{
    launchArgs           = @()
    startUrls            = @("https://www.xiaohongshu.com/explore")
    skipDefaultStartUrls = $true
  }
  $startJob = Start-Job -ScriptBlock {
    param($BridgeUrl, $BridgeToken, $ProfileId, $Params, $LibPath)
    . $LibPath
    Invoke-CapabilityCoreRpc -BridgeUrl $BridgeUrl -BridgeToken $BridgeToken -Method "BrowserInstanceStartWithParams" -RpcArgList @($ProfileId, $Params.launchArgs, $Params.startUrls, $Params.skipDefaultStartUrls) | Out-Null
    return "done"
  } -ArgumentList $harness.BridgeUrl, $harness.BridgeToken, $ProfileId, $startParams, (Join-Path $root "scripts/capability_scenario_lib.ps1")

  $deadline = (Get-Date).AddSeconds($PollSec)
  while ((Get-Date) -lt $deadline) {
    $st = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStatus" -RpcArgList @($ProfileId)
    $line = "{0:HH:mm:ss} running={1} debugReady={2} injectionReady={3} pid={4} port={5} lastError={6}" -f (Get-Date), $st.running, $st.debugReady, $st.injectionReady, $st.pid, $st.debugPort, $st.lastError
    Write-Host $line
    if ($st.running -and $st.debugReady -and $st.injectionReady) {
      Write-Host "READY" -ForegroundColor Green
      break
    }
    if ($st.lastError) { Write-Host "lastError: $($st.lastError)" -ForegroundColor Yellow }
    Start-Sleep -Seconds 5
  }

  $jobState = Get-Job $startJob -ErrorAction SilentlyContinue
  if ($jobState) {
    Receive-Job $startJob -ErrorAction SilentlyContinue | Write-Host
    Remove-Job $startJob -Force -ErrorAction SilentlyContinue
  }
} finally {
  if ($harness) { Stop-CapabilityCoreHarness -Harness $harness }
}
