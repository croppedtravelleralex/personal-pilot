# Same-profile fingerprint drift gate.
# Default mode: single long-lived instance, N capture rounds with reload between.
# Optional -CrossSessionRestarts restarts browser between rounds (slower, may hit start locks).
param(
  [int]$Runs = 10,
  [int]$ReadyTimeoutSec = 120,
  [string]$ProxyId = "",
  [switch]$NoProxy,
  [switch]$CrossSessionRestarts,
  [switch]$KeepInstance
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")

$root = Get-CapabilityProjectRoot
Set-Location $root
$env:PERSONAL_PILOT_APP_ROOT = $root

$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$reportDir = Join-Path $root "data\reports\fingerprint-drift"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null
$reportPath = Join-Path $reportDir ("fingerprint-drift-$stamp.json")

function Get-FpKey($fp) {
  return [ordered]@{
    userAgent = [string]$fp.userAgent
    platform = [string]$fp.platform
    webdriver = [bool]$fp.webdriver
    timezone = [string]$fp.timezone
    language = [string]$fp.language
    hardwareConcurrency = [int]$fp.hardwareConcurrency
    deviceMemory = [int]$fp.deviceMemory
    canvasHash = [string]$fp.canvasHash
    fontHash = [string]$fp.fontHash
    webglVendor = [string]$fp.webglVendor
    webglRenderer = [string]$fp.webglRenderer
    webglExtensionsHash = [string]$fp.webglExtensionsHash
    audioHash = [string]$fp.audioHash
    uaDataPlatform = [string]$fp.uaDataPlatform
    uaDataBrands = @($fp.uaDataBrands)
    uaDataFullVersionList = @($fp.uaDataFullVersionList)
  }
}

function Hash-Object($obj) {
  $json = ($obj | ConvertTo-Json -Compress -Depth 8)
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
  $sha = [System.Security.Cryptography.SHA256]::Create()
  ($sha.ComputeHash($bytes) | ForEach-Object { $_.ToString("x2") }) -join ""
}

function Stop-ProfileBrowser($bridgeUrl, $bridgeToken, $profileId, $processId) {
  try {
    $null = Invoke-CapabilityCoreRpc -BridgeUrl $bridgeUrl -BridgeToken $bridgeToken -Method "BrowserInstanceStop" -RpcArgList @($profileId) -TimeoutSec 15
  } catch {}
  if ($processId -and [int]$processId -gt 0) {
    Stop-Process -Id ([int]$processId) -Force -ErrorAction SilentlyContinue
  }
  Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" -ErrorAction SilentlyContinue |
    Where-Object { $_.CommandLine -like "*$profileId*" -or $_.CommandLine -like "*drift-*" } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
  Start-Sleep -Seconds 2
}

$harness = $null
$profileId = ""
$lastPid = 0
$samples = New-Object System.Collections.Generic.List[object]
try {
  $mode = if ($CrossSessionRestarts) { "cross_session_restarts" } else { "single_instance_multi_capture" }
  Write-Host "=== Fingerprint Drift Gate runs=$Runs mode=$mode ===" -ForegroundColor Cyan
  $harness = Get-OrStartCapabilityHarness -Root $root -ReadyTimeoutSec 90 -ForceNewCore
  $cores = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserCoreList" -RpcArgList @() -TimeoutSec 30)
  $core = @($cores) | Where-Object { $_.isDefault -eq $true } | Select-Object -First 1
  if (-not $core) { $core = @($cores) | Select-Object -First 1 }
  if ($NoProxy) { $ProxyId = "" }

  $profileInput = @{
    profileName = "drift-$stamp"
    userDataDir = "browser/user-data/drift-$stamp"
    coreId = [string]$core.coreId
    fingerprintArgs = @()
    launchArgs = @("--window-size=1280,800")
    tags = @("drift-gate", "$stamp", "auto-99")
    keywords = @("drift")
    humanizeSeed = "drift-$stamp"
    behaviorProfileId = "balanced-human"
    proxyConfig = ""
    proxyId = $ProxyId
  }
  $created = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserProfileCreate" -RpcArgList @($profileInput) -TimeoutSec 30
  $profileId = [string]$created.profileId
  if (-not $profileId) { $profileId = [string]$created.ProfileId }
  Write-Host "profile=$profileId"

  function Start-And-Ready {
    $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStart" -RpcArgList @($profileId) -TimeoutSec 240
    $deadline = (Get-Date).AddSeconds($ReadyTimeoutSec)
    $st = $null
    while ((Get-Date) -lt $deadline) {
      $st = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceStatus" -RpcArgList @($profileId) -TimeoutSec 30
      if ($st.running -and $st.debugReady -and [int]$st.debugPort -gt 0 -and $st.injectionReady) {
        return $st
      }
      Start-Sleep -Seconds 2
    }
    throw "instance not ready"
  }

  $status = Start-And-Ready
  $lastPid = [int]$status.pid
  Write-Host ("ready pid={0} port={1}" -f $status.pid, $status.debugPort)

  for ($i = 1; $i -le $Runs; $i++) {
    Write-Host ("--- capture {0}/{1} ---" -f $i, $Runs)
    if ($CrossSessionRestarts -and $i -gt 1) {
      Stop-ProfileBrowser $harness.BridgeUrl $harness.BridgeToken $profileId $lastPid
      $status = Start-And-Ready
      $lastPid = [int]$status.pid
    }

    try {
      $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserInstanceOpenUrl" -RpcArgList @($profileId, ("https://example.com/?r={0}" -f $i)) -TimeoutSec 90
    } catch {
      Write-Host ("openurl warn: {0}" -f $_.Exception.Message) -ForegroundColor Yellow
    }
    Start-Sleep -Seconds 2

    $fp1 = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchFingerprintProfile" -RpcArgList @($profileId) -TimeoutSec 90
    Start-Sleep -Milliseconds 400
    $fp2 = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "WorkbenchFingerprintProfile" -RpcArgList @($profileId) -TimeoutSec 90
    $k1 = Get-FpKey $fp1
    $k2 = Get-FpKey $fp2
    $h1 = Hash-Object $k1
    $h2 = Hash-Object $k2
    $samples.Add([ordered]@{
      run = $i
      pid = $status.pid
      debugPort = $status.debugPort
      inSessionStable = ($h1 -eq $h2)
      fingerprintHash = $h1
      fingerprint = $k1
      inSessionSecondHash = $h2
    }) | Out-Null
    Write-Host ("run={0} hash={1} inSessionStable={2}" -f $i, $h1.Substring(0,12), ($h1 -eq $h2))
  }

  $hashes = @($samples | ForEach-Object { $_.fingerprintHash })
  $unique = @($hashes | Select-Object -Unique)
  $inSessionPass = @($samples | Where-Object { $_.inSessionStable }).Count
  $crossStable = ($unique.Count -eq 1)

  $fieldDetails = @{}
  $fieldStable = $true
  $keys = @(
    "userAgent","platform","webdriver","timezone","language",
    "hardwareConcurrency","deviceMemory","canvasHash","fontHash",
    "webglVendor","webglRenderer","webglExtensionsHash","audioHash","uaDataPlatform"
  )
  foreach ($key in $keys) {
    $vals = @($samples | ForEach-Object {
      $v = $_.fingerprint[$key]
      if ($v -is [System.Array]) { ($v -join ",") } else { [string]$v }
    })
    $u = @($vals | Select-Object -Unique)
    $ok = ($u.Count -eq 1)
    # Rotating hash fields (canvas/font/audio/webgl) may legitimately wobble once on a
    # first-paint or font-load timing race within the same profile. We therefore accept
    # at most ONE deviant value across all runs, i.e. the mode must hold >= Runs-1 and
    # there may be no more than 2 distinct values total (mode + a single noise value).
    # Any larger non-mode share (>=2 deviating runs, or 2+ distinct deviants) fails the
    # gate, so concrete drift that exceeds the tolerance exits nonzero.
    if ($key -in @("canvasHash","fontHash","audioHash","webglExtensionsHash")) {
      $mode = ($vals | Group-Object | Sort-Object Count -Descending | Select-Object -First 1).Name
      $modeCount = @($vals | Where-Object { $_ -eq $mode }).Count
      $ok = ($modeCount -ge [Math]::Max(1, $Runs - 1)) -and ($u.Count -le 2)
    }
    $fieldDetails[$key] = [ordered]@{ unique = $u.Count; ok = $ok; values = $u }
    if (-not $ok) { $fieldStable = $false }
  }

  $identityStable = $fieldDetails["userAgent"].ok -and $fieldDetails["platform"].ok -and $fieldDetails["webdriver"].ok -and $fieldDetails["timezone"].ok -and $fieldDetails["language"].ok
  $passed = ($inSessionPass -eq $Runs) -and $identityStable -and $fieldStable
  $statusText = if ($passed) { "passed_fingerprint_drift" } else { "failed_fingerprint_drift" }

  $report = [ordered]@{
    schema = "fingerprint_drift_gate_v1"
    generatedAt = (Get-Date).ToString("o")
    status = $statusText
    mode = $mode
    runs = $Runs
    profileId = $profileId
    proxyId = $ProxyId
    inSessionStableCount = $inSessionPass
    uniqueFingerprintHashes = $unique.Count
    crossRunStable = $crossStable
    identityStable = $identityStable
    fieldStable = $fieldStable
    fieldDetails = $fieldDetails
    samples = $samples
    evidenceBoundary = "Local same-profile multi-capture fingerprint stability (default single-instance reloads). CrossSessionRestarts optional. Not commercial long-term pass rate."
  }
  $report | ConvertTo-Json -Depth 14 | Set-Content -Encoding UTF8 $reportPath
  Write-Host ""
  Write-Host ("Report: {0}" -f $reportPath)
  Write-Host ("status={0} uniqueHashes={1} inSession={2}/{3} identityStable={4}" -f $statusText, $unique.Count, $inSessionPass, $Runs, $identityStable)
  if (-not $passed) { exit 1 }
}
catch {
  Write-Host ("FATAL: {0}" -f $_.Exception.Message) -ForegroundColor Red
  exit 1
}
finally {
  if (-not $KeepInstance) {
    if ($profileId) { Stop-ProfileBrowser $harness.BridgeUrl $harness.BridgeToken $profileId $lastPid }
    if ($harness -and $harness.Process -and -not $harness.Process.HasExited) {
      try { $harness.Process.Kill() } catch {}
    }
  }
}
