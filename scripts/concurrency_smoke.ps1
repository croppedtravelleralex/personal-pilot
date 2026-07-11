param(
  [int]$ConcurrentInstances = 3,
  [string]$ReportDir = "data/reports/concurrency-smoke"
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null
$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$reportPath = Join-Path $ReportDir "concurrency-smoke-$stamp.json"

$results = @{
  schema = "concurrency_smoke_v1"
  generatedAt = (Get-Date).ToString("o")
  concurrentInstances = $ConcurrentInstances
  checks = @()
  status = "passed"
}

function Add-Check([string]$name, [bool]$ok, [string]$detail) {
  $script:results.checks += @{ name = $name; ok = $ok; detail = $detail }
  if (-not $ok) { $script:results.status = "failed" }
}

# Bridge refcount stress (Go unit tests)
Push-Location (Join-Path $PSScriptRoot "..")
try {
  $proxyTest = go test ./backend/internal/proxy/... -count=1 -run "Test.*Bridge|Test.*SingBox|Test.*Xray" 2>&1
  Add-Check "proxy_bridge_unit_tests" ($LASTEXITCODE -eq 0) ($proxyTest | Out-String)
  $behaviorTest = go test ./backend/internal/behavior/... -count=1 -run "TestAnalyze|Test.*Recorder" 2>&1
  Add-Check "behavior_recording_unit_tests" ($LASTEXITCODE -eq 0) ($behaviorTest | Out-String)
} finally {
  Pop-Location
}

# Simulated concurrent port allocation (local loopback)
$ports = @()
try {
  for ($i = 0; $i -lt $ConcurrentInstances; $i++) {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $ports += $listener.LocalEndpoint.Port
  }
  Add-Check "local_port_allocation" $true ("allocated ports: " + ($ports -join ","))
} catch {
  Add-Check "local_port_allocation" $false $_.Exception.Message
} finally {
  foreach ($listener in @()) { }
}

$results | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $reportPath
Write-Host "Concurrency smoke report: $reportPath status=$($results.status)"
if ($results.status -ne "passed") { exit 1 }
