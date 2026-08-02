# Zero-human 99+ autopilot gate
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

Write-Host "=== Stealth Autopilot Gate (zero-human) ===" -ForegroundColor Cyan

Write-Host "[1/4] trust harvest unit tests..."
Push-Location backend
go test ./internal/trust/... -count=1
if ($LASTEXITCODE -ne 0) { Pop-Location; exit 1 }

Write-Host "[2/4] autopilot unit tests..."
go test ./... -run "TestAutopilot|TestProfileWantsStealth|TestStealthMatrix99" -count=1
if ($LASTEXITCODE -ne 0) { Pop-Location; exit 1 }

Write-Host "[3/4] compile..."
go build -o NUL ./cmd/personal-pilot-core
if ($LASTEXITCODE -ne 0) { Pop-Location; exit 1 }
Pop-Location

Write-Host "[4/4] usage reminder..."
Write-Host "  Tag profile with 'stealth-autopilot' OR call AsymmetricAutoReach99Plus(profileId)"
Write-Host "  Optional env: PERSONAL_PILOT_MS_CLIENT_ID, PERSONAL_PILOT_MS_REFRESH_TOKEN"

Write-Host ""
Write-Host "PASS: stealth autopilot gate" -ForegroundColor Green
