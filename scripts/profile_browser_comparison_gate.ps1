param(
  [string]$OutputDir = "data/reports/profile-browser-comparison"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$validationDir = Join-Path $projectRoot "data\validation-reports"
$reports = @()
if (Test-Path $validationDir) {
  Get-ChildItem -Path $validationDir -Filter *.json -File | ForEach-Object {
    try {
      $value = Get-Content $_.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
      $reports += [ordered]@{ path = $_.FullName; generatedAt = $value.generatedAt; report = $value }
    } catch {}
  }
}
$reports = @($reports | Sort-Object generatedAt -Descending)
$latest = if ($reports.Count -gt 0) { $reports[0] } else { $null }
$signals = if ($null -ne $latest) { @($latest.report.signals) } else { @() }
$desktopSignals = @($signals | Where-Object { $_.collectorScope -eq "desktop-webview" -or $_.runtimeAdapter -eq "desktop_webview" })
$profileSignals = @($signals | Where-Object { $_.targetProfileBrowser -eq $true -or $_.collectorScope -eq "profile-browser" })
$categories = @("webrtc", "canvas", "audio", "leak", "fingerprint")
$categoryComparison = foreach ($category in $categories) {
  $desktopCount = @($desktopSignals | Where-Object { $_.category -eq $category }).Count
  $profileCount = @($profileSignals | Where-Object { $_.category -eq $category }).Count
  [ordered]@{
    category = $category
    desktopWebViewObserved = $desktopCount
    profileBrowserObserved = $profileCount
    status = if ($desktopCount -gt 0 -and $profileCount -gt 0) { "comparable" } elseif ($desktopCount -gt 0) { "desktop_only" } elseif ($profileCount -gt 0) { "profile_only" } else { "missing" }
  }
}
$comparableCount = @($categoryComparison | Where-Object { $_.status -eq "comparable" }).Count
$status = if ($comparableCount -eq $categories.Count) {
  "passed"
} elseif ($latest -eq $null) {
  "blocked_missing_validation_report"
} else {
  "partial_comparison_only"
}
$failureReason = if ($status -eq "passed") { "" } elseif ($latest -eq $null) { "no validation report found" } else { "desktop WebView and profile browser evidence are not both present for every category" }

$report = [ordered]@{
  schemaVersion = "profile_browser_comparison_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  latestValidationReportPath = if ($latest) { $latest.path } else { $null }
  desktopSignalCount = $desktopSignals.Count
  profileBrowserSignalCount = $profileSignals.Count
  comparableCategoryCount = $comparableCount
  categoryComparison = $categoryComparison
  failureReason = $failureReason
  notes = @(
    "This gate compares existing reports only; it does not launch a profile browser.",
    "Desktop WebView evidence must not be treated as profile-browser proof."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("profile-browser-comparison-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Profile browser comparison report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }
if ($status -like "blocked*") { exit 2 }
