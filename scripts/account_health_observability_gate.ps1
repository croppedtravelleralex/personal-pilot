# docs/55 AH5 — account health observability gate
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

Write-Host "=== Account Health Observability Gate ===" -ForegroundColor Cyan

$reportDir = "data/reports/account-health"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null
$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$reportPath = Join-Path $reportDir "account-health-gate-$stamp.json"

$results = @{
  schema = "account_health_observability_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  checks = @()
  status = "passed"
}

function Add-Check([string]$name, [bool]$ok, [string]$detail) {
  $script:results.checks += @{ name = $name; ok = $ok; detail = $detail }
  if (-not $ok) { $script:results.status = "failed" }
}

Write-Host "[1/5] unit tests: account health + lifecycle cadence..."
$testOut = go test ./backend/ -count=1 -run "TestBuildAttributionBucket|TestExtractJSONStringField|TestTopRiskBuckets|TestBuildHealthDailyRow" 2>&1
Add-Check "account_health_unit" ($LASTEXITCODE -eq 0) ($testOut | Out-String)

$lifeOut = go test ./backend/internal/behavior/lifecycle/ -count=1 -run "TestPlanDailySessionAppliesCadenceBudget" 2>&1
Add-Check "cadence_budget_unit" ($LASTEXITCODE -eq 0) ($lifeOut | Out-String)

$packOut = go test ./backend/internal/platformpack/ -count=1 2>&1
Add-Check "cadence_yaml_load" ($LASTEXITCODE -eq 0) ($packOut | Out-String)

Write-Host "[2/5] source wiring: runDailyLifecycleCycle + NoteAccountHealthObservation..."
$lifeSrc = Get-Content -Raw "backend/app_lifecycle.go"
Add-Check "warmup_engine_wired" ($lifeSrc -match "runDailyLifecycleCycle" -and $lifeSrc -match "LoadCadence") "lifecycle loads cadence.yaml"

$challengeSrc = Get-Content -Raw "backend/app_stealth_engine.go"
Add-Check "challenge_rollup_wired" ($challengeSrc -match "NoteAccountHealthObservation") "recordChallengeOnly notes health observation"
Add-Check "challenge_rate_feedback" ($challengeSrc -match "observedChallengeRatePct") "AsymmetricApplyFeedbackAuto uses observed challenge rate"

Write-Host "[3/5] RPC allowlist..."
$allow = Get-Content -Raw "backend/cmd/personal-pilot-core/main.go"
Add-Check "rpc_account_health_trend" ($allow -match '"AccountHealthTrend"') "AccountHealthTrend allowlisted"
Add-Check "rpc_challenge_attribution" ($allow -match '"ChallengeAttributionReport"') "ChallengeAttributionReport allowlisted"

Write-Host "[4/5] cadence.yaml exists..."
Add-Check "cadence_yaml_file" (Test-Path "platform-packs/xhs/cadence.yaml") "platform-packs/xhs/cadence.yaml"

Write-Host "[5/5] compile backend..."
Push-Location backend
go build -o NUL ./cmd/personal-pilot-core
Add-Check "core_compile" ($LASTEXITCODE -eq 0) "personal-pilot-core build"
Pop-Location

$results | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $reportPath
Write-Host "Report: $reportPath status=$($results.status)"
if ($results.status -ne "passed") { exit 1 }
Write-Host "PASS: account health observability gate" -ForegroundColor Green
