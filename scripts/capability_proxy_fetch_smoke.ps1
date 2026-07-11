param(
  [string]$Url = "https://api.ipify.org?format=json",
  [string]$OutputDir = "data/reports/capability-proxy-fetch"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")
$root = Get-CapabilityProjectRoot
Set-Location $root
Import-CapabilityProxyConfig -Root $root | Out-Null
$proxy = Import-CapabilityProxyConfig -Root $root
if (-not $proxy) { Write-Error "proxy.local.env missing" }

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$reportPath = Join-Path $OutputDir "proxy-fetch-$stamp.json"

$checks = @()
function Add-Check([string]$id, [bool]$ok, [string]$detail) {
  $script:checks += @{ id = $id; ok = $ok; detail = ($detail | Out-String).Trim().Substring(0, [Math]::Min(1200, ($detail | Out-String).Trim().Length)) }
}

# ipify via proxy
Write-Host "[proxy-fetch] ipify via udeal"
$ipifyOut = & curl.exe -sS --max-time 45 -x $proxy.ProxyServer $Url 2>&1
Add-Check "P23-015-ipify" ($LASTEXITCODE -eq 0 -and $ipifyOut -match '"ip"') ($ipifyOut -join "`n")

# example.com HTML collection
Write-Host "[proxy-fetch] example.com html"
$htmlOut = & curl.exe -sS --max-time 45 -x $proxy.ProxyServer "https://example.com" 2>&1
Add-Check "P21-002-html" ($LASTEXITCODE -eq 0 -and $htmlOut -match "<html") ($htmlOut.Substring(0, [Math]::Min(200, $htmlOut.Length)))

# httpbin html page
Write-Host "[proxy-fetch] httpbin html"
$binOut = & curl.exe -sS --max-time 45 -x $proxy.ProxyServer "https://httpbin.org/html" 2>&1
Add-Check "P21-003-fetch" ($LASTEXITCODE -eq 0 -and $binOut.Length -gt 100) "bytes=$($binOut.Length)"

$failed = @($checks | Where-Object { -not $_.ok })
$status = if ($failed.Count -eq 0) { "passed" } else { "failed" }
$report = @{
  schema = "capability_proxy_fetch_smoke_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  proxyServer = $proxy.ProxyServer
  checks = $checks
}
$report | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $reportPath
Write-Host "Report: $reportPath"
if ($status -eq "failed") { exit 1 }
exit 0
