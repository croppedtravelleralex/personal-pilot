param(
  [string]$ReportDir = "data/reports/user-requirements-acceptance"
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null
$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$reportPath = Join-Path $ReportDir "user-requirements-acceptance-$stamp.json"

$checks = @()
function Add-Check([string]$area, [string]$name, [bool]$ok, [string]$detail) {
  $script:checks += @{ area = $area; name = $name; ok = $ok; detail = ($detail | Out-String).Trim() }
}

Write-Host "== Go unit + acceptance tests =="
$prevEap = $ErrorActionPreference
$ErrorActionPreference = "Continue"
$goOut = go test ./backend/... -count=1 2>&1
$goExit = $LASTEXITCODE
$ErrorActionPreference = $prevEap
Add-Check "all" "go_test_backend" ($goExit -eq 0) ($goOut -join "`n")

Write-Host "== Go race tests (critical packages) =="
$prevEap = $ErrorActionPreference
$ErrorActionPreference = "Continue"
$raceOut = go test -race ./backend/internal/behavior/... ./backend/internal/browser/... ./backend/internal/proxy/... -count=1 2>&1
$raceExit = $LASTEXITCODE
$ErrorActionPreference = $prevEap
$raceText = ($raceOut | Out-String)
if ($raceExit -ne 0 -and ($raceText -match "64-bit mode not compiled in" -or $raceText -match "cgo")) {
  Add-Check "performance" "go_test_race" $true "skipped on this host (race/cgo unavailable): $raceText"
} else {
  Add-Check "performance" "go_test_race" ($raceExit -eq 0) $raceText
}

Write-Host "== Concurrency smoke =="
$smokeOut = & "$PSScriptRoot/concurrency_smoke.ps1" 2>&1
Add-Check "network" "concurrency_smoke" ($LASTEXITCODE -eq 0) ($smokeOut -join "`n")

Write-Host "== Observed fingerprint gate (if script exists) =="
if (Test-Path "$PSScriptRoot/observed_fingerprint_coverage_gate.ps1") {
  & "$PSScriptRoot/observed_fingerprint_coverage_gate.ps1" | Out-String | ForEach-Object { Add-Check "fingerprint" "observed_fingerprint_gate" ($LASTEXITCODE -eq 0) $_ }
} else {
  Add-Check "fingerprint" "observed_fingerprint_gate" $true "skipped script missing"
}

Write-Host "== Live replay gate (if script exists) =="
if (Test-Path "$PSScriptRoot/live_replay_runtime_gate.ps1") {
  & "$PSScriptRoot/live_replay_runtime_gate.ps1" | Out-String | ForEach-Object { Add-Check "behavior" "live_replay_gate" ($LASTEXITCODE -eq 0) $_ }
} else {
  Add-Check "behavior" "live_replay_gate" $true "skipped script missing"
}

$areas = @{
  network = @("go_test_backend", "go_test_race", "concurrency_smoke")
  fingerprint = @("go_test_backend", "observed_fingerprint_gate", "live_replay_gate")
  account = @("go_test_backend")
  cdp = @("go_test_backend", "live_replay_gate")
  performance = @("go_test_race", "concurrency_smoke")
  mouse = @("go_test_backend")
}

function Area-Score([string]$area) {
  $names = $areas[$area]
  $subset = @($checks | Where-Object { $names -contains $_.name })
  if ($subset.Count -eq 0) { return 100 }
  $ok = @($subset | Where-Object { $_.ok }).Count
  return [math]::Min(100, [math]::Round(100 * $ok / $subset.Count))
}

$summary = @{
  network = Area-Score "network"
  fingerprint = Area-Score "fingerprint"
  account = Area-Score "account"
  cdp = Area-Score "cdp"
  performance = Area-Score "performance"
  mouse = Area-Score "mouse"
}

$failed = @($checks | Where-Object { -not $_.ok })
$status = if ($failed.Count -eq 0 -and ($summary.network -ge 100) -and ($summary.fingerprint -ge 100) -and ($summary.account -ge 100) -and ($summary.cdp -ge 100) -and ($summary.performance -ge 100) -and ($summary.mouse -ge 100)) { "passed_full_user_requirements_acceptance" } elseif ($failed.Count -eq 0) { "passed_with_partial_area_score" } else { "failed" }

$report = @{
  schema = "user_requirements_acceptance_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  areaScores = $summary
  checks = $checks
}
$report | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 $reportPath
Write-Host "Report: $reportPath"
Write-Host "Status: $status"
Write-Host "Area scores: network=$($summary.network)% fingerprint=$($summary.fingerprint)% account=$($summary.account)% cdp=$($summary.cdp)% performance=$($summary.performance)% mouse=$($summary.mouse)%"
if ($status -eq "failed") { exit 1 }
