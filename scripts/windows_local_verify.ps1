param(
    [switch]$SkipDesktopRelease,
    [switch]$SkipFullCargoTest,
    [switch]$SkipContinuityTest
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot

function Invoke-Step {
    param(
        [string]$Name,
        [scriptblock]$Action
    )

    Write-Host ""
    Write-Host "==> $Name" -ForegroundColor Cyan
    & $Action
}

try {
    Invoke-Step "pnpm typecheck" { pnpm typecheck }
    Invoke-Step "pnpm build" { pnpm build }
    Invoke-Step "Win11 baseline enforcement" {
        powershell -ExecutionPolicy Bypass -File `
            C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 `
            -ProjectRoot $repoRoot
    }
    Invoke-Step "cargo test --lib -- --test-threads=1" { cargo test --lib -- --test-threads=1 }

    if (-not $SkipDesktopRelease) {
        Invoke-Step "pnpm desktop:release" { pnpm desktop:release }
    }

    if (-not $SkipFullCargoTest) {
        Invoke-Step "cargo test --quiet" { cargo test --quiet }
    }

    if (-not $SkipContinuityTest) {
        Invoke-Step "cargo test --test integration_continuity_control_plane -- --nocapture" {
            cargo test --test integration_continuity_control_plane -- --nocapture
        }
    }

    Write-Host ""
    Write-Host "Local Windows verification completed." -ForegroundColor Green
}
finally {
    Pop-Location
}
