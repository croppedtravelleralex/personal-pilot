# C4 AC2: TLS/UA coherence probe via peet.ws
param(
  [string]$ProxyUrl = "http://127.0.0.1:7897",
  [string]$ExpectedUAMajor = "139",
  [string]$TargetUrl = "https://tls.peet.ws/api/all",
  [string]$ChromeUA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.7258.154 Safari/537.36",
  [int]$TimeoutSeconds = 30,
  [switch]$AllowBlocked
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root
$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$reportDir = Join-Path $root "data\reports\tls-ua-coherence"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null
$reportPath = Join-Path $reportDir ("tls-ua-coherence-$stamp.json")

function Redact-Proxy([string]$Value) {
  if ([string]::IsNullOrWhiteSpace($Value)) { return "" }
  return ($Value -replace '://([^/@]+)@', '://***@')
}

function Get-JsonPath($obj, [string[]]$path) {
  $cur = $obj
  foreach ($p in $path) {
    if ($null -eq $cur) { return $null }
    $prop = $cur.PSObject.Properties[$p]
    if ($null -eq $prop) { return $null }
    $cur = $prop.Value
  }
  return $cur
}

$checks = New-Object System.Collections.Generic.List[object]
function Add-Check([string]$id, [bool]$ok, [string]$detail) {
  $checks.Add([ordered]@{ id=$id; ok=$ok; detail=$detail }) | Out-Null
  $tone = if ($ok) { "Green" } else { "Red" }
  Write-Host ("[{0}] {1}: {2}" -f $(if($ok){"PASS"}else{"FAIL"}), $id, $detail) -ForegroundColor $tone
}

Write-Host "=== TLS/UA Coherence Probe ===" -ForegroundColor Cyan
Write-Host ("proxy={0} target={1}" -f (Redact-Proxy $ProxyUrl), $TargetUrl)

$parsed = $null
$raw = ""
try {
  $curlArgs = @("-sS", "--max-time", "$TimeoutSeconds", "-A", $ChromeUA)
  if (-not [string]::IsNullOrWhiteSpace($ProxyUrl)) { $curlArgs += @("-x", $ProxyUrl) }
  $curlArgs += $TargetUrl
  $raw = & curl.exe @curlArgs 2>&1 | Out-String
  $parsed = $raw | ConvertFrom-Json
  Add-Check "tls_endpoint_reachable" $true ("bytes={0}" -f $raw.Length)
} catch {
  Add-Check "tls_endpoint_reachable" $false $_.Exception.Message
}

$ua = ""; $ja3 = ""; $ja3Hash = ""; $ja4 = ""; $httpVersion = ""; $ip = ""
if ($parsed) {
  $ua = [string](Get-JsonPath $parsed @("user_agent"))
  if (-not $ua) { $ua = [string](Get-JsonPath $parsed @("ua")) }
  $ja3 = [string](Get-JsonPath $parsed @("tls","ja3"))
  if (-not $ja3) { $ja3 = [string](Get-JsonPath $parsed @("ja3")) }
  $ja3Hash = [string](Get-JsonPath $parsed @("tls","ja3_hash"))
  if (-not $ja3Hash) { $ja3Hash = [string](Get-JsonPath $parsed @("ja3_hash")) }
  $ja4 = [string](Get-JsonPath $parsed @("tls","ja4"))
  if (-not $ja4) { $ja4 = [string](Get-JsonPath $parsed @("ja4")) }
  $httpVersion = [string](Get-JsonPath $parsed @("http_version"))
  $ip = [string](Get-JsonPath $parsed @("ip"))
}

$uaMajor = ""
if ($ua -match "Chrome/(\d+)") { $uaMajor = $Matches[1] }
Add-Check "ua_present" (-not [string]::IsNullOrWhiteSpace($ua)) ("ua=$ua")
Add-Check "ua_major_expected" ($uaMajor -eq $ExpectedUAMajor -or [string]::IsNullOrWhiteSpace($ExpectedUAMajor)) ("observed=$uaMajor expected=$ExpectedUAMajor")
Add-Check "ja3_observed" ((-not [string]::IsNullOrWhiteSpace($ja3)) -or (-not [string]::IsNullOrWhiteSpace($ja3Hash))) ("ja3=$ja3 ja3_hash=$ja3Hash")
Add-Check "http_version_observed" (-not [string]::IsNullOrWhiteSpace($httpVersion)) ("httpVersion=$httpVersion")

$headerCoherent = ($uaMajor -ne "") -and ($uaMajor -eq $ExpectedUAMajor)
$transportObserved = ((-not [string]::IsNullOrWhiteSpace($ja3)) -or (-not [string]::IsNullOrWhiteSpace($ja3Hash)) -or (-not [string]::IsNullOrWhiteSpace($ja4)))
$coherent = $headerCoherent -and $transportObserved
Add-Check "tls_ua_header_coherent" $headerCoherent ("uaMajor=$uaMajor expected=$ExpectedUAMajor")
Add-Check "tls_transport_observed" $transportObserved ("ja3Present=$([bool]$ja3) ja3HashPresent=$([bool]$ja3Hash) ja4Present=$([bool]$ja4)")
Add-Check "tls_ua_coherent_proxy_path" $coherent ("headerCoherent=$headerCoherent transportObserved=$transportObserved")

$templateOk = $false
try {
  Push-Location (Join-Path $root "backend")
  $goOut = & go test ./internal/transport -count=1 -run "TestTLSUACoherent|TestChromeMajorTLSBaseline" 2>&1 | Out-String
  $code = $LASTEXITCODE
  Pop-Location
  $templateOk = ($code -eq 0)
  Add-Check "tls_template_unit" $templateOk ("go test transport exit=$code")
} catch {
  try { Pop-Location } catch {}
  Add-Check "tls_template_unit" $false $_.Exception.Message
}

$failed = @($checks | Where-Object { -not $_.ok })
$status = if ($failed.Count -eq 0) { "passed_tls_ua_coherence" } elseif ($AllowBlocked) { "blocked_or_partial" } else { "failed_tls_ua_coherence" }
$report = [ordered]@{
  schema = "tls_ua_coherence_probe_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  proxyUrlRedacted = (Redact-Proxy $ProxyUrl)
  targetUrl = $TargetUrl
  expectedUAMajor = $ExpectedUAMajor
  chromeUA = $ChromeUA
  observed = [ordered]@{
    ip = $ip
    userAgent = $ua
    uaMajor = $uaMajor
    ja3 = $ja3
    ja3Hash = $ja3Hash
    ja4 = $ja4
    httpVersion = $httpVersion
  }
  checks = $checks
  failedCount = $failed.Count
  evidenceBoundary = "Proxy-path TLS observation via peet.ws using Chrome UA header + local TLS template unit. curl ClientHello != Chromium ClientHello; browser-context JA3 remains optional deeper proof."
}
$report | ConvertTo-Json -Depth 10 | Set-Content -Encoding UTF8 $reportPath
Write-Host ""
Write-Host ("Report: {0} status={1}" -f $reportPath, $status)
if ($status -eq "failed_tls_ua_coherence") { exit 1 }
