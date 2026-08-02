# Sprint K — 99+ asymmetric stealth readiness gate
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

Write-Host "=== Asymmetric Stealth Gate (99+) ===" -ForegroundColor Cyan

Write-Host "[1/3] go test asymmetric + detection packages..."
Push-Location backend
go test ./internal/asymmetric/... ./internal/detection/... -count=1
if ($LASTEXITCODE -ne 0) { Pop-Location; exit 1 }
Pop-Location

Write-Host "[2/3] matrix unit: 99+ scenario..."
Push-Location backend
go test ./internal/asymmetric/... -run TestStealthMatrix99PlusWithTrustAndBonuses -count=1 -v
if ($LASTEXITCODE -ne 0) { Pop-Location; exit 1 }
Pop-Location

Write-Host "[3/3] compile backend..."
Push-Location backend
go build -o NUL ./cmd/personal-pilot-core
if ($LASTEXITCODE -ne 0) { Pop-Location; exit 1 }
Pop-Location

Write-Host ""
Write-Host "PASS: asymmetric stealth gate (matrix 99+ unit + compile)" -ForegroundColor Green
Write-Host "Note: live CreepJS/WebRTC probe requires running profile — use WorkbenchRunStealthProbeSuite in headed smoke."
