param(
  [string]$ProjectRoot = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($ProjectRoot)) {
  $ProjectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
} else {
  $ProjectRoot = (Resolve-Path -LiteralPath $ProjectRoot).Path
}

$releaseExe = Join-Path $ProjectRoot "src-tauri\target\release\personal-pilot-tauri.exe"
$rootExe = Join-Path $ProjectRoot "personal-pilot-tauri.exe"

if (-not (Test-Path -LiteralPath $releaseExe -PathType Leaf)) {
  throw "Release executable not found: $releaseExe"
}

$resolvedRelease = (Resolve-Path -LiteralPath $releaseExe).Path
if (-not $resolvedRelease.StartsWith($ProjectRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
  throw "Release executable is outside project root: $resolvedRelease"
}

Copy-Item -LiteralPath $resolvedRelease -Destination $rootExe -Force

$duplicateEntries = @(
  (Join-Path $ProjectRoot "src-tauri\target\release\persona-pilot-desktop.exe"),
  (Join-Path $ProjectRoot "src-tauri\target\release\PersonaPilot.exe"),
  (Join-Path $ProjectRoot "portable.exe")
)

foreach ($entry in $duplicateEntries) {
  if (-not (Test-Path -LiteralPath $entry)) { continue }
  $resolved = (Resolve-Path -LiteralPath $entry).Path
  if (-not $resolved.StartsWith($ProjectRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to delete outside project: $resolved"
  }
  if ($resolved -ieq (Resolve-Path -LiteralPath $rootExe).Path) { continue }
  Remove-Item -LiteralPath $resolved -Force
}

$item = Get-Item -LiteralPath $rootExe
Write-Host "[OK] Main entry: $($item.FullName)"
Write-Host "[OK] Size: $($item.Length) bytes"
