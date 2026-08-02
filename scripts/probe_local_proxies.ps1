param()
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")
$root = Get-CapabilityProjectRoot
Set-Location $root
$env:PERSONAL_PILOT_APP_ROOT = $root

Write-Host "=== Local ports ==="
foreach ($port in 7890,7891,7897,9097,18082,18090,20171,20172,1080,10808,10809) {
  try {
    $ok = Test-NetConnection -ComputerName 127.0.0.1 -Port $port -WarningAction SilentlyContinue -InformationLevel Quiet
    if ($ok) { Write-Host "OPEN 127.0.0.1:$port" }
  } catch {}
}

Write-Host "=== Clash controller ==="
try {
  $ver = Invoke-RestMethod -Uri "http://127.0.0.1:9097/version" -TimeoutSec 3
  Write-Host ("version={0}" -f ($ver | ConvertTo-Json -Compress))
} catch {
  Write-Host ("version_err={0}" -f $_.Exception.Message)
}
try {
  $cfg = Invoke-RestMethod -Uri "http://127.0.0.1:9097/configs" -TimeoutSec 3
  Write-Host ("mode={0} mixed-port={1} socks-port={2} port={3}" -f $cfg.mode, $cfg.'mixed-port', $cfg.'socks-port', $cfg.port)
} catch {
  Write-Host ("configs_err={0}" -f $_.Exception.Message)
}
try {
  $proxies = Invoke-RestMethod -Uri "http://127.0.0.1:9097/proxies" -TimeoutSec 5
  $names = @($proxies.proxies.PSObject.Properties.Name)
  Write-Host ("proxy_groups_or_nodes={0}" -f $names.Count)
  $names | Select-Object -First 30 | ForEach-Object { Write-Host ("  " + $_) }
} catch {
  Write-Host ("proxies_err={0}" -f $_.Exception.Message)
}

Write-Host "=== App proxy list sample ==="
$harness = Get-OrStartCapabilityHarness -Root $root -ReadyTimeoutSec 60 -ForceNewCore
try {
  $list = @(Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method "BrowserProxyList" -RpcArgList @())
  Write-Host ("app_proxy_count={0}" -f $list.Count)
  $shown = 0
  foreach ($p in $list) {
    $id = [string]$p.proxyId
    if (-not $id -or $id.StartsWith("__")) { continue }
    $cfg = [string]$p.proxyConfig
    $name = [string]$p.proxyName
    $json = [string]$p.lastIPHealthJson
    $ok = $json -match '"ok"\s*:\s*true'
    $kind = "other"
    if ($cfg -match '^(socks5|socks5h|http|https)://') { $kind = "standard" }
    elseif ($cfg -match 'type:\s*anytls|anytls://') { $kind = "anytls" }
    elseif ($cfg -match 'type:\s*hysteria|hysteria2://') { $kind = "hysteria" }
    elseif ($cfg -match 'vmess|vless|trojan|ss://') { $kind = "xray" }
    if ($kind -eq "standard" -or $ok -or $name -match 'clash|udeal|US|JP|LA|美国|日本') {
      Write-Host ("id={0} kind={1} ok={2} lat={3} name={4}" -f $id, $kind, $ok, $p.lastLatencyMs, $name)
      Write-Host ("  cfg={0}" -f $cfg.Substring(0, [Math]::Min(120, $cfg.Length)))
      $shown++
      if ($shown -ge 25) { break }
    }
  }
} finally {
  if ($harness.Process -and -not $harness.Process.HasExited) {
    try { $harness.Process.Kill() } catch {}
  }
}
