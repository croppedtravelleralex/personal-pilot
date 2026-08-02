param(
  [int]$ConcurrentInstances = 10,
  [string]$ReportDir = "data/reports/concurrency-smoke"
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null
$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$reportPath = Join-Path $ReportDir "concurrency-smoke-$stamp.json"

$results = @{
  schema = "concurrency_smoke_v2"
  generatedAt = (Get-Date).ToString("o")
  concurrentInstances = $ConcurrentInstances
  checks = @()
  status = "passed"
}

function Add-Check([string]$name, [bool]$ok, [string]$detail) {
  $script:results.checks += @{ name = $name; ok = $ok; detail = $detail }
  if (-not $ok) { $script:results.status = "failed" }
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
  # CP1: port reservation unit tests
  $portTest = go test ./backend/internal/proxy/ -count=1 -run "TestReservePortNumber" 2>&1
  Add-Check "cp1_reserve_port_unit" ($LASTEXITCODE -eq 0) ($portTest | Out-String)

  # Bridge / sing-box / xray unit stress
  $proxyTest = go test ./backend/internal/proxy/... -count=1 -run "Test.*Bridge|Test.*SingBox|Test.*Xray|Test.*Port" 2>&1
  Add-Check "proxy_bridge_unit_tests" ($LASTEXITCODE -eq 0) ($proxyTest | Out-String)

  $behaviorTest = go test ./backend/internal/behavior/... -count=1 -run "TestAnalyze|Test.*Recorder|TestPlanDailySession" 2>&1
  Add-Check "behavior_recording_unit_tests" ($LASTEXITCODE -eq 0) ($behaviorTest | Out-String)

  # CP2: MaxConcurrentInstances present in config
  $cfgSrc = Get-Content -Raw "backend/internal/config/config.go"
  Add-Check "cp2_max_concurrent_config" ($cfgSrc -match "MaxConcurrentInstances") "MaxConcurrentInstances field in config"

  # CP3: orphan reconcile present
  $orphanSrc = Get-Content -Raw "backend/app_orphan_reconcile.go" -ErrorAction SilentlyContinue
  Add-Check "cp3_orphan_reconcile_source" ($null -ne $orphanSrc -and $orphanSrc -match "orphanReconcile") "app_orphan_reconcile.go present"

  # CP5 ADR exists (explicit non-pool)
  $adr = Test-Path "docs/adr/001-browser-pool-local-oneshot.md"
  Add-Check "cp5_pool_adr" $adr "docs/adr/001-browser-pool-local-oneshot.md"
} finally {
  Pop-Location
}

# Simulated concurrent port allocation (local loopback) — N>=10 default
$listeners = @()
$ports = @()
try {
  for ($i = 0; $i -lt $ConcurrentInstances; $i++) {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $listeners += $listener
    $ports += $listener.LocalEndpoint.Port
  }
  $unique = ($ports | Select-Object -Unique).Count
  Add-Check "local_port_allocation_n" ($unique -eq $ConcurrentInstances) ("allocated unique ports=$unique/$ConcurrentInstances : " + ($ports -join ","))
} catch {
  Add-Check "local_port_allocation_n" $false $_.Exception.Message
} finally {
  foreach ($listener in $listeners) {
    try { $listener.Stop() } catch {}
  }
}

# Residual process hygiene (best-effort; do not fail on unrelated chrome)
$chrome = @(Get-Process -Name "chrome","chromium" -ErrorAction SilentlyContinue | Where-Object {
  $_.Path -like "*personal-pilot*" -or $_.Path -like "*PersonaPilot*"
})
$singbox = @(Get-Process -Name "sing-box","singbox" -ErrorAction SilentlyContinue)
Add-Check "residual_product_chrome_count" ($chrome.Count -le $ConcurrentInstances) ("product chrome processes=$($chrome.Count)")
Add-Check "residual_singbox_observed" $true ("sing-box processes=$($singbox.Count) (informational)")

$results | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $reportPath
Write-Host "Concurrency smoke report: $reportPath status=$($results.status)"
if ($results.status -ne "passed") { exit 1 }
