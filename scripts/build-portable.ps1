$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$releaseExe = Join-Path $projectRoot "src-tauri\target\release\personal-pilot-tauri.exe"
$portableExe = Join-Path $projectRoot "portable.exe"
$bundleDir = Join-Path $projectRoot "src-tauri\target\release\bundle"

if (-not (Test-Path -LiteralPath $releaseExe -PathType Leaf)) {
  throw "Release executable not found: $releaseExe"
}

$resolvedRelease = (Resolve-Path -LiteralPath $releaseExe).Path
if (-not $resolvedRelease.StartsWith($projectRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
  throw "Release executable is outside project root: $resolvedRelease"
}

Copy-Item -LiteralPath $resolvedRelease -Destination $portableExe -Force

if (Test-Path -LiteralPath $bundleDir) {
  $resolvedBundle = (Resolve-Path -LiteralPath $bundleDir).Path
  if (-not $resolvedBundle.StartsWith($projectRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Bundle directory is outside project root: $resolvedBundle"
  }
  Remove-Item -LiteralPath $resolvedBundle -Recurse -Force
}

$item = Get-Item -LiteralPath $portableExe
Write-Host "[OK] Portable executable: $($item.FullName)"
Write-Host "[OK] Size: $($item.Length) bytes"
