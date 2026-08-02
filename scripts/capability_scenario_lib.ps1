# Shared helpers for capability scenario suite (v4)

function Get-CapabilityProjectRoot {
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Import-CapabilityProxyConfig {
  param([string]$Root = "")
  if (-not $Root) { $Root = Get-CapabilityProjectRoot }
  $paths = @(
    (Join-Path $Root "data/scenarios/fixtures/proxy.local.env"),
    (Join-Path $Root "scripts/scenarios/fixtures/proxy.local.env")
  )
  foreach ($p in $paths) {
    if (-not (Test-Path $p)) { continue }
    Get-Content $p | ForEach-Object {
      $line = $_.Trim()
      if ($line -eq "" -or $line.StartsWith("#")) { return }
      $idx = $line.IndexOf("=")
      if ($idx -lt 1) { return }
      $key = $line.Substring(0, $idx).Trim()
      $val = $line.Substring($idx + 1).Trim()
      if ($key) { Set-Item -Path "Env:$key" -Value $val }
    }
    break
  }
  $host_ = $env:CAPABILITY_PROXY_HOST
  $port = $env:CAPABILITY_PROXY_PORT
  $user = $env:CAPABILITY_PROXY_USERNAME
  $pass = $env:CAPABILITY_PROXY_PASSWORD
  $proto = if ($env:CAPABILITY_PROXY_PROTOCOL) { $env:CAPABILITY_PROXY_PROTOCOL } else { "socks5" }
  if (-not $host_ -or -not $port) { return $null }
  $auth = ""
  if ($user) {
    $encUser = [uri]::EscapeDataString($user)
    $encPass = if ($pass) { [uri]::EscapeDataString($pass) } else { "" }
    $auth = "${encUser}:${encPass}@"
  }
  return [pscustomobject]@{
    Name = $env:CAPABILITY_PROXY_NAME
    Group = $env:CAPABILITY_PROXY_GROUP
    Protocol = $proto
    Host = $host_
    Port = [int]$port
    ProxyServer = "${proto}://${auth}${host_}:${port}"
  }
}

function Get-LatestGateReport {
  param(
    [string]$Directory,
    [string]$Pattern = "*.json"
  )
  if (-not (Test-Path $Directory)) { return $null }
  $file = Get-ChildItem -LiteralPath $Directory -Filter $Pattern -File -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $file) { return $null }
  return Get-Content -LiteralPath $file.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
}

function Invoke-CapabilityCoreRpc {
  param(
    [string]$BridgeUrl,
    [string]$BridgeToken,
    [string]$Method,
    [object[]]$RpcArgList = @(),
    [int]$TimeoutSec = 120
  )
  $encodedArgs = @()
  foreach ($item in $RpcArgList) {
    if ($null -eq $item) {
      $encodedArgs += $null
      continue
    }
    $encodedArgs += ,$item
  }
  $bodyObj = @{ method = $Method; args = $encodedArgs }
  $body = $bodyObj | ConvertTo-Json -Depth 12 -Compress
  $headers = @{
    "Content-Type" = "application/json"
    "X-Personal-Pilot-Bridge-Token" = $BridgeToken
  }
  $resp = Invoke-RestMethod -Uri "$($BridgeUrl.TrimEnd('/'))/rpc" -Method Post -Headers $headers -Body $body -TimeoutSec $TimeoutSec
  if (-not $resp.ok) {
    throw "RPC $Method failed: $($resp.error)"
  }
  return $resp.result
}

function Start-CapabilityCoreHarness {
  param(
    [string]$Root = "",
    [int]$ReadyTimeoutSec = 45
  )
  if (-not $Root) { $Root = Get-CapabilityProjectRoot }
  $coreExe = Join-Path $Root "bin/personal-pilot-core.exe"
  Push-Location (Join-Path $Root "backend")
  go build -o $coreExe ./cmd/personal-pilot-core
  if ($LASTEXITCODE -ne 0) { Pop-Location; throw "failed to build personal-pilot-core" }
  Pop-Location
  if (-not (Test-Path $coreExe)) {
    throw "personal-pilot-core not found after build"
  }

  $psi = New-Object System.Diagnostics.ProcessStartInfo
  $psi.FileName = $coreExe
  $psi.Arguments = "-app-root `"$Root`""
  $psi.WorkingDirectory = $Root
  $psi.UseShellExecute = $false
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.CreateNoWindow = $true
  $proc = [System.Diagnostics.Process]::Start($psi)

  $ready = $null
  $deadline = (Get-Date).AddSeconds($ReadyTimeoutSec)
  while ((Get-Date) -lt $deadline) {
    if ($proc.HasExited) { break }
    $line = $proc.StandardOutput.ReadLine()
    if ($line -match "^PERSONAL_PILOT_CORE_READY\s+(.+)$") {
      $ready = $Matches[1] | ConvertFrom-Json
      break
    }
    Start-Sleep -Milliseconds 200
  }
  if (-not $ready) {
    try { $proc.Kill() } catch {}
    throw "core harness did not become ready within ${ReadyTimeoutSec}s"
  }
  return [pscustomobject]@{
    Process = $proc
    BridgeUrl = [string]$ready.bridgeUrl
    BridgeToken = [string]$ready.bridgeToken
    EventUrl = [string]$ready.eventUrl
  }
}

function Test-LaunchServerHealthy {
  param(
    [string]$BaseUrl = "http://127.0.0.1:19876",
    [int]$TimeoutSec = 5
  )
  try {
    $health = Invoke-RestMethod -Uri "$($BaseUrl.TrimEnd('/'))/api/health" -TimeoutSec $TimeoutSec
    return ($health.ok -eq $true)
  } catch {
    return $false
  }
}

function Get-OrStartCapabilityHarness {
  param(
    [string]$Root = "",
    [int]$ReadyTimeoutSec = 60,
    [switch]$ForceNewCore,
    [string]$LaunchBase = "http://127.0.0.1:19876"
  )
  if (-not $Root) { $Root = Get-CapabilityProjectRoot }
  $launchBase = $LaunchBase.TrimEnd("/")
  if (-not $ForceNewCore -and (Test-LaunchServerHealthy -BaseUrl $launchBase)) {
    return [pscustomobject]@{
      Process     = $null
      BridgeUrl   = ""
      BridgeToken = ""
      EventUrl    = ""
      Owned       = $false
      HttpOnly    = $true
      LaunchBase  = $launchBase
    }
  }
  $harness = Start-CapabilityCoreHarness -Root $Root -ReadyTimeoutSec $ReadyTimeoutSec
  $launchInfo = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "GetLaunchServerInfo"
  return [pscustomobject]@{
    Process     = $harness.Process
    BridgeUrl   = $harness.BridgeUrl
    BridgeToken = $harness.BridgeToken
    EventUrl    = $harness.EventUrl
    Owned       = $true
    HttpOnly    = $false
    LaunchBase  = [string]$launchInfo.baseUrl
  }
}

function Stop-CapabilityCoreHarness {
  param($Harness)
  if (-not $Harness) { return }
  if ($Harness.PSObject.Properties.Match("Owned").Count -gt 0 -and -not $Harness.Owned) { return }
  try {
    if ($Harness.BridgeUrl -and $Harness.BridgeToken) {
      $body = '{"mode":"app-only"}'
      $headers = @{
        "Content-Type" = "application/json"
        "X-Personal-Pilot-Bridge-Token" = $Harness.BridgeToken
      }
      Invoke-RestMethod -Uri "$($Harness.BridgeUrl.TrimEnd('/'))/shutdown" -Method Post -Headers $headers -Body $body -TimeoutSec 10 | Out-Null
    }
  } catch {}
  if ($Harness.Process -and -not $Harness.Process.HasExited) {
    Start-Sleep -Seconds 2
    if (-not $Harness.Process.HasExited) {
      try { $Harness.Process.Kill() } catch {}
    }
  }
}

function Measure-ObservabilityScore {
  param(
    [hashtable]$Subscores
  )
  $weights = @{
    CollectionSLI = 0.20
    PortraitCoverage = 0.20
    IPStabilitySLO = 0.20
    EventTaxonomyCoverage = 0.20
    StateMonitoring = 0.10
    CrossCorrelation = 0.10
  }
  $total = 0.0
  foreach ($k in $weights.Keys) {
    $v = if ($Subscores.ContainsKey($k)) { [double]$Subscores[$k] } else { 0.0 }
    $total += $weights[$k] * $v
  }
  return [math]::Round($total, 1)
}

function Get-CapabilityRadarDimensions {
  return @(
    @{ id = "data_collection"; label = "Data Collection"; designTarget = 100 },
    @{ id = "state_monitoring"; label = "State Monitoring"; designTarget = 100 },
    @{ id = "fingerprint_portrait"; label = "Fingerprint Portrait"; designTarget = 100 },
    @{ id = "ip_stability"; label = "IP Stability"; designTarget = 100 },
    @{ id = "fingerprint_data"; label = "Fingerprint Data"; designTarget = 100 },
    @{ id = "event_taxonomy"; label = "Event Taxonomy"; designTarget = 100 },
    @{ id = "cross_correlation"; label = "Cross Signal"; designTarget = 100 },
    @{ id = "platform_business"; label = "Platform Business"; designTarget = 100 }
  )
}

function Update-RadarFromResults {
  param(
    [array]$Results,
    [hashtable]$Radar
  )
  $dimMap = @{
    data_collection = @("data_collection_pipeline", "cdp_capture", "platform_scrape")
    state_monitoring = @("state_monitoring", "telemetry_dashboard")
    fingerprint_portrait = @("fingerprint_portrait", "fingerprint")
    ip_stability = @("ip_stability_telemetry", "network_stealth")
    fingerprint_data = @("fingerprint_data_continuity", "session_portability")
    event_taxonomy = @("event_taxonomy_monitoring", "observability_events")
    cross_correlation = @("cross_signal_correlation")
    platform_business = @("platform_xhs", "platform_auth_api", "asymmetric_cost", "account_cadence")
  }
  $keys = @($Radar.Keys)
  foreach ($axis in $keys) {
    $dims = $dimMap[$axis]
    if (-not $dims) { continue }
    $related = @($Results | Where-Object {
      $hit = $false
      foreach ($d in @($_.dimensions)) {
        if ($dims -contains $d) { $hit = $true; break }
      }
      $hit
    })
    if ($related.Count -eq 0) { continue }
    $passed = @($related | Where-Object { $_.status -eq "passed" }).Count
    $Radar[$axis] = [math]::Round(100.0 * $passed / $related.Count, 1)
  }
}
