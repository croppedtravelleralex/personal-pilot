param(
  [string]$ProjectRoot = "",
  [string]$ProxyUrl = "",
  [string]$ProxyUrlEnvName = "PERSONA_PILOT_REMOTE_PROXY_URL",
  [string]$TargetUrl = "https://tls.peet.ws/api/all",
  [string]$ExpectedExitIp = "",
  [int]$TimeoutSeconds = 30,
  [string]$OutputDir = "data/reports/remote-proxy-tls",
  [switch]$SkipDirectBaseline,
  [switch]$AllowBlocked
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  if (-not [string]::IsNullOrWhiteSpace($ProjectRoot)) {
    return (Resolve-Path $ProjectRoot).Path
  }
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Write-JsonNoBom([string]$Path, [object]$Value, [int]$Depth = 10) {
  $json = $Value | ConvertTo-Json -Depth $Depth
  $encoding = [System.Text.UTF8Encoding]::new($false)
  [System.IO.File]::WriteAllText($Path, $json, $encoding)
}

function Redact-ProxyUrl([string]$Value) {
  if ([string]::IsNullOrWhiteSpace($Value)) { return "" }
  try {
    $uri = [System.Uri]::new($Value)
    if ([string]::IsNullOrWhiteSpace($uri.UserInfo)) { return $Value }
    $builder = [System.UriBuilder]::new($uri)
    $builder.UserName = "***"
    $builder.Password = "***"
    return $builder.Uri.AbsoluteUri
  } catch {
    return ($Value -replace '://([^/@]+)@', '://***@')
  }
}

function Redact-Text([string]$Value, [string]$Secret) {
  if ($null -eq $Value) { return "" }
  $redacted = $Value
  if (-not [string]::IsNullOrWhiteSpace($Secret)) {
    $redacted = $redacted.Replace($Secret, (Redact-ProxyUrl $Secret))
  }
  return $redacted
}

function Shorten([string]$Value, [int]$MaxLength = 1200) {
  if ($null -eq $Value) { return "" }
  if ($Value.Length -le $MaxLength) { return $Value }
  return $Value.Substring(0, $MaxLength) + "...<truncated>"
}

function New-Check([string]$Id, [bool]$Passed, [string]$Evidence, [bool]$Required = $true) {
  return [ordered]@{
    id = $Id
    status = if ($Passed) { "passed" } elseif ($Required) { "failed" } else { "not_run" }
    required = $Required
    evidence = $Evidence
  }
}

function Get-JsonField([object]$Value, [string[]]$Path) {
  $current = $Value
  foreach ($key in $Path) {
    if ($null -eq $current) { return $null }
    $property = $current.PSObject.Properties[$key]
    if ($null -eq $property) { return $null }
    $current = $property.Value
  }
  return $current
}

function Get-ObservationFields([object]$Parsed) {
  $observedIp = [string](Get-JsonField $Parsed @("ip"))
  $httpVersion = [string](Get-JsonField $Parsed @("http_version"))
  $tlsJa3 = [string](Get-JsonField $Parsed @("tls", "ja3"))
  $tlsJa3Hash = [string](Get-JsonField $Parsed @("tls", "ja3_hash"))
  $tlsJa4 = [string](Get-JsonField $Parsed @("tls", "ja4"))
  $tlsVersion = [string](Get-JsonField $Parsed @("tls", "tls_version_record"))
  if ([string]::IsNullOrWhiteSpace($tlsVersion)) { $tlsVersion = [string](Get-JsonField $Parsed @("tls", "version")) }

  return [ordered]@{
    exitIp = $observedIp
    httpVersion = $httpVersion
    tlsJa3 = $tlsJa3
    tlsJa3Hash = $tlsJa3Hash
    tlsJa4 = $tlsJa4
    tlsVersion = $tlsVersion
  }
}

function Test-TlsObserved([object]$Observation) {
  if ($null -eq $Observation) { return $false }
  return (-not [string]::IsNullOrWhiteSpace([string]$Observation["tlsJa3Hash"])) -or
    (-not [string]::IsNullOrWhiteSpace([string]$Observation["tlsJa3"])) -or
    (-not [string]::IsNullOrWhiteSpace([string]$Observation["tlsJa4"]))
}

function Invoke-CurlObservation([string]$CurlPath, [string[]]$Arguments, [string]$WorkDir, [string]$Name, [string]$Secret) {
  $stdoutPath = Join-Path $WorkDir ("{0}-stdout.json" -f $Name)
  $stderrPath = Join-Path $WorkDir ("{0}-stderr.txt" -f $Name)
  $startedAt = Get-Date
  $process = Start-Process -FilePath $CurlPath -ArgumentList $Arguments -NoNewWindow -Wait -PassThru -RedirectStandardError $stderrPath
  $durationMs = [int]((Get-Date) - $startedAt).TotalMilliseconds
  $exitCode = $process.ExitCode
  $stdout = if (Test-Path $stdoutPath) { Get-Content -Path $stdoutPath -Raw } else { "" }
  $stderr = if (Test-Path $stderrPath) { Get-Content -Path $stderrPath -Raw } else { "" }

  $parsed = $null
  try {
    if (-not [string]::IsNullOrWhiteSpace($stdout)) { $parsed = $stdout | ConvertFrom-Json }
  } catch {
    $parsed = $null
  }

  return [ordered]@{
    exitCode = $exitCode
    durationMs = $durationMs
    stdout = $stdout
    stderr = $stderr
    observed = Get-ObservationFields $parsed
    stdoutPreview = Shorten (Redact-Text $stdout $Secret) 1600
    stderrPreview = Shorten (Redact-Text $stderr $Secret) 1200
  }
}

function New-DirectBaselineArguments([string]$Target, [int]$MaxTimeSeconds, [string]$StdoutPath) {
  return @(
    "--silent",
    "--show-error",
    "--location",
    "--max-time", [string]$MaxTimeSeconds,
    "--noproxy", "*",
    "--output", $StdoutPath,
    $Target
  )
}

$root = Resolve-ProjectRoot
$timestamp = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
$absoluteOutputDir = Join-Path $root $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null
$reportPath = Join-Path $absoluteOutputDir ("remote-proxy-tls-probe-{0}.json" -f $timestamp)

$envProxy = if ([string]::IsNullOrWhiteSpace($ProxyUrlEnvName)) { "" } else { [Environment]::GetEnvironmentVariable($ProxyUrlEnvName) }
if ([string]::IsNullOrWhiteSpace($ProxyUrl)) { $ProxyUrl = $envProxy }
$proxyRedacted = Redact-ProxyUrl $ProxyUrl

if ([string]::IsNullOrWhiteSpace($ProxyUrl)) {
  $report = [ordered]@{
    schemaVersion = "remote_proxy_tls_probe_v1"
    generatedAt = (Get-Date).ToString("o")
    projectRoot = $root
    status = "blocked_remote_proxy_required"
    failureReason = "No remote proxy URL was provided; set -ProxyUrl or PERSONA_PILOT_REMOTE_PROXY_URL before claiming remote egress/TLS evidence."
    targetUrl = $TargetUrl
    proxyConfigured = $false
    proxyUrlRedacted = ""
    observationScope = "remote_proxy_egress_and_tls_fingerprint_observation"
    directBaseline = [ordered]@{
      status = "not_run_remote_proxy_required"
      evidenceBoundary = "direct baseline is only used as a comparison after a remote proxy is configured"
    }
    checks = @(
      (New-Check "remote_proxy_configured" $false "ProxyUrl parameter or $ProxyUrlEnvName environment variable is required.")
    )
    observed = [ordered]@{}
    notes = @(
      "This blocked report is intentional: local direct outbound or config validation must not be counted as remote proxy egress/TLS evidence.",
      "The proxy URL is never written with credentials; reports use a redacted URL."
    )
  }
  Write-JsonNoBom $reportPath $report 10
  Write-Host "Remote proxy TLS probe report: $reportPath"
  Write-Host "Status: $($report.status)"
  Write-Host "Failure reason: $($report.failureReason)"
  if ($AllowBlocked) { exit 0 }
  exit 2
}

$curl = Get-Command curl.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if ($null -eq $curl) {
  $report = [ordered]@{
    schemaVersion = "remote_proxy_tls_probe_v1"
    generatedAt = (Get-Date).ToString("o")
    projectRoot = $root
    status = "failed"
    failureReason = "curl.exe is required to run the proxy-aware TLS observation probe."
    targetUrl = $TargetUrl
    proxyConfigured = $true
    proxyUrlRedacted = $proxyRedacted
    observationScope = "remote_proxy_egress_and_tls_fingerprint_observation"
    checks = @(
      (New-Check "remote_proxy_configured" $true "Remote proxy URL provided."),
      (New-Check "curl_available" $false "curl.exe was not found on PATH.")
    )
    observed = [ordered]@{}
  }
  Write-JsonNoBom $reportPath $report 10
  Write-Host "Remote proxy TLS probe report: $reportPath"
  Write-Host "Status: $($report.status)"
  Write-Host "Failure reason: $($report.failureReason)"
  exit 1
}

$workDir = Join-Path $root (Join-Path ".codex_tmp\remote-proxy-tls" $timestamp)
New-Item -ItemType Directory -Force -Path $workDir | Out-Null

$proxiedArguments = @(
  "--silent",
  "--show-error",
  "--location",
  "--max-time", [string]$TimeoutSeconds,
  "--proxy", $ProxyUrl,
  "--output", (Join-Path $workDir "proxied-stdout.json"),
  $TargetUrl
)

$proxiedResult = Invoke-CurlObservation $curl.Source $proxiedArguments $workDir "proxied" $ProxyUrl
$exitCode = [int]$proxiedResult["exitCode"]
$durationMs = [int]$proxiedResult["durationMs"]
$proxiedObserved = $proxiedResult["observed"]
$observedIp = [string]$proxiedObserved["exitIp"]
$httpVersion = [string]$proxiedObserved["httpVersion"]
$tlsJa3 = [string]$proxiedObserved["tlsJa3"]
$tlsJa3Hash = [string]$proxiedObserved["tlsJa3Hash"]
$tlsJa4 = [string]$proxiedObserved["tlsJa4"]
$tlsVersion = [string]$proxiedObserved["tlsVersion"]

$egressObserved = $exitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($observedIp)
$tlsObserved = $exitCode -eq 0 -and (Test-TlsObserved $proxiedObserved)
$expectedExitMatches = if ([string]::IsNullOrWhiteSpace($ExpectedExitIp)) { $null } else { $observedIp -eq $ExpectedExitIp }

$directBaseline = [ordered]@{
  status = "skipped"
  exitIp = $null
  httpVersion = $null
  tlsJa3Hash = $null
  tlsJa4 = $null
  exitIpDifferentFromProxied = $null
  tlsFingerprintDifferentFromProxied = $null
  durationMs = $null
  failureReason = "Direct baseline skipped by -SkipDirectBaseline."
  evidenceBoundary = "direct baseline compares local direct egress with proxied egress; it is not proxy proof by itself"
}
if (-not $SkipDirectBaseline) {
  $directArguments = New-DirectBaselineArguments $TargetUrl $TimeoutSeconds (Join-Path $workDir "direct-stdout.json")
  $directResult = Invoke-CurlObservation $curl.Source $directArguments $workDir "direct" ""
  $directObserved = $directResult["observed"]
  $directExitCode = [int]$directResult["exitCode"]
  $directExitIp = [string]$directObserved["exitIp"]
  $directTlsJa3Hash = [string]$directObserved["tlsJa3Hash"]
  $directTlsJa4 = [string]$directObserved["tlsJa4"]
  $directBaseline = [ordered]@{
    status = if ($directExitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($directExitIp)) { "observed" } else { "failed" }
    exitIp = if ([string]::IsNullOrWhiteSpace($directExitIp)) { $null } else { $directExitIp }
    httpVersion = if ([string]::IsNullOrWhiteSpace([string]$directObserved["httpVersion"])) { $null } else { [string]$directObserved["httpVersion"] }
    tlsJa3Hash = if ([string]::IsNullOrWhiteSpace($directTlsJa3Hash)) { $null } else { $directTlsJa3Hash }
    tlsJa4 = if ([string]::IsNullOrWhiteSpace($directTlsJa4)) { $null } else { $directTlsJa4 }
    exitIpDifferentFromProxied = if ($egressObserved -and -not [string]::IsNullOrWhiteSpace($directExitIp)) { $directExitIp -ne $observedIp } else { $null }
    tlsFingerprintDifferentFromProxied = if ($tlsObserved -and (-not [string]::IsNullOrWhiteSpace($directTlsJa3Hash) -or -not [string]::IsNullOrWhiteSpace($directTlsJa4))) { ($directTlsJa3Hash -ne $tlsJa3Hash) -or ($directTlsJa4 -ne $tlsJa4) } else { $null }
    durationMs = [int]$directResult["durationMs"]
    failureReason = if ($directExitCode -eq 0) { "" } else { "direct baseline curl request failed" }
    proxyPolicy = "forced_no_proxy_with_curl_noproxy_star"
    evidenceBoundary = "direct baseline compares local direct egress with proxied egress; it is not proxy proof by itself"
    stderrPreview = $directResult["stderrPreview"]
  }
}

$checks = @(
  (New-Check "remote_proxy_configured" $true "Remote proxy URL provided as $proxyRedacted."),
  (New-Check "curl_available" $true "curl.exe found at $($curl.Source)."),
  (New-Check "proxied_request_succeeded" ($exitCode -eq 0) "curl exitCode=$exitCode durationMs=$durationMs."),
  (New-Check "egress_ip_observed" $egressObserved "observedIp=$observedIp."),
  (New-Check "tls_fingerprint_observed" $tlsObserved "ja3Hash=$tlsJa3Hash ja4=$tlsJa4."),
  (New-Check "direct_baseline_recorded" ($SkipDirectBaseline -or [string]$directBaseline["status"] -eq "observed") "directBaselineStatus=$($directBaseline["status"])." $false)
)
if (-not [string]::IsNullOrWhiteSpace($ExpectedExitIp)) {
  $checks += New-Check "expected_exit_ip_match" ([bool]$expectedExitMatches) "expected=$ExpectedExitIp observed=$observedIp."
}

$status = if ($exitCode -ne 0) {
  "failed"
} elseif ($egressObserved -and $tlsObserved -and ($null -eq $expectedExitMatches -or $expectedExitMatches)) {
  "passed_remote_proxy_tls_observed"
} elseif ($egressObserved) {
  "partial_remote_proxy_egress_observed"
} else {
  "failed"
}

$failureReason = switch ($status) {
  "passed_remote_proxy_tls_observed" { "" }
  "partial_remote_proxy_egress_observed" {
    if ($expectedExitMatches -eq $false) { "remote proxy egress was observed, but the exit IP did not match ExpectedExitIp" }
    elseif (-not $tlsObserved) { "remote proxy egress was observed, but TLS fingerprint fields were missing from the target response" }
    else { "remote proxy egress was observed, but the probe did not meet all pass criteria" }
  }
  default {
    if ($exitCode -ne 0) { "proxied curl request failed" } else { "remote proxy egress observation did not return a parseable exit IP" }
  }
}

$commandPreview = "curl.exe --silent --show-error --location --max-time $TimeoutSeconds --proxy $proxyRedacted --output <stdout> $TargetUrl"
$report = [ordered]@{
  schemaVersion = "remote_proxy_tls_probe_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $root
  status = $status
  failureReason = $failureReason
  targetUrl = $TargetUrl
  proxyConfigured = $true
  proxyUrlRedacted = $proxyRedacted
  observationScope = "remote_proxy_egress_and_tls_fingerprint_observation"
  expectedExitIp = if ([string]::IsNullOrWhiteSpace($ExpectedExitIp)) { $null } else { $ExpectedExitIp }
  observed = [ordered]@{
    exitIp = $observedIp
    httpVersion = $httpVersion
    tlsJa3 = $tlsJa3
    tlsJa3Hash = $tlsJa3Hash
    tlsJa4 = $tlsJa4
    tlsVersion = $tlsVersion
    expectedExitIpMatched = $expectedExitMatches
  }
  directBaseline = $directBaseline
  curl = [ordered]@{
    command = $commandPreview
    exitCode = $exitCode
    durationMs = $durationMs
    stderrPreview = $proxiedResult["stderrPreview"]
    stdoutPreview = $proxiedResult["stdoutPreview"]
  }
  checks = $checks
  notes = @(
    "This report is runtime egress/TLS evidence for the configured remote proxy only.",
    "It does not prove browser-scoped WebRTC/DNS leak closure or full headed runtime coherence.",
    "Local transport_binary_smoke proves config acceptance only; this probe records external observation."
  )
}

Write-JsonNoBom $reportPath $report 12
Write-Host "Remote proxy TLS probe report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }
if ($status -eq "failed") { exit 1 }
if ($status -eq "partial_remote_proxy_egress_observed" -and -not $AllowBlocked) { exit 2 }
