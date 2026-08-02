param(
  [string]$ProjectRoot = "",
  [string]$BinaryPath = "",
  [string]$ProxyServer = "",
  [int]$TimeoutSeconds = 60,
  [string]$OutputDir = "data/reports/detector-site-headed-smoke"
)

$ErrorActionPreference = "Stop"
$root = if ($ProjectRoot) { (Resolve-Path $ProjectRoot).Path } else { (Resolve-Path (Join-Path $PSScriptRoot "..")).Path }
Set-Location $root
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$reportPath = Join-Path $OutputDir "detector-site-headed-smoke-$stamp.json"

$sites = @(
  @{ id = "browserleaks-ip"; url = "https://browserleaks.com/ip" },
  @{ id = "browserleaks-webrtc"; url = "https://browserleaks.com/webrtc" },
  @{ id = "pixelscan"; url = "https://pixelscan.net/" }
)

$checks = @()
function Add-Check([string]$name, [bool]$ok, [string]$detail) {
  $script:checks += @{ name = $name; ok = $ok; detail = ($detail | Out-String).Trim() }
}

foreach ($site in $sites) {
  Write-Host "== Headed smoke: $($site.id) =="
  $args = @(
    "-File", (Join-Path $PSScriptRoot "headed_external_smoke.ps1"),
    "-Action", "get_title",
    "-Url", $site.url,
    "-TimeoutSeconds", $TimeoutSeconds,
    "-NoReport"
  )
  if ($BinaryPath) { $args += @("-BinaryPath", $BinaryPath) }
  if ($ProxyServer) { $args += @("-ProxyServer", $ProxyServer) }
  $out = powershell -NoProfile -ExecutionPolicy Bypass @args 2>&1
  Add-Check $site.id ($LASTEXITCODE -eq 0) ($out -join "`n")
}

$failed = @($checks | Where-Object { -not $_.ok })
$status = if ($failed.Count -eq 0) { "passed_detector_site_headed_smoke" } else { "failed" }
$report = @{
  schema = "detector_site_headed_smoke_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  proxyServer = $ProxyServer
  checks = $checks
  manualReview = @(
    "Visually confirm IP matches proxy exit on browserleaks/ip",
    "Confirm no local/private ICE on browserleaks/webrtc",
    "Review pixelscan bot/fingerprint consistency",
    "For automation: call WorkbenchRunDetectionBundle(profileId) inside the app with a running profile"
  )
}
$report | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $reportPath
Write-Host "Report: $reportPath"
Write-Host "Status: $status"
if ($status -eq "failed") { exit 1 }
