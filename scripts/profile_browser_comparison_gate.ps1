param(
  [string]$OutputDir = "data/reports/profile-browser-comparison",
  [int]$MaxPairAgeMinutes = 120
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

function ConvertTo-ReportSortKey($value, $fallback) {
  if ($null -ne $value) {
    $text = [string]$value
    $epochMs = 0L
    if ([Int64]::TryParse($text, [ref]$epochMs)) {
      return [DateTimeOffset]::FromUnixTimeMilliseconds($epochMs).UtcDateTime
    }

    try {
      return ([DateTimeOffset]::Parse($text)).UtcDateTime
    } catch {}
  }

  return $fallback.ToUniversalTime()
}

function Get-SignalField($signal, [string]$name) {
  if ($null -eq $signal) { return $null }
  $property = $signal.PSObject.Properties[$name]
  if ($null -ne $property) { return $property.Value }
  return $null
}

function Test-ReportProjectRootMatches($report) {
  $reportProjectRoot = [string](Get-SignalField $report "projectRoot")
  if ([string]::IsNullOrWhiteSpace($reportProjectRoot)) { return $true }
  $left = $reportProjectRoot.TrimEnd('\', '/')
  $right = $projectRoot.TrimEnd('\', '/')
  return $left.Equals($right, [System.StringComparison]::OrdinalIgnoreCase)
}

function Get-ReportSignals($report) {
  $values = @()

  if ($null -ne $report.signals) {
    $values += @($report.signals)
  }

  if ($null -ne $report.attempts) {
    foreach ($attempt in @($report.attempts)) {
      if ($null -ne $attempt.signals) {
        $values += @($attempt.signals)
      }
      if ($null -ne $attempt.rawResult -and $null -ne $attempt.rawResult.validation_signals) {
        $values += @($attempt.rawResult.validation_signals)
      }
    }
  }

  if ($null -ne $report.validation_signals) {
    $values += @($report.validation_signals)
  }

  if ($null -ne $report.realBinaryTask) {
    if ($null -ne $report.realBinaryTask.validation_signals) {
      $values += @($report.realBinaryTask.validation_signals)
    }
    if ($null -ne $report.realBinaryTask.result -and $null -ne $report.realBinaryTask.result.validation_signals) {
      $values += @($report.realBinaryTask.result.validation_signals)
    }
  }

  return @($values | Where-Object { $null -ne $_ -and $null -ne (Get-SignalField $_ "category") })
}

function Test-IsDesktopSignal($signal) {
  $collectorScope = [string](Get-SignalField $signal "collectorScope")
  $runtimeAdapter = [string](Get-SignalField $signal "runtimeAdapter")
  $detail = [string](Get-SignalField $signal "detail")
  return $collectorScope -in @("desktop-webview", "desktop_webview") `
    -or $runtimeAdapter -in @("desktop-webview", "desktop_webview") `
    -or $detail.Contains("scope=desktop-webview")
}

function Test-IsProfileSignal($signal) {
  $targetProfileBrowser = Get-SignalField $signal "targetProfileBrowser"
  $collectorScope = [string](Get-SignalField $signal "collectorScope")
  $runtimeAdapter = [string](Get-SignalField $signal "runtimeAdapter")
  $detail = [string](Get-SignalField $signal "detail")
  return $targetProfileBrowser -eq $true `
    -or $collectorScope -in @("profile-browser", "profile-browser-runtime", "profile_browser") `
    -or $runtimeAdapter -in @("chromium-cdp", "lightpanda", "camoufox", "headed_external") `
    -or $detail.Contains("scope=profile-browser-runtime") `
    -or $detail.Contains("target-profile-browser=true")
}

function Read-ValidationEvidenceReports() {
  $candidateDirs = @(
    "data\validation-reports",
    "data\reports\validation",
    "data\reports\profile-browser-environment",
    "data\reports\validation-smoke",
    "data\reports\headed-external-smoke",
    "data\reports\camoufox-binary-task"
  )

  $items = @()
  foreach ($relativeDir in $candidateDirs) {
    $dir = Join-Path $projectRoot $relativeDir
    if (-not (Test-Path $dir)) { continue }

    Get-ChildItem -Path $dir -Filter *.json -File | ForEach-Object {
      try {
        $value = Get-Content $_.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
        if (Test-ReportProjectRootMatches $value) {
          $signals = Get-ReportSignals $value
          $desktopSignals = @($signals | Where-Object { Test-IsDesktopSignal $_ })
          $profileSignals = @($signals | Where-Object { Test-IsProfileSignal $_ })
          $items += [pscustomobject]@{
            path = $_.FullName
            generatedAt = $value.generatedAt
            sortKey = ConvertTo-ReportSortKey $value.generatedAt $_.LastWriteTimeUtc
            report = $value
            signals = $signals
            desktopSignals = $desktopSignals
            profileSignals = $profileSignals
          }
        }
      } catch {}
    }
  }

  return @($items | Sort-Object -Property sortKey -Descending)
}

$reports = Read-ValidationEvidenceReports
$latest = if ($reports.Count -gt 0) { $reports[0] } else { $null }
$latestDesktop = @($reports | Where-Object { $_.desktopSignals.Count -gt 0 } | Select-Object -First 1)
$latestProfile = @($reports | Where-Object { $_.profileSignals.Count -gt 0 } | Select-Object -First 1)
$desktopSignals = if ($latestDesktop.Count -gt 0) { @($latestDesktop[0].desktopSignals) } else { @() }
$profileSignals = if ($latestProfile.Count -gt 0) { @($latestProfile[0].profileSignals) } else { @() }
$categories = @("webrtc", "canvas", "audio", "leak", "fingerprint")
$categoryComparison = foreach ($category in $categories) {
  $desktopCount = @($desktopSignals | Where-Object { (Get-SignalField $_ "category") -eq $category }).Count
  $profileCount = @($profileSignals | Where-Object { (Get-SignalField $_ "category") -eq $category }).Count
  [ordered]@{
    category = $category
    desktopWebViewObserved = $desktopCount
    profileBrowserObserved = $profileCount
    status = if ($desktopCount -gt 0 -and $profileCount -gt 0) { "comparable" } elseif ($desktopCount -gt 0) { "desktop_only" } elseif ($profileCount -gt 0) { "profile_only" } else { "missing" }
  }
}
$comparableCount = @($categoryComparison | Where-Object { $_.status -eq "comparable" }).Count
$desktopProfileDeltaMinutes = $null
$sameRunStatus = if ($latestDesktop.Count -gt 0 -and $latestProfile.Count -gt 0) {
  $desktopProfileDeltaMinutes = [Math]::Round([Math]::Abs(($latestDesktop[0].sortKey - $latestProfile[0].sortKey).TotalMinutes), 2)
  if ($desktopProfileDeltaMinutes -le $MaxPairAgeMinutes) { "within_window" } else { "stale_pair" }
} elseif ($latestDesktop.Count -eq 0 -and $latestProfile.Count -eq 0) {
  "missing_both"
} elseif ($latestDesktop.Count -eq 0) {
  "missing_desktop_webview"
} else {
  "missing_profile_browser"
}
$status = if ($comparableCount -eq $categories.Count -and $sameRunStatus -eq "within_window") {
  "passed"
} elseif ($latest -eq $null) {
  "blocked_missing_validation_report"
} elseif ($desktopSignals.Count -eq 0) {
  "blocked_missing_desktop_webview_report"
} elseif ($profileSignals.Count -eq 0) {
  "blocked_missing_profile_browser_report"
} elseif ($comparableCount -eq $categories.Count -and $sameRunStatus -eq "stale_pair") {
  "blocked_stale_comparison_pair"
} else {
  "partial_comparison_only"
}
$failureReason = if ($status -eq "passed") {
  ""
} elseif ($latest -eq $null) {
  "no validation report found"
} elseif ($desktopSignals.Count -eq 0) {
  "no desktop WebView validation report found in data/validation-reports, data/reports/validation, data/reports/profile-browser-environment, data/reports/validation-smoke, data/reports/headed-external-smoke, or data/reports/camoufox-binary-task"
} elseif ($profileSignals.Count -eq 0) {
  "no profile browser validation report found in data/validation-reports, data/reports/validation, data/reports/profile-browser-environment, data/reports/validation-smoke, data/reports/headed-external-smoke, or data/reports/camoufox-binary-task"
} elseif ($sameRunStatus -eq "stale_pair") {
  "desktop WebView and profile browser evidence are both present but not within the same-run window: delta=$desktopProfileDeltaMinutes minutes, max=$MaxPairAgeMinutes minutes"
} else {
  "desktop WebView and profile browser evidence are not both present for every category"
}

$report = [ordered]@{
  schemaVersion = "profile_browser_comparison_gate_v3"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  latestValidationReportPath = if ($latest) { $latest.path } else { $null }
  latestDesktopValidationReportPath = if ($latestDesktop.Count -gt 0) { $latestDesktop[0].path } else { $null }
  latestProfileValidationReportPath = if ($latestProfile.Count -gt 0) { $latestProfile[0].path } else { $null }
  maxPairAgeMinutes = $MaxPairAgeMinutes
  desktopProfileDeltaMinutes = $desktopProfileDeltaMinutes
  sameRunStatus = $sameRunStatus
  scannedReportCount = $reports.Count
  desktopSignalCount = $desktopSignals.Count
  profileBrowserSignalCount = $profileSignals.Count
  comparableCategoryCount = $comparableCount
  categoryComparison = $categoryComparison
  failureReason = $failureReason
  notes = @(
    "This gate compares existing reports only; it does not launch a desktop WebView or profile browser.",
    "It independently selects the latest desktop WebView report and latest profile browser report from standard evidence directories.",
    "Passed status requires both sides to include comparable categories and to be within the same-run time window.",
    "Headed external and Camoufox task reports count only when they contain real profile-browser validation signals.",
    "Desktop WebView evidence must not be treated as profile-browser proof."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("profile-browser-comparison-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Profile browser comparison report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }
if ($status -like "blocked*") { exit 2 }
