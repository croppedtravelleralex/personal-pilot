param(
  [switch]$RequireProviderSmoke,
  [switch]$Live,
  [string]$OutputDir = "data/reports/provider-smoke"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$credentialKeys = @(
  "CAPTCHA_PROVIDER_KEY",
  "SMS_PROVIDER_KEY",
  "EMAIL_PROVIDER_KEY"
)

$missing = @()
$present = @()
foreach ($name in $credentialKeys) {
  $value = [Environment]::GetEnvironmentVariable($name)
  if ([string]::IsNullOrWhiteSpace($value)) {
    $missing += $name
  } else {
    $present += $name
  }
}

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$reportPath = Join-Path $absoluteOutputDir ("provider-smoke-{0}.json" -f $timestamp)

if ($present.Count -eq 0) {
  $report = [ordered]@{
    status = "blocked_missing_credentials"
    missing_credentials = $missing
    present_credentials = @()
    balance_check = "skipped_no_credentials"
    generated_at = (Get-Date).ToUniversalTime().ToString("o")
  }
  $report | ConvertTo-Json -Depth 6 | Set-Content -Path $reportPath -Encoding UTF8
  if ($RequireProviderSmoke) {
    Write-Error "provider smoke required but credentials missing: $($missing -join ', ')"
    exit 1
  }
  Write-Host "provider smoke: no credentials configured (report: $reportPath)"
  exit 0
}

if ($Live) {
  $report = [ordered]@{
    status = "blocked_live_not_implemented"
    present_credentials = $present
    missing_credentials = $missing
    balance_check = "skipped_live_not_implemented"
    message = "Live provider balance checks are not implemented; refusing paid API calls."
    generated_at = (Get-Date).ToUniversalTime().ToString("o")
  }
  $report | ConvertTo-Json -Depth 6 | Set-Content -Path $reportPath -Encoding UTF8
  Write-Host "provider smoke: live mode not implemented (report: $reportPath)"
  exit 1
}

$report = [ordered]@{
  status = "partial"
  present_credentials = $present
  missing_credentials = $missing
  balance_check = "skipped_no_live_call"
  generated_at = (Get-Date).ToUniversalTime().ToString("o")
}
$report | ConvertTo-Json -Depth 6 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "provider smoke: partial credentials present (report: $reportPath)"
exit 0
