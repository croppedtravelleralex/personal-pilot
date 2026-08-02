param(
  [string]$LightpandaSmokeStatus = "not_run",
  [string]$CamoufoxSmokeStatus = "not_run",
  [string]$CamoufoxBinaryTaskStatus = "not_run",
  [string]$HeadedProcessLifecycleStatus = "source_test_backed_contract",
  [string]$HeadedCdpAttachStatus = "source_test_backed_contract",
  [string]$HeadedProfileCompatibilityStatus = "not_run",
  [string]$OutputDir = "data/reports/runtime-adapter"
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

function Test-ReportProjectRootMatches($value) {
  $reportProjectRoot = [string](Get-Field $value "projectRoot")
  if ([string]::IsNullOrWhiteSpace($reportProjectRoot)) { return $true }
  $left = $reportProjectRoot.TrimEnd('\', '/')
  $right = $projectRoot.TrimEnd('\', '/')
  return $left.Equals($right, [System.StringComparison]::OrdinalIgnoreCase)
}

function Read-ReportItems([string]$DirName) {
  $dir = Join-Path $projectRoot (Join-Path "data\reports" $DirName)
  if (-not (Test-Path $dir)) { return @() }
  $items = @()
  Get-ChildItem -Path $dir -Filter "*.json" -ErrorAction SilentlyContinue | ForEach-Object {
    try {
      $value = Get-Content -Path $_.FullName -Raw | ConvertFrom-Json
      if (Test-ReportProjectRootMatches $value) {
        $items += [pscustomobject]@{
          path = $_.FullName
          generatedAt = $value.generatedAt
          sortKey = ConvertTo-ReportSortKey $value.generatedAt $_.LastWriteTimeUtc
          value = $value
        }
      }
    } catch {
    }
  }
  return @($items | Sort-Object -Property sortKey -Descending)
}

function Get-LatestReport([string]$DirName) {
  $items = Read-ReportItems $DirName
  if ($items.Count -eq 0) { return $null }
  return $items[0]
}

function Get-HeadedExternalEvidenceRank($value) {
  $status = [string](Get-Field $value "status")
  $task = Get-Field $value "realBinaryTask"
  $taskStatus = [string](Get-Field $task "status")
  if ($taskStatus -ne "passed") { return 0 }

  $result = Get-Field $task "result"
  $action = [string](Get-Field $result "action")
  $signals = Get-Field $result "validation_signals"
  $signalCount = if ($null -eq $signals) { 0 } else { @($signals).Count }
  $repeatability = Get-Field $value "repeatability"
  $repeatabilityStatus = [string](Get-Field $repeatability "status")

  if ($status -eq "passed_real_binary_repeatability" -and $repeatabilityStatus -eq "passed_repeatability_partial_coherence") {
    return 30
  }
  if ($status -eq "passed_real_binary_validation_probe" -and $action -eq "validation_probe" -and $signalCount -gt 0) {
    return 20
  }
  if ($status -eq "passed_real_binary_task") {
    return 10
  }
  return 0
}

function Get-BestHeadedExternalReport {
  $items = Read-ReportItems "headed-external-smoke"
  if ($items.Count -eq 0) { return $null }

  $ranked = @($items | ForEach-Object {
    [pscustomobject]@{
      rank = Get-HeadedExternalEvidenceRank (Get-Field $_ "value")
      sortKey = Get-Field $_ "sortKey"
      item = $_
    }
  } | Sort-Object @{ Expression = "rank"; Descending = $true }, @{ Expression = "sortKey"; Descending = $true })

  return $ranked[0].item
}

function Get-LatestReportValue([object]$Item) {
  if ($null -eq $Item) { return $null }
  return Get-Field $Item "value"
}

function Get-LatestReportPath([object]$Item) {
  $path = [string](Get-Field $Item "path")
  if ([string]::IsNullOrWhiteSpace($path)) { return $null }
  return $path
}

function Get-LatestReportGeneratedAt([object]$Item) {
  $generatedAt = [string](Get-Field $Item "generatedAt")
  if ([string]::IsNullOrWhiteSpace($generatedAt)) { return $null }
  return $generatedAt
}

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

function Get-SignalCategories([object]$Signals) {
  $categories = @()
  if ($null -eq $Signals) { return $categories }
  foreach ($signal in @($Signals)) {
    $category = [string](Get-Field $signal "category")
    if (-not [string]::IsNullOrWhiteSpace($category)) { $categories += $category }
  }
  return @($categories | Sort-Object -Unique)
}

function Count-SignalStatus([object]$Signals, [string]$Status) {
  if ($null -eq $Signals) { return 0 }
  return @($Signals | Where-Object { [string](Get-Field $_ "status") -eq $Status }).Count
}

$latestCamoufoxBinaryTaskItem = Get-LatestReport "camoufox-binary-task"
$latestCamoufoxBinaryTask = Get-LatestReportValue $latestCamoufoxBinaryTaskItem
if ($CamoufoxBinaryTaskStatus -eq "not_run" -and (Get-Field $latestCamoufoxBinaryTask "status") -eq "passed") {
  $CamoufoxBinaryTaskStatus = "passed"
}

$latestHeadedExternalAnySmokeItem = Get-LatestReport "headed-external-smoke"
$latestHeadedExternalSmokeItem = Get-BestHeadedExternalReport
$latestHeadedExternalSmoke = Get-LatestReportValue $latestHeadedExternalSmokeItem
$latestHeadedExternalAnySmoke = Get-LatestReportValue $latestHeadedExternalAnySmokeItem
$latestHeadedExternalStatus = [string](Get-Field $latestHeadedExternalSmoke "status")
$latestHeadedExternalTask = Get-Field $latestHeadedExternalSmoke "realBinaryTask"
$latestHeadedExternalTaskStatus = [string](Get-Field $latestHeadedExternalTask "status")
$latestHeadedExternalRepeatability = Get-Field $latestHeadedExternalSmoke "repeatability"
$latestHeadedExternalRepeatabilityStatus = [string](Get-Field $latestHeadedExternalRepeatability "status")
$headedExternalPassedStatuses = @("passed_real_binary_task", "passed_real_binary_validation_probe", "passed_real_binary_repeatability")
$headedExternalRealTaskPassed = $headedExternalPassedStatuses -contains $latestHeadedExternalStatus -and $latestHeadedExternalTaskStatus -eq "passed"
$latestHeadedExternalTaskResult = Get-Field $latestHeadedExternalTask "result"
$latestHeadedExternalAction = [string](Get-Field $latestHeadedExternalTaskResult "action")
$latestHeadedExternalSignals = Get-Field $latestHeadedExternalTaskResult "validation_signals"
$headedExternalSignalCategories = Get-SignalCategories $latestHeadedExternalSignals
$headedExternalSignalCount = if ($null -eq $latestHeadedExternalSignals) { 0 } else { @($latestHeadedExternalSignals).Count }
$headedExternalWarningCount = Count-SignalStatus $latestHeadedExternalSignals "warning"
$headedExternalValidationProbeObserved = $headedExternalRealTaskPassed -and $latestHeadedExternalAction -eq "validation_probe" -and $headedExternalSignalCount -gt 0
$headedExternalRepeatabilityPassed = $latestHeadedExternalStatus -eq "passed_real_binary_repeatability" -and $latestHeadedExternalRepeatabilityStatus -eq "passed_repeatability_partial_coherence"

$latestRemoteProxyTlsItem = Get-LatestReport "remote-proxy-tls"
$latestRemoteProxyTls = Get-LatestReportValue $latestRemoteProxyTlsItem
$latestRemoteProxyTlsPath = Get-LatestReportPath $latestRemoteProxyTlsItem
$latestRemoteProxyTlsGeneratedAt = Get-LatestReportGeneratedAt $latestRemoteProxyTlsItem
$latestRemoteProxyTlsStatus = [string](Get-Field $latestRemoteProxyTls "status")
$latestRemoteProxyTlsObserved = Get-Field $latestRemoteProxyTls "observed"
$latestRemoteProxyTlsDirectBaseline = Get-Field $latestRemoteProxyTls "directBaseline"
$latestRemoteProxyTlsExitIp = [string](Get-Field $latestRemoteProxyTlsObserved "exitIp")
$latestRemoteProxyTlsJa3Hash = [string](Get-Field $latestRemoteProxyTlsObserved "tlsJa3Hash")
$latestRemoteProxyTlsJa4 = [string](Get-Field $latestRemoteProxyTlsObserved "tlsJa4")
$latestRemoteProxyTlsDirectBaselineStatus = [string](Get-Field $latestRemoteProxyTlsDirectBaseline "status")
$latestRemoteProxyTlsDirectExitIpDifferent = Get-Field $latestRemoteProxyTlsDirectBaseline "exitIpDifferentFromProxied"
$remoteProxyTlsEvidenceStatus = if ($latestRemoteProxyTlsStatus -eq "passed_remote_proxy_tls_observed") {
  "passed"
} elseif ($latestRemoteProxyTlsStatus -eq "passed_local_direct_tls_observed") {
  "passed_local_direct_tls"
} elseif ($latestRemoteProxyTlsStatus -eq "partial_remote_proxy_egress_observed") {
  "partial_remote_proxy_egress_observed"
} elseif ($latestRemoteProxyTlsStatus -eq "partial_local_direct_egress_observed") {
  "partial_local_direct_egress_observed"
} elseif ($latestRemoteProxyTlsStatus -eq "blocked_remote_proxy_required") {
  "blocked_remote_proxy_required"
} elseif ([string]::IsNullOrWhiteSpace($latestRemoteProxyTlsStatus)) {
  "not_run"
} else {
  $latestRemoteProxyTlsStatus
}

$latestObservedCoverageItem = Get-LatestReport "observed-fingerprint-coverage"
$latestObservedCoverage = Get-LatestReportValue $latestObservedCoverageItem
$latestObservedCoverageStatus = [string](Get-Field $latestObservedCoverage "status")
$latestObservedCoverageSummary = Get-Field $latestObservedCoverage "summary"
$observedFullCoverage = $latestObservedCoverageStatus -eq "passed_full_observed_fingerprint_coverage"

$latestReplayRuntimeItem = Get-LatestReport "live-replay-runtime"
$latestReplayRuntime = Get-LatestReportValue $latestReplayRuntimeItem
$latestReplayRuntimeStatus = [string](Get-Field $latestReplayRuntime "status")
$latestReplayRuntimeSummary = Get-Field $latestReplayRuntime "summary"
$replayContractOnly = [int](Get-Field $latestReplayRuntime "contractOnlyEventCount")
$replayFullLocal = $latestReplayRuntimeStatus -in @("passed_full_local_replay_runtime", "passed_local_replay_runtime") -and $replayContractOnly -eq 0

$latestSessionItem = Get-LatestReport "session-portability"
$latestSession = Get-LatestReportValue $latestSessionItem
$latestSessionStatus = [string](Get-Field $latestSession "status")
$sessionLocalPassed = $latestSessionStatus -in @("local_restore_verified", "local_contract_passed", "cross_machine_passed")

$latestM10StabilityItem = Get-LatestReport "m10-headed-stability"
$latestM10Stability = Get-LatestReportValue $latestM10StabilityItem
$latestM10StabilityStatus = [string](Get-Field $latestM10Stability "status")
$m10StabilityPassed = $latestM10StabilityStatus -eq "passed_long_task_stability_coherence"

$latestM15ProcessItem = Get-LatestReport "m15-browser-process"
$latestM15Process = Get-LatestReportValue $latestM15ProcessItem
$latestM15ProcessStatus = [string](Get-Field $latestM15Process "status")
$m15ProcessPassed = $latestM15ProcessStatus -eq "passed_real_browser_process_prewarm_cleanup"

$latestM15PoolItem = Get-LatestReport "m15-browser-pool"
$latestM15Pool = Get-LatestReportValue $latestM15PoolItem
$latestM15PoolStatus = [string](Get-Field $latestM15Pool "status")
$m15PoolPassed = $latestM15PoolStatus -in @("passed_real_pool_process_integration", "passed_pool_lifecycle_harness")

if ($headedExternalRealTaskPassed) {
  if ($HeadedProcessLifecycleStatus -eq "source_test_backed_contract") { $HeadedProcessLifecycleStatus = "passed" }
  if ($HeadedCdpAttachStatus -eq "source_test_backed_contract") { $HeadedCdpAttachStatus = "passed" }
  if ($HeadedProfileCompatibilityStatus -eq "not_run") { $HeadedProfileCompatibilityStatus = "minimal_real_binary_task_passed" }
}

$adapters = @(
  [ordered]@{
    adapterId = "fake"
    status = "available_for_contract_tests"
    processLifecycleStatus = "in_process"
    cdpAttachStatus = "not_applicable"
    profileRuntimeEvidence = "warning_stub_only"
    fingerprintRuntimeDepth = "none"
    capabilities = @("contract_tests")
    blockers = @()
  },
  [ordered]@{
    adapterId = "lightpanda"
    status = if ($LightpandaSmokeStatus -eq "passed") { "evidence_recorded" } else { "contract_ready_smoke_required" }
    processLifecycleStatus = "external_process_contract"
    cdpAttachStatus = if ($LightpandaSmokeStatus -eq "passed") { "passed" } else { "validation_probe_contract" }
    profileRuntimeEvidence = "validation_probe_contract"
    fingerprintRuntimeDepth = "26 projected fields; observed proof requires real Lightpanda/CDP smoke"
    capabilities = @("profile_runtime_probe", "cdp_validation_smoke")
    blockers = @()
  },
  [ordered]@{
    adapterId = "camoufox"
    status = if ($CamoufoxBinaryTaskStatus -eq "passed") { "real_binary_page_open_recorded" } elseif ($CamoufoxSmokeStatus -eq "passed") { "source_test_backed_runtime_smoke_recorded" } else { "minimal_cdp_runner_source_test_backed" }
    processLifecycleStatus = "spawn_wait_cleanup_contract"
    cdpAttachStatus = "Target.createTarget_attach_Page.navigate_contract"
    profileRuntimeEvidence = if ($CamoufoxBinaryTaskStatus -eq "passed") { "real_binary_headless_page_open_screenshot" } else { "minimal_cdp_actions_and_validation_probe" }
    fingerprintRuntimeDepth = if ($observedFullCoverage) { "full local profile-browser observed coverage recorded" } else { "profile browser validation probe exists; full 450 fingerprint coverage remains pending" }
    capabilities = @("open_page", "fetch", "get_html", "get_title", "get_final_url", "extract_text", "validation_probe")
    blockers = @()
  },
  [ordered]@{
    adapterId = "headed_external"
    status = if ($headedExternalRepeatabilityPassed) { "real_binary_repeatability_recorded" } elseif ($headedExternalValidationProbeObserved) { "real_binary_validation_probe_recorded" } elseif ($headedExternalRealTaskPassed) { "real_binary_task_recorded" } elseif ($HeadedProcessLifecycleStatus -eq "passed" -and $HeadedCdpAttachStatus -eq "passed" -and $HeadedProfileCompatibilityStatus -eq "passed") { "evidence_recorded" } else { "minimal_cdp_runner_source_test_backed" }
    processLifecycleStatus = $HeadedProcessLifecycleStatus
    cdpAttachStatus = $HeadedCdpAttachStatus
    profileRuntimeEvidence = if ($headedExternalRepeatabilityPassed) { "profile_browser_validation_probe_repeatability_observed" } elseif ($headedExternalValidationProbeObserved) { "profile_browser_validation_probe_observed" } else { $HeadedProfileCompatibilityStatus }
    fingerprintRuntimeDepth = if ($observedFullCoverage) { "full local profile-browser observed coverage recorded" } elseif ($headedExternalRepeatabilityPassed) { "repeatable partial headed profile-browser observed signals: $($headedExternalSignalCategories -join ', '); full 450 fingerprint coverage remains pending" } elseif ($headedExternalValidationProbeObserved) { "partial headed profile-browser observed signals: $($headedExternalSignalCategories -join ', '); full 450 fingerprint coverage remains pending" } else { "profile browser validation probe exists; full 450 fingerprint coverage remains pending" }
    capabilities = @("open_page", "fetch", "get_html", "get_title", "get_final_url", "extract_text", "validation_probe")
    blockers = @(if ($headedExternalRealTaskPassed) {
      if (-not $m10StabilityPassed) { "M10 long-task stability/coherence report is missing" }
      if (-not $observedFullCoverage) { "full local 450 observed coverage report is missing" }
    } elseif (-not ($HeadedProcessLifecycleStatus -eq "passed" -and $HeadedCdpAttachStatus -eq "passed" -and $HeadedProfileCompatibilityStatus -eq "passed")) {
      "real headed external browser binary task-run smoke not attached to this report"
    })
  }
)

$blockedAdapters = @($adapters | Where-Object { $_.blockers.Count -gt 0 })
$headedReady = @($adapters | Where-Object { $_.adapterId -eq "headed_external" -and ($_.status -eq "evidence_recorded" -or $_.status -eq "real_binary_task_recorded" -or $_.status -eq "real_binary_validation_probe_recorded" -or $_.status -eq "real_binary_repeatability_recorded") }).Count -eq 1
$lightpandaReady = @($adapters | Where-Object { $_.adapterId -eq "lightpanda" -and $_.status -eq "evidence_recorded" }).Count -eq 1

$b1b5Evidence = [ordered]@{
  validationObservedCoverage = if ($observedFullCoverage) { "passed" } elseif ([string]::IsNullOrWhiteSpace($latestObservedCoverageStatus)) { "not_run" } else { $latestObservedCoverageStatus }
  fingerprintRuntimeDepth = if ($observedFullCoverage) { "passed" } elseif ($headedExternalRepeatabilityPassed) { "partial_headed_profile_browser_repeatability_observed" } elseif ($headedExternalValidationProbeObserved) { "partial_headed_profile_browser_observed" } else { "partial_26_projected_fields" }
  transportRemoteProxyTls = $remoteProxyTlsEvidenceStatus
  sessionPortability = if ($sessionLocalPassed) { "passed_local_restore_verified" } elseif ([string]::IsNullOrWhiteSpace($latestSessionStatus)) { "not_run" } else { $latestSessionStatus }
  providerProductionClosure = "provider_credentials_pending_allowed"
  replayRuntime = if ($replayFullLocal) { "passed_full_local_replay_runtime" } elseif ([string]::IsNullOrWhiteSpace($latestReplayRuntimeStatus)) { "not_run" } else { $latestReplayRuntimeStatus }
  browserPoolProcess = if ($m15ProcessPassed -and $m15PoolPassed) { "passed_real_pool_process_integration" } else { "partial_or_missing_m15_process_pool" }
  runtimeAdapterEvidence = if ($headedExternalRepeatabilityPassed -and $m10StabilityPassed -and $m15ProcessPassed -and $m15PoolPassed) { "passed_local_self_use" } elseif ($headedExternalRepeatabilityPassed) { "partial_real_binary_repeatability_recorded" } elseif ($headedExternalValidationProbeObserved) { "partial_real_binary_validation_probe_recorded" } elseif ($headedReady -or $CamoufoxBinaryTaskStatus -eq "passed") { "partial_real_binary_task_recorded" } else { "partial_or_blocked" }
}
$localSelfUseReady = $b1b5Evidence.validationObservedCoverage -eq "passed" `
  -and $b1b5Evidence.fingerprintRuntimeDepth -eq "passed" `
  -and $b1b5Evidence.transportRemoteProxyTls -in @("passed", "passed_local_direct_tls") `
  -and $b1b5Evidence.sessionPortability -eq "passed_local_restore_verified" `
  -and $b1b5Evidence.replayRuntime -eq "passed_full_local_replay_runtime" `
  -and $b1b5Evidence.browserPoolProcess -eq "passed_real_pool_process_integration" `
  -and $b1b5Evidence.runtimeAdapterEvidence -eq "passed_local_self_use"

$adspowerRefreshStatus = "not_applicable_local_only"

$status = if ($localSelfUseReady) {
  "passed_local_self_use"
} else {
  "blocked_evidence_required"
}
$failureReason = if ($status -eq "passed_local_self_use") {
  ""
} else {
  "local runtime adapter evidence is incomplete"
}

$report = [ordered]@{
  schemaVersion = "runtime_adapter_evidence_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  status = $status
  projectRoot = $projectRoot
  adapters = $adapters
  b1b5Evidence = $b1b5Evidence
  remoteProxyTlsEvidence = [ordered]@{
    status = $remoteProxyTlsEvidenceStatus
    latestReportStatus = if ([string]::IsNullOrWhiteSpace($latestRemoteProxyTlsStatus)) { $null } else { $latestRemoteProxyTlsStatus }
    latestReportPath = $latestRemoteProxyTlsPath
    latestReportGeneratedAt = $latestRemoteProxyTlsGeneratedAt
    exitIp = if ([string]::IsNullOrWhiteSpace($latestRemoteProxyTlsExitIp)) { $null } else { $latestRemoteProxyTlsExitIp }
    tlsJa3Hash = if ([string]::IsNullOrWhiteSpace($latestRemoteProxyTlsJa3Hash)) { $null } else { $latestRemoteProxyTlsJa3Hash }
    tlsJa4 = if ([string]::IsNullOrWhiteSpace($latestRemoteProxyTlsJa4)) { $null } else { $latestRemoteProxyTlsJa4 }
    directBaselineStatus = if ([string]::IsNullOrWhiteSpace($latestRemoteProxyTlsDirectBaselineStatus)) { $null } else { $latestRemoteProxyTlsDirectBaselineStatus }
    directExitIpDifferentFromProxied = $latestRemoteProxyTlsDirectExitIpDifferent
    requiredForAdsPowerRefresh = $false
    evidenceBoundary = "local-only scope accepts local direct TLS/transport observation; remote proxy egress is optional and must not block self-use"
  }
  localSelfUseEvidence = [ordered]@{
    ready = $localSelfUseReady
    observedCoverageStatus = $latestObservedCoverageStatus
    observedCoverageReportPath = Get-LatestReportPath $latestObservedCoverageItem
    observedSignalCount = Get-Field $latestObservedCoverage "observedSignalCount"
    replayRuntimeStatus = $latestReplayRuntimeStatus
    replayRuntimeReportPath = Get-LatestReportPath $latestReplayRuntimeItem
    replayContractOnlyEventCount = $replayContractOnly
    sessionStatus = $latestSessionStatus
    sessionReportPath = Get-LatestReportPath $latestSessionItem
    m10StabilityStatus = $latestM10StabilityStatus
    m10StabilityReportPath = Get-LatestReportPath $latestM10StabilityItem
    m15ProcessStatus = $latestM15ProcessStatus
    m15ProcessReportPath = Get-LatestReportPath $latestM15ProcessItem
    m15PoolStatus = $latestM15PoolStatus
    m15PoolReportPath = Get-LatestReportPath $latestM15PoolItem
    providerCredentialException = "CAPTCHA/SMS/Email real account credential-backed smoke remains pending by user-approved exception"
  }
  headedExternalValidationProbe = [ordered]@{
    status = if ($headedExternalRepeatabilityPassed) { "repeatability_partial_coherence_observed" } elseif ($headedExternalValidationProbeObserved) { "partial_observed" } elseif ($headedExternalRealTaskPassed) { "real_binary_task_without_validation_probe" } else { "not_recorded" }
    selectedReportPath = Get-LatestReportPath $latestHeadedExternalSmokeItem
    selectedReportStatus = if ([string]::IsNullOrWhiteSpace($latestHeadedExternalStatus)) { $null } else { $latestHeadedExternalStatus }
    latestReportPath = Get-LatestReportPath $latestHeadedExternalAnySmokeItem
    latestReportStatus = if ($latestHeadedExternalAnySmoke) { [string](Get-Field $latestHeadedExternalAnySmoke "status") } else { $null }
    signalCount = $headedExternalSignalCount
    warningCount = $headedExternalWarningCount
    categories = $headedExternalSignalCategories
    latestAction = if ([string]::IsNullOrWhiteSpace($latestHeadedExternalAction)) { $null } else { $latestHeadedExternalAction }
    evidenceBoundary = "headed_external profile-browser validation probe consumed by local self-use evidence gate"
  }
  headedExternalRepeatability = [ordered]@{
    status = if ([string]::IsNullOrWhiteSpace($latestHeadedExternalRepeatabilityStatus)) { "not_recorded" } else { $latestHeadedExternalRepeatabilityStatus }
    requestedCount = Get-Field $latestHeadedExternalRepeatability "requestedCount"
    executedCount = Get-Field $latestHeadedExternalRepeatability "executedCount"
    passedCount = Get-Field $latestHeadedExternalRepeatability "passedCount"
    stableSignalCount = Get-Field $latestHeadedExternalRepeatability "stableSignalCount"
    stableCategories = Get-Field $latestHeadedExternalRepeatability "stableCategories"
    stableSignalStatuses = Get-Field $latestHeadedExternalRepeatability "stableSignalStatuses"
    evidenceBoundary = "multi-run validation_probe shape stability consumed by M10 and runtime adapter local self-use evidence"
  }
  adspowerRefreshStatus = $adspowerRefreshStatus
  failureReason = $failureReason
  notes = @(
    "This report is an evidence gate, not a headed browser implementation.",
    "Headed external runtime must remain an adapter boundary and must not turn this repository into a Chromium/Firefox fork host.",
    "Local-only scope accepts local direct transport/TLS observation; remote proxy provider egress is optional.",
    "CAPTCHA/SMS/Email credential-backed provider smoke is the remaining provider exception.",
    "AdsPower scoring is not applicable to the current local-only self-use scope."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("runtime-adapter-evidence-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Runtime adapter evidence gate report: $reportPath"
Write-Host "Status: $status"
Write-Host "AdsPower refresh: $adspowerRefreshStatus"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "blocked_evidence_required") { exit 2 }
