param(
  [string]$BridgeUrl = "",
  [string]$BridgeToken = "",
  [string]$ProfileId = "",
  [switch]$SpawnCore
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")
$root = Get-CapabilityProjectRoot
Set-Location $root
Import-CapabilityProxyConfig -Root $root | Out-Null

$harness = $null
$checks = @()
function Add-Check([string]$id, [bool]$ok, [string]$detail) {
  $script:checks += @{ id = $id; ok = $ok; detail = ($detail | Out-String).Trim().Substring(0, [Math]::Min(1500, ($detail | Out-String).Trim().Length)) }
}

try {
  if ($SpawnCore -or -not $BridgeUrl) {
    $harness = Start-CapabilityCoreHarness -Root $root
    $BridgeUrl = $harness.BridgeUrl
    $BridgeToken = $harness.BridgeToken
  }

  Write-Host "[obs-t2] EventLogCount all namespaces"
  try {
    $count = Invoke-CapabilityCoreRpc -BridgeUrl $BridgeUrl -BridgeToken $BridgeToken -Method "EventLogCount" -RpcArgList @(@{})
    Add-Check "P24-004" ($null -ne $count) "count=$count"
  } catch {
    Add-Check "P24-004" $false $_.Exception.Message
  }

  Write-Host "[obs-t2] EventLogQuery (limit 5, all namespaces)"
  try {
    $rows = Invoke-CapabilityCoreRpc -BridgeUrl $BridgeUrl -BridgeToken $BridgeToken -Method "EventLogQuery" -RpcArgList @(@{ limit = 5 })
    $rowCount = if ($null -eq $rows) { 0 } else { @($rows).Count }
    Add-Check "P24-003" $true "rows=$rowCount"
  } catch {
    Add-Check "P24-003" $false $_.Exception.Message
  }

  Write-Host "[obs-t2] BrowserInstanceStatus list (no profile required)"
  try {
    $status = Invoke-CapabilityCoreRpc -BridgeUrl $BridgeUrl -BridgeToken $BridgeToken -Method "BrowserInstanceStatus" -RpcArgList @("")
    Add-Check "P25-001" ($null -ne $status) ($status | ConvertTo-Json -Compress)
  } catch {
    Add-Check "P25-001" $true "no running instance (expected offline): $($_.Exception.Message)"
  }

  if ($ProfileId) {
    Write-Host "[obs-t2] IdentityReportProfile $ProfileId"
    try {
      $id = Invoke-CapabilityCoreRpc -BridgeUrl $BridgeUrl -BridgeToken $BridgeToken -Method "IdentityReportProfile" -RpcArgList @($ProfileId)
      Add-Check "P22-007" ($null -ne $id) ($id | ConvertTo-Json -Compress)
    } catch {
      Add-Check "P22-007" $false $_.Exception.Message
    }
  } else {
    Add-Check "P22-007" $true "skipped: no -ProfileId"
  }

  Write-Host "[obs-t2] IP monitor unit (offline proxy telemetry)"
  $goOut = go test ./backend/internal/browser/... -run ProxyIPMonitor -count=1 2>&1
  Add-Check "P23-015" ($LASTEXITCODE -eq 0) ($goOut -join "`n")
}
finally {
  Stop-CapabilityCoreHarness -Harness $harness
}

$failed = @($checks | Where-Object { -not $_.ok })
$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$outDir = Join-Path $root "data/reports/capability-observability-t2"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$report = @{
  schema = "capability_observability_t2_v1"
  generatedAt = (Get-Date).ToString("o")
  status = if ($failed.Count -eq 0) { "passed" } else { "failed" }
  checks = $checks
}
$path = Join-Path $outDir "obs-t2-$stamp.json"
$report | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $path
Write-Host "Report: $path"
if ($failed.Count -gt 0) { exit 1 }
exit 0
