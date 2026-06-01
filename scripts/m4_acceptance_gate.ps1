param(
  [string]$OutputDir = "data/reports/m4-acceptance",
  [switch]$SkipLiveTruthRefresh,
  [switch]$SkipProviderPreflightRefresh
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

function Get-Field([object]$Value, [string]$Name) {
  if ($null -eq $Value) { return $null }
  if ($Value -is [System.Collections.IDictionary]) {
    if ($Value.Contains($Name)) { return $Value[$Name] }
    return $null
  }
  $property = $Value.PSObject.Properties[$Name]
  if ($null -eq $property) { return $null }
  return $property.Value
}

function ConvertTo-ReportSortKey([object]$Value, [datetime]$Fallback) {
  if ($null -ne $Value) {
    $text = [string]$Value
    $epochMs = 0L
    if ([Int64]::TryParse($text, [ref]$epochMs)) {
      return [DateTimeOffset]::FromUnixTimeMilliseconds($epochMs).UtcDateTime
    }

    try {
      return ([DateTimeOffset]::Parse($text)).UtcDateTime
    } catch {}
  }

  return $Fallback.ToUniversalTime()
}

function Test-ReportProjectRootMatches([object]$Report) {
  $reportProjectRoot = [string](Get-Field $Report "projectRoot")
  if ([string]::IsNullOrWhiteSpace($reportProjectRoot)) { return $true }
  $left = $reportProjectRoot.TrimEnd('\', '/')
  $right = $projectRoot.TrimEnd('\', '/')
  return $left.Equals($right, [System.StringComparison]::OrdinalIgnoreCase)
}

function Get-LatestReport([string]$DirName, [string]$Pattern) {
  $dir = Join-Path $projectRoot (Join-Path "data\reports" $DirName)
  if (-not (Test-Path $dir)) { return $null }

  $items = @()
  Get-ChildItem -Path $dir -Filter $Pattern -File -ErrorAction SilentlyContinue | ForEach-Object {
    try {
      $value = Get-Content -LiteralPath $_.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
      if (Test-ReportProjectRootMatches $value) {
        $items += [pscustomobject]@{
          path = $_.FullName
          generatedAt = Get-Field $value "generatedAt"
          sortKey = ConvertTo-ReportSortKey (Get-Field $value "generatedAt") $_.LastWriteTimeUtc
          value = $value
        }
      }
    } catch {
      $items += [pscustomobject]@{
        path = $_.FullName
        generatedAt = $null
        sortKey = $_.LastWriteTimeUtc
        value = $null
        parseError = $_.Exception.Message
      }
    }
  }

  $sorted = @($items | Sort-Object -Property sortKey -Descending)
  if ($sorted.Count -eq 0) { return $null }
  return $sorted[0]
}

function New-GateResult(
  [string]$Id,
  [string]$Status,
  [object]$ReportItem,
  [string]$Classification,
  [string]$Reason,
  [string[]]$Failures,
  [string[]]$ExpectedBlockers
) {
  $report = if ($null -ne $ReportItem) { Get-Field $ReportItem "value" } else { $null }
  return [ordered]@{
    id = $Id
    status = $Status
    classification = $Classification
    reportPath = if ($null -ne $ReportItem) { Get-Field $ReportItem "path" } else { $null }
    generatedAt = if ($null -ne $ReportItem) { Get-Field $ReportItem "generatedAt" } else { $null }
    schemaVersion = if ($null -ne $report) { Get-Field $report "schemaVersion" } else { $null }
    reason = $Reason
    failures = @($Failures)
    expectedBlockers = @($ExpectedBlockers)
  }
}

function Get-NonPassedRequiredGateIds([object]$GateResults) {
  if ($null -eq $GateResults) { return @() }
  return @($GateResults | Where-Object {
    (Get-Field $_ "requiredForCrossMachine") -eq $true -and [string](Get-Field $_ "status") -ne "passed"
  } | ForEach-Object { [string](Get-Field $_ "id") })
}

function Get-ProviderBlockedItemIds([object]$Items) {
  if ($null -eq $Items) { return @() }
  return @($Items | Where-Object {
    [string](Get-Field $_ "acceptanceStatus") -ne "accepted" -or @((Get-Field $_ "blockers")).Count -gt 0
  } | ForEach-Object {
    $domain = [string](Get-Field $_ "domain")
    if ([string]::IsNullOrWhiteSpace($domain)) { "provider_item" } else { $domain }
  })
}

function Get-ProviderDryRunContractFailures([object]$Report) {
  $failures = @()
  if ($null -eq $Report) {
    return @("provider dry-run report is missing")
  }

  $items = @((Get-Field $Report "items"))
  if ($items.Count -eq 0) {
    $failures += "provider report has no items"
  }

  $topDryRunStatus = [string](Get-Field $Report "dryRunStatus")
  $topTaxonomyStatus = [string](Get-Field $Report "failureTaxonomyStatus")
  if ([string]::IsNullOrWhiteSpace($topDryRunStatus)) {
    $failures += "provider report missing top-level dryRunStatus"
  }
  if ([string]::IsNullOrWhiteSpace($topTaxonomyStatus)) {
    $failures += "provider report missing top-level failureTaxonomyStatus"
  }

  foreach ($item in $items) {
    $domain = [string](Get-Field $item "domain")
    if ([string]::IsNullOrWhiteSpace($domain)) { $domain = "provider_item" }

    $dryRunStatus = [string](Get-Field $item "dryRunStatus")
    $dryRunAvailable = (Get-Field $item "dryRunAvailable") -eq $true
    $dryRunContract = @((Get-Field $item "dryRunContract") | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
    $dryRunNextAction = [string](Get-Field $item "dryRunNextAction")
    $failureTaxonomyStatus = [string](Get-Field $item "failureTaxonomyStatus")
    $failureTaxonomy = @((Get-Field $item "failureTaxonomy") | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })

    if ([string]::IsNullOrWhiteSpace($dryRunStatus)) {
      $failures += "$domain missing dryRunStatus"
    }
    if (-not $dryRunAvailable) {
      $failures += "$domain dryRunAvailable is not true"
    }
    if ($dryRunContract.Count -eq 0) {
      $failures += "$domain missing dryRunContract"
    }
    if ([string]::IsNullOrWhiteSpace($dryRunNextAction)) {
      $failures += "$domain missing dryRunNextAction"
    }
    if ([string]::IsNullOrWhiteSpace($failureTaxonomyStatus)) {
      $failures += "$domain missing failureTaxonomyStatus"
    }
    if ($failureTaxonomy.Count -eq 0) {
      $failures += "$domain missing failureTaxonomy"
    }
  }

  return $failures
}

function Invoke-LiveTruthGuard {
  if ($SkipLiveTruthRefresh) { return }

  $script = Join-Path $PSScriptRoot "live_truth_guard.ps1"
  if (-not (Test-Path $script)) { return }

  & powershell -NoProfile -ExecutionPolicy Bypass -File $script | Out-Host
  $script:liveTruthExitCode = $LASTEXITCODE
}

function Invoke-ProviderAcceptancePreflight {
  if ($SkipProviderPreflightRefresh) { return }

  $script = Join-Path $PSScriptRoot "provider_acceptance_preflight.ps1"
  if (-not (Test-Path $script)) { return }

  & powershell -NoProfile -ExecutionPolicy Bypass -File $script | Out-Host
  $script:providerPreflightExitCode = $LASTEXITCODE
}

function New-LocalGateResult(
  [string]$Id,
  [string]$Status,
  [string]$Classification,
  [string]$Reason,
  [string[]]$Failures
) {
  return [ordered]@{
    id = $Id
    status = $Status
    classification = $Classification
    reportPath = $null
    generatedAt = (Get-Date).ToString("o")
    schemaVersion = "local_source_contract_v1"
    reason = $Reason
    failures = @($Failures)
    expectedBlockers = @()
  }
}

function Test-AutomationPrimitiveContract {
  $runnerPath = Join-Path $projectRoot "backend\internal\scheduler\cdp_runner.go"
  $testPath = Join-Path $projectRoot "backend\internal\scheduler\cdp_runner_test.go"
  $failures = @()

  if (-not (Test-Path $runnerPath)) {
    $failures += "missing cdp_runner.go"
  }
  if (-not (Test-Path $testPath)) {
    $failures += "missing cdp_runner_test.go"
  }

  $runnerText = if (Test-Path $runnerPath) { Get-Content -LiteralPath $runnerPath -Raw -Encoding UTF8 } else { "" }
  $testText = if (Test-Path $testPath) { Get-Content -LiteralPath $testPath -Raw -Encoding UTF8 } else { "" }
  $requiredActions = @("select", "dialog", "download", "upload", "iframe", "tab")
  foreach ($action in $requiredActions) {
    if ($runnerText -notmatch ("case `"{0}`"" -f [regex]::Escape($action))) {
      $failures += "missing typed runner action: $action"
    }
    if ($testText -notmatch [regex]::Escape($action)) {
      $failures += "missing typed runner test coverage marker: $action"
    }
  }
  if ($testText -notmatch "TestCDPTaskRunner_TypedM4PrimitiveActions") {
    $failures += "missing typed primitive test function"
  }

  if ($failures.Count -gt 0) {
    return New-LocalGateResult "automation_primitives_contract" "missing_coverage" "failed" "M4.3 typed primitive source/test contract is incomplete" $failures
  }
  return New-LocalGateResult "automation_primitives_contract" "passed" "passed" "M4.3 typed primitive source/test contract is present; run go test for behavioral proof" @()
}

function Test-ProviderDryRunContract {
  $item = Get-LatestReport "provider-acceptance" "provider-acceptance-preflight-*.json"
  $report = if ($null -ne $item) { Get-Field $item "value" } else { $null }
  $failures = Get-ProviderDryRunContractFailures $report
  if ($providerPreflightExitCode -ne $null -and $providerPreflightExitCode -notin @(0, 2)) {
    $failures += "provider_acceptance_preflight script exited $providerPreflightExitCode"
  }
  if ($failures.Count -gt 0) {
    return New-LocalGateResult "provider_dry_run_contract" "missing_coverage" "failed" "M4.4 provider dry-run/failure taxonomy contract is incomplete" $failures
  }
  return New-LocalGateResult "provider_dry_run_contract" "passed" "passed" "M4.4 provider dry-run/failure taxonomy report contract is present; real smoke remains externally blocked" @()
}

$liveTruthExitCode = $null
$providerPreflightExitCode = $null
Invoke-LiveTruthGuard
Invoke-ProviderAcceptancePreflight

$gateSpecs = @(
  [ordered]@{ id = "live_truth_guard"; dir = "governance"; pattern = "live-truth-guard-*.json" },
  [ordered]@{ id = "runtime_adapter_evidence_gate"; dir = "runtime-adapter"; pattern = "runtime-adapter-evidence-gate-*.json" },
  [ordered]@{ id = "provider_acceptance_preflight"; dir = "provider-acceptance"; pattern = "provider-acceptance-preflight-*.json" },
  [ordered]@{ id = "session_bundle_portability_smoke"; dir = "session-portability"; pattern = "session-bundle-portability-smoke-*.json" },
  [ordered]@{ id = "profile_browser_comparison_gate"; dir = "profile-browser-comparison"; pattern = "profile-browser-comparison-*.json" }
)

$gates = @()
foreach ($spec in $gateSpecs) {
  $item = Get-LatestReport $spec.dir $spec.pattern
  if ($null -eq $item) {
    $gates += New-GateResult $spec.id "missing" $null "failed" "critical report is missing" @("missing critical report: $($spec.id)") @()
    continue
  }

  $report = Get-Field $item "value"
  if ($null -eq $report) {
    $gates += New-GateResult $spec.id "unreadable" $item "failed" "latest critical report cannot be parsed" @("unreadable critical report: $($spec.id)") @()
    continue
  }

  $status = [string](Get-Field $report "status")
  $classification = "failed"
  $reason = ""
  $failures = @()
  $expectedBlockers = @()

  switch ($spec.id) {
    "live_truth_guard" {
      $failedChecks = @((Get-Field $report "checks") | Where-Object { [string](Get-Field $_ "status") -ne "passed" } | ForEach-Object { [string](Get-Field $_ "id") })
      if ($liveTruthExitCode -ne $null -and $liveTruthExitCode -ne 0) {
        $failures += "live_truth_guard script exited $liveTruthExitCode"
      }
      if ($status -ne "passed") {
        $failures += "live truth drift status: $status"
      }
      if ($failedChecks.Count -gt 0) {
        $failures += "live truth failed checks: $($failedChecks -join ', ')"
      }
      if ($failures.Count -eq 0) {
        $classification = "passed"
        $reason = "live truth guard passed"
      } else {
        $classification = "failed"
        $reason = "live truth drift detected"
      }
    }
    "runtime_adapter_evidence_gate" {
      $b1b5 = Get-Field $report "b1b5Evidence"
      $b1b5Blocked = @()
      $b1b5Properties = @()
      if ($null -ne $b1b5) {
        $b1b5Properties = @($b1b5.PSObject.Properties)
        foreach ($property in $b1b5Properties) {
          $value = [string]$property.Value
          if ($value -like "blocked*" -or $value -like "deferred*") {
            $b1b5Blocked += "$($property.Name)=$value"
          }
        }
      }
      $adspower = [string](Get-Field $report "adspowerRefreshStatus")
      if ($status -eq "passed") {
        if ($b1b5Properties.Count -eq 0 -or $b1b5Blocked.Count -gt 0 -or $adspower -eq "deferred_by_evidence_gate") {
          $classification = "failed"
          $reason = "runtime adapter report is marked passed while blockers remain"
          $failures += "blocked or missing evidence misreported as passed: $($b1b5Blocked -join ', ') adspowerRefreshStatus=$adspower b1b5PropertyCount=$($b1b5Properties.Count)"
        } else {
          $classification = "passed"
          $reason = "runtime adapter evidence gate passed"
        }
      } elseif ($status -eq "blocked_evidence_required") {
        $classification = "expected_blocked"
        $reason = "runtime adapter B1-B5 evidence remains externally blocked"
        $expectedBlockers += @($b1b5Blocked)
        if ($adspower -eq "deferred_by_evidence_gate") { $expectedBlockers += "adspowerRefreshStatus=$adspower" }
      } else {
        $classification = "failed"
        $reason = "unexpected runtime adapter status"
        $failures += "unexpected status: $status"
      }
    }
    "provider_acceptance_preflight" {
      $providerItems = @((Get-Field $report "items"))
      $blockedItems = Get-ProviderBlockedItemIds $providerItems
      $acceptedCount = [int](Get-Field $report "acceptedCount")
      $dryRunFailures = Get-ProviderDryRunContractFailures $report
      if ($providerPreflightExitCode -ne $null -and $providerPreflightExitCode -notin @(0, 2)) {
        $dryRunFailures += "provider_acceptance_preflight script exited $providerPreflightExitCode"
      }
      if ($status -eq "accepted") {
        if ($providerItems.Count -eq 0 -or $acceptedCount -ne $providerItems.Count -or $blockedItems.Count -gt 0 -or $dryRunFailures.Count -gt 0) {
          $classification = "failed"
          $reason = "provider report is marked accepted while item blockers remain"
          $failures += "blocked or missing provider acceptance misreported as accepted: $($blockedItems -join ', ') acceptedCount=$acceptedCount itemCount=$($providerItems.Count)"
          $failures += $dryRunFailures
        } else {
          $classification = "passed"
          $reason = "provider acceptance preflight accepted"
        }
      } elseif ($status -in @("blocked_missing_credentials", "credential_ready_but_runtime_closure_required")) {
        if ($dryRunFailures.Count -gt 0) {
          $classification = "failed"
          $reason = "provider dry-run/failure taxonomy fields are missing"
          $failures += $dryRunFailures
        } else {
          $classification = "expected_blocked"
          $reason = "provider acceptance requires credentials and real smoke evidence"
          $expectedBlockers += if ($blockedItems.Count -gt 0) { $blockedItems } else { $status }
        }
      } else {
        $classification = "failed"
        $reason = "unexpected provider acceptance status"
        $failures += "unexpected status: $status"
      }
    }
    "session_bundle_portability_smoke" {
      $crossMachineComplete = (Get-Field $report "crossMachineComplete") -eq $true
      $sessionGateResults = @((Get-Field $report "gateResults"))
      $missingCrossMachineGateIds = Get-NonPassedRequiredGateIds $sessionGateResults
      if ($status -eq "cross_machine_passed") {
        if (-not $crossMachineComplete -or $sessionGateResults.Count -eq 0 -or $missingCrossMachineGateIds.Count -gt 0) {
          $classification = "failed"
          $reason = "session portability report is marked passed while cross-machine gates remain blocked"
          $failures += "blocked or missing cross-machine portability misreported as passed: $($missingCrossMachineGateIds -join ', ') gateResultCount=$($sessionGateResults.Count)"
        } else {
          $classification = "passed"
          $reason = "cross-machine SessionBundle portability passed"
        }
      } elseif ($status -in @("local_contract_passed", "blocked_requires_second_machine_evidence")) {
        $classification = "expected_blocked"
        $reason = "SessionBundle local contract exists; second-machine evidence is still required"
        $expectedBlockers += if ($missingCrossMachineGateIds.Count -gt 0) { $missingCrossMachineGateIds } else { "second_machine_evidence_required" }
      } else {
        $classification = "failed"
        $reason = "unexpected SessionBundle portability status"
        $failures += "unexpected status: $status"
      }
    }
    "profile_browser_comparison_gate" {
      $desktopSignalCount = [int](Get-Field $report "desktopSignalCount")
      $profileSignalCount = [int](Get-Field $report "profileBrowserSignalCount")
      $comparableCategoryCount = [int](Get-Field $report "comparableCategoryCount")
      $categoryComparisonCount = @((Get-Field $report "categoryComparison")).Count
      if ($status -eq "passed") {
        if ($desktopSignalCount -le 0 -or $profileSignalCount -le 0 -or $categoryComparisonCount -eq 0 -or $comparableCategoryCount -ne $categoryComparisonCount) {
          $classification = "failed"
          $reason = "profile/browser comparison is marked passed without comparable evidence"
          $failures += "blocked or incomplete comparison misreported as passed: desktop=$desktopSignalCount profile=$profileSignalCount comparable=$comparableCategoryCount categoryComparisonCount=$categoryComparisonCount"
        } else {
          $classification = "passed"
          $reason = "profile/browser comparison passed"
        }
      } elseif ($status -in @("blocked_missing_validation_report", "blocked_missing_desktop_webview_report", "blocked_missing_profile_browser_report", "partial_comparison_only")) {
        $classification = "expected_blocked"
        $reason = "profile/browser comparison evidence is incomplete"
        $expectedBlockers += $status
      } else {
        $classification = "failed"
        $reason = "unexpected profile/browser comparison status"
        $failures += "unexpected status: $status"
      }
    }
  }

  $gates += New-GateResult $spec.id $status $item $classification $reason $failures $expectedBlockers
}

$gates += Test-AutomationPrimitiveContract
$gates += Test-ProviderDryRunContract

$failedGates = @($gates | Where-Object { $_.classification -eq "failed" })
$expectedBlockedGates = @($gates | Where-Object { $_.classification -eq "expected_blocked" })
$passedGates = @($gates | Where-Object { $_.classification -eq "passed" })

$status = if ($failedGates.Count -gt 0) {
  "failed"
} elseif ($expectedBlockedGates.Count -gt 0) {
  "expected_blocked"
} else {
  "passed"
}

$report = [ordered]@{
  schemaVersion = "m4_acceptance_gate_v3"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  summary = [ordered]@{
    passed = $passedGates.Count
    expectedBlocked = $expectedBlockedGates.Count
    failed = $failedGates.Count
    total = $gates.Count
  }
  gates = $gates
  failureReason = if ($failedGates.Count -eq 0) { "" } else { @($failedGates | ForEach-Object { "$($_.id): $($_.reason)" }) -join "; " }
  expectedBlockedReason = if ($expectedBlockedGates.Count -eq 0) { "" } else { @($expectedBlockedGates | ForEach-Object { "$($_.id): $($_.reason)" }) -join "; " }
  notes = @(
    "External blockers are allowed only as expected_blocked.",
    "Live truth drift, missing critical reports, unreadable reports, and passed reports with remaining blockers fail this gate.",
    "This gate refreshes live_truth_guard unless -SkipLiveTruthRefresh is set and refreshes provider acceptance preflight unless -SkipProviderPreflightRefresh is set; external gates are aggregated from latest reports.",
    "Local automation primitive contract checks source/test coverage markers only; behavioral proof still comes from go test.",
    "Provider dry-run contract is local report schema evidence only; provider acceptance remains expected_blocked until credential-backed real smoke passes."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-acceptance-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding UTF8

Write-Host "M4 acceptance gate report: $reportPath"
Write-Host "Status: $status"
Write-Host "Summary: passed=$($passedGates.Count), expected_blocked=$($expectedBlockedGates.Count), failed=$($failedGates.Count)"
if ($report.failureReason) { Write-Host "Failure reason: $($report.failureReason)" }
if ($report.expectedBlockedReason) { Write-Host "Expected blocked: $($report.expectedBlockedReason)" }

if ($status -eq "failed") { exit 1 }
exit 0
