param(
  [string]$OutputDir = "data/reports/governance"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$files = @(
  "docs\02-current-state.md",
  "docs\03-roadmap.md",
  "docs\04-improvement-backlog.md",
  "docs\25-overall-remaining-work-register.md",
  "docs\final-goal-progress-breakdown.md",
  "RUN_STATE.json"
)

$checks = @()
foreach ($relative in $files) {
  $path = Join-Path $projectRoot $relative
  if (-not (Test-Path $path)) {
    $checks += [ordered]@{ id = "file_present:$relative"; status = "failed"; detail = "missing" }
    continue
  }
  $text = Get-Content $path -Raw -Encoding UTF8
  $checks += [ordered]@{ id = "no_live_77_23:$relative"; status = if ($text.Contains("77% / 23%") -and -not $text.Contains("historical-only")) { "failed" } else { "passed" }; detail = "77/23 must remain historical only" }
  $checks += [ordered]@{ id = "no_live_82_18:$relative"; status = if ($text.Contains("82% / 18%") -and -not $text.Contains("historical-only")) { "failed" } else { "passed" }; detail = "82/18 must remain historical only" }
}

$currentState = Get-Content (Join-Path $projectRoot "docs\02-current-state.md") -Raw -Encoding UTF8
$providerExceptionPresent = ($currentState -match "CAPTCHA") `
  -and ($currentState -match "SMS") `
  -and ($currentState -match "Email") `
  -and (($currentState -match "真实账号凭证") -or ($currentState -match "blocked_missing_credentials"))
$checks += [ordered]@{ id = "local_self_use_green_present"; status = if ($currentState.Contains("Local self-use") -and $currentState.Contains("100% / 0% / green")) { "passed" } else { "failed" }; detail = "canonical local-only live truth must remain 100/0/green" }
$checks += [ordered]@{ id = "mainline_green_present"; status = if ($currentState.Contains("Mainline delivery") -and $currentState.Contains("100% / 0% / green")) { "passed" } else { "failed" }; detail = "mainline closeout truth must remain separate" }
$checks += [ordered]@{ id = "taxonomy_seed_not_runtime"; status = if ($currentState.Contains("taxonomy seed") -and $currentState.Contains("replay runtime") -and $currentState.Contains("passed_full_observed_fingerprint_coverage")) { "passed" } else { "failed" }; detail = "taxonomy seed must stay separate from observed/replay runtime proof" }
$checks += [ordered]@{ id = "local_only_provider_exception"; status = if ($providerExceptionPresent) { "passed" } else { "failed" }; detail = "only CAPTCHA/SMS/Email credential-backed provider smoke may remain pending" }

$failed = @($checks | Where-Object { $_.status -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed" } else { "failed" }
$report = [ordered]@{
  schemaVersion = "live_truth_guard_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  checks = $checks
  failureReason = if ($failed.Count -eq 0) { "" } else { "live-truth guard failed: $(@($failed | ForEach-Object { $_.id }) -join ', ')" }
  notes = @(
    "This guard prevents historical progress ratios and taxonomy target claims from becoming live truth.",
    "Local-only scope cancels AdsPower refresh, external distribution, release performance budget, and second-machine portability as current goals."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("live-truth-guard-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Live truth guard report: $reportPath"
Write-Host "Status: $status"
if ($report.failureReason) { Write-Host "Failure reason: $($report.failureReason)" }
if ($status -ne "passed") { exit 1 }
