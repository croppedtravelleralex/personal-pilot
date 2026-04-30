param(
  [string]$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

$ErrorActionPreference = "Stop"

$binDir = Join-Path $ProjectRoot "bin"
New-Item -ItemType Directory -Path $binDir -Force | Out-Null

$plainExe = Join-Path $binDir "personal-pilot-core.exe"
$tauriSidecarExe = Join-Path $binDir "personal-pilot-core-x86_64-pc-windows-msvc.exe"

Push-Location $ProjectRoot
try {
  go build -trimpath -ldflags "-s -w" -o $plainExe ./backend/cmd/personal-pilot-core
  Copy-Item -LiteralPath $plainExe -Destination $tauriSidecarExe -Force
  Write-Host "[OK] Built sidecar: $plainExe"
  Write-Host "[OK] Built Tauri sidecar copy: $tauriSidecarExe"
} finally {
  Pop-Location
}
