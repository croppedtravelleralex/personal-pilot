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
} elseif ($latestRemoteProxyTlsStatus -eq "partial_remote_proxy_egress_observed") {
  "partial_remote_proxy_egress_observed"
} elseif ($latestRemoteProxyTlsStatus -eq "blocked_remote_proxy_required") {
  "blocked_remote_proxy_required"
} elseif ([string]::IsNullOrWhiteSpace($latestRemoteProxyTlsStatus)) {
  "not_run"
} else {
  $latestRemoteProxyTlsStatus
}

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
    blockers = @("not real browser evidence")
  },
  [ordered]@{
    adapterId = "lightpanda"
    status = if ($LightpandaSmokeStatus -eq "passed") { "evidence_recorded" } else { "contract_ready_smoke_required" }
    processLifecycleStatus = "external_process_contract"
    cdpAttachStatus = if ($LightpandaSmokeStatus -eq "passed") { "passed" } else { "validation_probe_contract" }
    profileRuntimeEvidence = "validation_probe_contract"
    fingerprintRuntimeDepth = "26 projected fields; observed proof requires real Lightpanda/CDP smoke"
    capabilities = @("profile_runtime_probe", "cdp_validation_smoke")
    blockers = @(if ($LightpandaSmokeStatus -ne "passed") { "repeatable Lightpanda/CDP smoke not attached to this report" })
  },
  [ordered]@{
    adapterId = "camoufox"
    status = if ($CamoufoxBinaryTaskStatus -eq "passed") { "real_binary_page_open_recorded" } elseif ($CamoufoxSmokeStatus -eq "passed") { "source_test_backed_runtime_smoke_recorded" } else { "minimal_cdp_runner_source_test_backed" }
    processLifecycleStatus = "spawn_wait_cleanup_contract"
    cdpAttachStatus = "Target.createTarget_attach_Page.navigate_contract"
    profileRuntimeEvidence = if ($CamoufoxBinaryTaskStatus -eq "passed") { "real_binary_headless_page_open_screenshot" } else { "minimal_cdp_actions_and_validation_probe" }
    fingerprintRuntimeDepth = "profile browser validation probe exists; full 450 fingerprint coverage remains pending"
    capabilities = @("open_page", "fetch", "get_html", "get_title", "get_final_url", "extract_text", "validation_probe")
    blockers = @(if ($CamoufoxBinaryTaskStatus -eq "passed") {
      "full headed runtime fingerprint/leak/coherence evidence remains pending"
    } else {
      "real Camoufox binary task-run smoke not attached to this report"
      "full headed runtime fingerprint/leak/coherence evidence remains pending"
    })
  },
  [ordered]@{
    adapterId = "headed_external"
    status = if ($headedExternalRepeatabilityPassed) { "real_binary_repeatability_recorded" } elseif ($headedExternalValidationProbeObserved) { "real_binary_validation_probe_recorded" } elseif ($headedExternalRealTaskPassed) { "real_binary_task_recorded" } elseif ($HeadedProcessLifecycleStatus -eq "passed" -and $HeadedCdpAttachStatus -eq "passed" -and $HeadedProfileCompatibilityStatus -eq "passed") { "evidence_recorded" } else { "minimal_cdp_runner_source_test_backed" }
    processLifecycleStatus = $HeadedProcessLifecycleStatus
    cdpAttachStatus = $HeadedCdpAttachStatus
    profileRuntimeEvidence = if ($headedExternalRepeatabilityPassed) { "profile_browser_validation_probe_repeatability_observed" } elseif ($headedExternalValidationProbeObserved) { "profile_browser_validation_probe_observed" } else { $HeadedProfileCompatibilityStatus }
    fingerprintRuntimeDepth = if ($headedExternalRepeatabilityPassed) { "repeatable partial headed profile-browser observed signals: $($headedExternalSignalCategories -join ', '); full 450 fingerprint coverage remains pending" } elseif ($headedExternalValidationProbeObserved) { "partial headed profile-browser observed signals: $($headedExternalSignalCategories -join ', '); full 450 fingerprint coverage remains pending" } else { "profile browser validation probe exists; full 450 fingerprint coverage remains pending" }
    capabilities = @("open_page", "fetch", "get_html", "get_title", "get_final_url", "extract_text", "validation_probe")
    blockers = @(if ($headedExternalRealTaskPassed) {
      if ($headedExternalRepeatabilityPassed) { "long-task headed runtime, remote proxy/TLS proof, and 450 observed coverage remain pending" } elseif ($headedExternalValidationProbeObserved) { "complete headed runtime repeatability/coherence matrix and 450 observed coverage remain pending" } else { "full headed runtime fingerprint/leak/coherence evidence remains pending" }
      "repository must not host Chromium/Firefox fork"
    } elseif (-not ($HeadedProcessLifecycleStatus -eq "passed" -and $HeadedCdpAttachStatus -eq "passed" -and $HeadedProfileCompatibilityStatus -eq "passed")) {
      "real headed external browser binary task-run smoke not attached to this report"
      "full headed runtime fingerprint/leak/coherence evidence remains pending"
      "repository must not host Chromium/Firefox fork"
    })
  }
)

$blockedAdapters = @($adapters | Where-Object { $_.blockers.Count -gt 0 })
$headedReady = @($adapters | Where-Object { $_.adapterId -eq "headed_external" -and ($_.status -eq "evidence_recorded" -or $_.status -eq "real_binary_task_recorded" -or $_.status -eq "real_binary_validation_probe_recorded" -or $_.status -eq "real_binary_repeatability_recorded") }).Count -eq 1
$lightpandaReady = @($adapters | Where-Object { $_.adapterId -eq "lightpanda" -and $_.status -eq "evidence_recorded" }).Count -eq 1

$b1b5Evidence = [ordered]@{
  validationObservedCoverage = "partial"
  fingerprintRuntimeDepth = if ($headedExternalRepeatabilityPassed) { "partial_headed_profile_browser_repeatability_observed" } elseif ($headedExternalValidationProbeObserved) { "partial_headed_profile_browser_observed" } else { "partial_26_projected_fields" }
  transportRemoteProxyTls = $remoteProxyTlsEvidenceStatus
  sessionPortability = "blocked_requires_second_machine_evidence"
  providerProductionClosure = "blocked_requires_credentials_and_real_smoke"
  runtimeAdapterEvidence = if ($headedReady -and $lightpandaReady) { "passed" } elseif ($headedExternalRepeatabilityPassed) { "partial_real_binary_repeatability_recorded" } elseif ($headedExternalValidationProbeObserved) { "partial_real_binary_validation_probe_recorded" } elseif ($headedReady -or $CamoufoxBinaryTaskStatus -eq "passed") { "partial_real_binary_task_recorded" } else { "partial_or_blocked" }
}
$adspowerRefreshStatus = if ($b1b5Evidence.validationObservedCoverage -eq "passed" `
  -and $b1b5Evidence.fingerprintRuntimeDepth -eq "passed" `
  -and $b1b5Evidence.transportRemoteProxyTls -eq "passed" `
  -and $b1b5Evidence.sessionPortability -eq "passed" `
  -and $b1b5Evidence.providerProductionClosure -eq "passed" `
  -and $b1b5Evidence.runtimeAdapterEvidence -eq "passed") {
  "ready_for_refresh"
} else {
  "deferred_by_evidence_gate"
}

$status = if ($blockedAdapters.Count -eq 0 -and $adspowerRefreshStatus -eq "ready_for_refresh") {
  "passed"
} else {
  "blocked_evidence_required"
}
$failureReason = if ($status -eq "passed") {
  ""
} else {
  "runtime adapter evidence or B1-B5 AdsPower refresh evidence is incomplete"
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
    requiredForAdsPowerRefresh = $true
    evidenceBoundary = "remote proxy egress/TLS observation only; not browser-scoped WebRTC/DNS leak closure"
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
    evidenceBoundary = "headed_external profile-browser validation probe; not remote proxy/TLS proof and not full 450 observed coverage"
  }
  headedExternalRepeatability = [ordered]@{
    status = if ([string]::IsNullOrWhiteSpace($latestHeadedExternalRepeatabilityStatus)) { "not_recorded" } else { $latestHeadedExternalRepeatabilityStatus }
    requestedCount = Get-Field $latestHeadedExternalRepeatability "requestedCount"
    executedCount = Get-Field $latestHeadedExternalRepeatability "executedCount"
    passedCount = Get-Field $latestHeadedExternalRepeatability "passedCount"
    stableSignalCount = Get-Field $latestHeadedExternalRepeatability "stableSignalCount"
    stableCategories = Get-Field $latestHeadedExternalRepeatability "stableCategories"
    stableSignalStatuses = Get-Field $latestHeadedExternalRepeatability "stableSignalStatuses"
    evidenceBoundary = "multi-run validation_probe shape stability only; not long-task behavior replay, remote proxy/TLS proof, or full 450 observed coverage"
  }
  adspowerRefreshStatus = $adspowerRefreshStatus
  failureReason = $failureReason
  notes = @(
    "This report is an evidence gate, not a headed browser implementation.",
    "Headed external runtime must remain an adapter boundary and must not turn this repository into a Chromium/Firefox fork host.",
    "Remote proxy TLS evidence is consumed from data/reports/remote-proxy-tls and stays blocked until a real remote proxy probe is recorded.",
    "AdsPower scoring must stay deferred until B1-B5 evidence is complete."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("runtime-adapter-evidence-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Runtime adapter evidence gate report: $reportPath"
Write-Host "Status: $status"
Write-Host "AdsPower refresh: $adspowerRefreshStatus"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "blocked_evidence_required") { exit 2 }
