param(
  [string]$ProxyConfig = "http://127.0.0.1:7897",
  [string]$ProxyName = "clash-mixed-7897",
  [string]$ProxyId = "proxy-clash-mixed-7897"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")
$root = Get-CapabilityProjectRoot
Set-Location $root
$env:PERSONAL_PILOT_APP_ROOT = $root

Write-Host "=== Ensure Clash proxy in app store ===" -ForegroundColor Cyan
$harness = Get-OrStartCapabilityHarness -Root $root -ReadyTimeoutSec 90 -ForceNewCore
try {
  $list = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserProxyList" -RpcArgList @())
  $updated = @()
  $found = $false
  foreach ($p in $list) {
    $id = [string]$p.proxyId
    if ($id -eq $ProxyId) {
      $found = $true
      $updated += @{
        proxyId      = $ProxyId
        proxyName    = $ProxyName
        proxyConfig  = $ProxyConfig
        groupName    = "clash-local"
        sortOrder    = 1
      }
    } else {
      $updated += $p
    }
  }
  if (-not $found) {
    $updated = @(@{
      proxyId     = $ProxyId
      proxyName   = $ProxyName
      proxyConfig = $ProxyConfig
      groupName   = "clash-local"
      sortOrder   = 1
    }) + $updated
  }

  # PowerShell may splat arrays; wrap as a single nested argument.
  $saveArgs = New-Object System.Collections.ArrayList
  [void]$saveArgs.Add(@($updated))
  $null = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "SaveBrowserProxies" -RpcArgList $saveArgs.ToArray() -TimeoutSec 60
  Write-Host ("saved proxyId={0} config={1} total={2}" -f $ProxyId, $ProxyConfig, @($updated).Count)

  $health = Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserProxyCheckIPHealth" -RpcArgList @($ProxyId)
  Write-Host ("health ok={0} ip={1} country={2} city={3} residential={4} fraud={5}" -f $health.ok, $health.ip, $health.country, $health.city, $health.isResidential, $health.fraudScore)
  Write-Host ("health_json={0}" -f ($health | ConvertTo-Json -Compress -Depth 6))
} finally {
  if ($harness.Process -and -not $harness.Process.HasExited) {
    try { $harness.Process.Kill() } catch {}
  }
}

Write-Host ""
Write-Host "=== Stealth probe with Clash proxy ===" -ForegroundColor Cyan
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "fingerprint_stealth_probe_live.ps1") -ProxyId $ProxyId
exit $LASTEXITCODE
