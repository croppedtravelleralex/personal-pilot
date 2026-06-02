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

function Test-SessionBundleOperatorContract {
  $settingsPath = Join-Path $projectRoot "src\modules\settings\SettingsPage.tsx"
  $desktopServicePath = Join-Path $projectRoot "src\services\desktop.ts"
  $desktopTypesPath = Join-Path $projectRoot "src\types\desktop.ts"
  $rustDesktopPath = Join-Path $projectRoot "src\desktop\mod.rs"
  $failures = @()

  foreach ($path in @($settingsPath, $desktopServicePath, $desktopTypesPath, $rustDesktopPath)) {
    if (-not (Test-Path $path)) {
      $failures += "missing source file: $path"
    }
  }

  $settingsText = if (Test-Path $settingsPath) { Get-Content -LiteralPath $settingsPath -Raw -Encoding UTF8 } else { "" }
  $desktopServiceText = if (Test-Path $desktopServicePath) { Get-Content -LiteralPath $desktopServicePath -Raw -Encoding UTF8 } else { "" }
  $desktopTypesText = if (Test-Path $desktopTypesPath) { Get-Content -LiteralPath $desktopTypesPath -Raw -Encoding UTF8 } else { "" }
  $rustDesktopText = if (Test-Path $rustDesktopPath) { Get-Content -LiteralPath $rustDesktopPath -Raw -Encoding UTF8 } else { "" }

  foreach ($token in @(
      "SessionBundle",
      "exportSessionBundle",
      "preflightSessionBundleImport",
      "restoreSessionBundle",
      "Dry-run",
      "handleSessionBundleRestore",
      "writePerformed",
      "portability",
      "allowProfileOverwrite",
      "confirm("
    )) {
    if ($settingsText -notmatch [regex]::Escape($token)) {
      $failures += "Settings SessionBundle operator UI missing marker: $token"
    }
  }

  foreach ($token in @("export_session_bundle", "preflight_session_bundle_import", "restore_session_bundle")) {
    if ($desktopServiceText -notmatch [regex]::Escape($token)) {
      $failures += "desktop service missing SessionBundle command wrapper: $token"
    }
  }

  foreach ($token in @(
      "DesktopSessionBundleExport",
      "DesktopSessionBundleImportPreflight",
      "DesktopSessionBundleRestoreResult",
      "restoreSupported",
      "writePerformed"
    )) {
    if ($desktopTypesText -notmatch [regex]::Escape($token)) {
      $failures += "desktop types missing SessionBundle type marker: $token"
    }
  }

  foreach ($token in @(
      "export_desktop_session_bundle",
      "preflight_desktop_session_bundle_import",
      "restore_desktop_session_bundle",
      "session_bundle_import_preflight_and_restore_write_confirmed_copy"
    )) {
    if ($rustDesktopText -notmatch [regex]::Escape($token)) {
      $failures += "Rust desktop SessionBundle implementation/test marker missing: $token"
    }
  }

  if ($failures.Count -gt 0) {
    return New-LocalGateResult "session_bundle_operator_contract" "missing_coverage" "failed" "M4.6 SessionBundle operator source contract is incomplete" $failures
  }
  return New-LocalGateResult "session_bundle_operator_contract" "passed" "passed" "M4.6 SessionBundle export/preflight/dry-run/confirmed local restore operator loop is wired in UI/API; cross-machine proof remains externally blocked" @()
}

function Test-TypedFacadeShrinkContract {
  $desktopServicePath = Join-Path $projectRoot "src\services\desktop.ts"
  $desktopTypesPath = Join-Path $projectRoot "src\types\desktop.ts"
  $syncApiPath = Join-Path $projectRoot "src\modules\synchronizer\api.ts"
  $browserApiPath = Join-Path $projectRoot "src\modules\browser\api.ts"
  $failures = @()

  foreach ($path in @($desktopServicePath, $desktopTypesPath, $syncApiPath, $browserApiPath)) {
    if (-not (Test-Path $path)) {
      $failures += "missing source file: $path"
    }
  }

  $desktopServiceText = if (Test-Path $desktopServicePath) { Get-Content -LiteralPath $desktopServicePath -Raw -Encoding UTF8 } else { "" }
  $desktopTypesText = if (Test-Path $desktopTypesPath) { Get-Content -LiteralPath $desktopTypesPath -Raw -Encoding UTF8 } else { "" }
  $syncApiText = if (Test-Path $syncApiPath) { Get-Content -LiteralPath $syncApiPath -Raw -Encoding UTF8 } else { "" }
  $browserApiText = if (Test-Path $browserApiPath) { Get-Content -LiteralPath $browserApiPath -Raw -Encoding UTF8 } else { "" }

  foreach ($token in @(
      "DesktopCoreSyncGroup",
      "DesktopCoreSyncOperation",
      "DesktopCoreSyncWindowPlacement",
      "DesktopCoreWorkbenchTask"
    )) {
    if ($desktopTypesText -notmatch [regex]::Escape($token)) {
      $failures += "desktop shared type missing: $token"
    }
    if ($desktopServiceText -notmatch [regex]::Escape($token)) {
      $failures += "desktop service wrapper does not use shared type: $token"
    }
  }

  foreach ($token in @(
      "synchronizerListGroups = (): Promise<unknown[]>",
      "synchronizerArrangeProfiles = (`r`n  profileIds: string[],`r`n  layout: `"grid`" | `"main-left`",`r`n): Promise<unknown[]>",
      "synchronizerGetOperationLog = (limit = 50): Promise<unknown[]>",
      "synchronizerListTasks = (limit = 200): Promise<unknown[]>"
    )) {
    if ($desktopServiceText -match [regex]::Escape($token)) {
      $failures += "desktop service still exposes unknown typed synchronizer facade: $token"
    }
  }

  foreach ($token in @(
      "synchronizerListGroups() as Promise<SyncGroup[]>",
      "synchronizerArrangeProfiles(profileIds, layout) as Promise<SyncWindowPlacement[]>",
      "synchronizerGetOperationLog(limit ?? 50) as Promise<SyncOperation[]>",
      "synchronizerListTasks(limit ?? 200) as Promise<WorkbenchTask[]>"
    )) {
    if ($syncApiText -match [regex]::Escape($token)) {
      $failures += "synchronizer API still casts high-traffic facade result: $token"
    }
  }

  foreach ($token in @(
      "type BrowserNativeBindings = Partial<{",
      "BrowserProfileList: () => Promise<BrowserProfile[]>",
      "BrowserInstanceStart: (profileId: string) => Promise<BrowserProfile>",
      "BrowserProxyBatchTestSpeed: (proxyIds: string[], concurrency: number) => Promise<ProxyTestResult[]>",
      "BehaviorRecordingSummaryList: () => Promise<RecordingSummary[]>",
      "LLMExecuteTask: (profileId: string, taskDescription: string) => Promise<void>",
      "function getWindowGoApp(): BrowserNativeBindings | null",
      "const bindings = await getBindings()"
    )) {
    if ($browserApiText -notmatch [regex]::Escape($token)) {
      $failures += "browser API missing typed Wails binding marker: $token"
    }
  }

  foreach ($token in @("const bindings: any = await getBindings()", "const goApp = (window as any).go?.main?.App")) {
    if ($browserApiText -match [regex]::Escape($token)) {
      $failures += "browser API still uses dynamic Wails binding marker: $token"
    }
  }

  if ($failures.Count -gt 0) {
    return New-LocalGateResult "typed_facade_shrink_contract" "missing_coverage" "failed" "M4.8 typed facade shrink source contract is incomplete" $failures
  }
  return New-LocalGateResult "typed_facade_shrink_contract" "passed" "passed" "M4.8 synchronizer DTOs and browser Wails bindings have typed source contracts; Wails bridge remains transitional" @()
}

function Test-BrowserPayloadSchemaContract {
  $apiPath = Join-Path $projectRoot "src\modules\browser\api.ts"
  $typesPath = Join-Path $projectRoot "src\modules\browser\types.ts"
  $listPagePath = Join-Path $projectRoot "src\modules\browser\pages\BrowserListPage.tsx"
  $detailPagePath = Join-Path $projectRoot "src\modules\browser\pages\BrowserDetailPage.tsx"
  $failures = @()

  foreach ($path in @($apiPath, $typesPath, $listPagePath, $detailPagePath)) {
    if (-not (Test-Path $path)) {
      $failures += "missing source file: $path"
    }
  }

  $apiText = if (Test-Path $apiPath) { Get-Content -LiteralPath $apiPath -Raw -Encoding UTF8 } else { "" }
  $typesText = if (Test-Path $typesPath) { Get-Content -LiteralPath $typesPath -Raw -Encoding UTF8 } else { "" }
  $listText = if (Test-Path $listPagePath) { Get-Content -LiteralPath $listPagePath -Raw -Encoding UTF8 } else { "" }
  $detailText = if (Test-Path $detailPagePath) { Get-Content -LiteralPath $detailPagePath -Raw -Encoding UTF8 } else { "" }

  foreach ($token in @("BrowserRuntimeEventPayload", "profileId", "error")) {
    if ($typesText -notmatch [regex]::Escape($token)) {
      $failures += "browser runtime event payload type missing marker: $token"
    }
  }

  foreach ($token in @("normalizeBrowserRuntimeEventPayload", "BrowserRuntimeEventPayload", "profile_id", "lastError")) {
    if ($apiText -notmatch [regex]::Escape($token)) {
      $failures += "browser runtime event normalizer missing marker: $token"
    }
  }

  foreach ($token in @("normalizeBrowserRuntimeEventPayload")) {
    if ($listText -notmatch [regex]::Escape($token)) {
      $failures += "BrowserListPage does not use browser runtime payload normalizer"
    }
    if ($detailText -notmatch [regex]::Escape($token)) {
      $failures += "BrowserDetailPage does not use browser runtime payload normalizer"
    }
  }

  foreach ($textAndName in @(
      [pscustomobject]@{ text = $listText; name = "BrowserListPage" },
      [pscustomobject]@{ text = $detailText; name = "BrowserDetailPage" }
    )) {
    if ($textAndName.text -match [regex]::Escape("payload: any")) {
      $failures += "$($textAndName.name) still uses payload:any for runtime events"
    }
  }

  foreach ($token in @("normalizeLaunchServerInfo(payload: any)", "normalizeRecordingDetail(payload: any")) {
    if ($apiText -match [regex]::Escape($token)) {
      $failures += "browser api normalizer still accepts any: $token"
    }
  }

  if ($failures.Count -gt 0) {
    return New-LocalGateResult "browser_payload_schema_contract" "missing_coverage" "failed" "M4.8 browser payload schema source contract is incomplete" $failures
  }
  return New-LocalGateResult "browser_payload_schema_contract" "passed" "passed" "M4.8 browser runtime event payload is normalized through a shared type; this does not remove every bridge compatibility path" @()
}

function Test-DashboardFacadeContract {
  $dashboardApiPath = Join-Path $projectRoot "src\modules\dashboard\api.ts"
  $dashboardPagePath = Join-Path $projectRoot "src\modules\dashboard\DashboardPage.tsx"
  $desktopServicePath = Join-Path $projectRoot "src\services\desktop.ts"
  $failures = @()

  foreach ($path in @($dashboardApiPath, $dashboardPagePath, $desktopServicePath)) {
    if (-not (Test-Path $path)) {
      $failures += "missing source file: $path"
    }
  }

  $dashboardApiText = if (Test-Path $dashboardApiPath) { Get-Content -LiteralPath $dashboardApiPath -Raw -Encoding UTF8 } else { "" }
  $dashboardPageText = if (Test-Path $dashboardPagePath) { Get-Content -LiteralPath $dashboardPagePath -Raw -Encoding UTF8 } else { "" }
  $desktopServiceText = if (Test-Path $desktopServicePath) { Get-Content -LiteralPath $desktopServicePath -Raw -Encoding UTF8 } else { "" }

  foreach ($token in @("readDashboardStats", "readLicenseStatus", "reloadDesktopConfig", "generateDesktopCdKeys")) {
    if ($dashboardApiText -notmatch [regex]::Escape($token)) {
      $failures += "dashboard API does not use typed desktop wrapper: $token"
    }
  }

  foreach ($token in @("const bindings: any", "import('../../wailsjs/go/main/App')", "bindings.", "getBindings")) {
    if ($dashboardApiText -match [regex]::Escape($token)) {
      $failures += "dashboard API still uses raw Wails/dashboard command marker: $token"
    }
  }

  foreach ($token in @("readDashboardStats", "readLicenseStatus", "reloadDesktopConfig", "generateDesktopCdKeys", "DesktopDashboardStatsResponse", "DesktopLicenseStatusResponse")) {
    if ($desktopServiceText -notmatch [regex]::Escape($token)) {
      $failures += "desktop service missing dashboard typed facade marker: $token"
    }
  }

  foreach ($token in @("fetchDashboardStats", "fetchEvidenceReportHistory", "fetchReleaseSmokeContract", "M4 Payload", "Runtime Adapter")) {
    if ($dashboardPageText -notmatch [regex]::Escape($token)) {
      $failures += "Dashboard page missing expected evidence/stat marker: $token"
    }
  }

  if ($failures.Count -gt 0) {
    return New-LocalGateResult "dashboard_facade_contract" "missing_coverage" "failed" "M4.8 Dashboard facade source contract is incomplete" $failures
  }
  return New-LocalGateResult "dashboard_facade_contract" "passed" "passed" "M4.8 Dashboard API uses typed desktop service wrappers while preserving evidence rows; Wails bridge remains transitional" @()
}

function Test-SettingsLogsFacadeContract {
  $settingsApiPath = Join-Path $projectRoot "src\modules\settings\api.ts"
  $logsPagePath = Join-Path $projectRoot "src\modules\browser\pages\BrowserLogsPage.tsx"
  $desktopServicePath = Join-Path $projectRoot "src\services\desktop.ts"
  $bridgePath = Join-Path $projectRoot "src\services\tauriWailsBridge.ts"
  $dashboardPagePath = Join-Path $projectRoot "src\modules\dashboard\DashboardPage.tsx"
  $failures = @()

  foreach ($path in @($settingsApiPath, $logsPagePath, $desktopServicePath, $bridgePath, $dashboardPagePath)) {
    if (-not (Test-Path $path)) {
      $failures += "missing source file: $path"
    }
  }

  $settingsApiText = if (Test-Path $settingsApiPath) { Get-Content -LiteralPath $settingsApiPath -Raw -Encoding UTF8 } else { "" }
  $logsPageText = if (Test-Path $logsPagePath) { Get-Content -LiteralPath $logsPagePath -Raw -Encoding UTF8 } else { "" }
  $desktopServiceText = if (Test-Path $desktopServicePath) { Get-Content -LiteralPath $desktopServicePath -Raw -Encoding UTF8 } else { "" }
  $bridgeText = if (Test-Path $bridgePath) { Get-Content -LiteralPath $bridgePath -Raw -Encoding UTF8 } else { "" }
  $dashboardPageText = if (Test-Path $dashboardPagePath) { Get-Content -LiteralPath $dashboardPagePath -Raw -Encoding UTF8 } else { "" }

  foreach ($token in @("initializeSystemDataFromDesktop", "exportSystemConfigFromDesktop", "importSystemConfigFromDesktop", "DesktopBackupActionResult")) {
    if ($settingsApiText -notmatch [regex]::Escape($token)) {
      $failures += "settings API does not use typed backup wrapper marker: $token"
    }
  }

  foreach ($token in @("const bindings: any", "import('../../wailsjs/go/main/App')", "bindings.", "getBindings")) {
    if ($settingsApiText -match [regex]::Escape($token)) {
      $failures += "settings API still uses raw Wails binding marker: $token"
    }
  }

  foreach ($token in @("getAppLogs", "clearAppLogs", "DesktopJsonValue")) {
    if ($logsPageText -notmatch [regex]::Escape($token)) {
      $failures += "BrowserLogsPage missing typed log wrapper marker: $token"
    }
  }

  foreach ($token in @("const bindings: any", "import('../../../wailsjs/go/main/App')", "bindings.")) {
    if ($logsPageText -match [regex]::Escape($token)) {
      $failures += "BrowserLogsPage still uses raw Wails binding marker: $token"
    }
  }

  foreach ($token in @("DesktopBackupActionResult", "DesktopDestructivePreflight", "initializeSystemData", "exportSystemConfig", "importSystemConfig", "getAppLogs", "clearAppLogs", "confirmDestructivePreflight")) {
    if ($desktopServiceText -notmatch [regex]::Escape($token)) {
      $failures += "desktop service missing settings/logs typed facade marker: $token"
    }
  }

  foreach ($token in @("BackupInitializeSystem", "BackupExportPackage", "BackupImportPackage", "GetAppLogs", "ClearAppLogs")) {
    if ($bridgeText -notmatch [regex]::Escape($token)) {
      $failures += "tauriWailsBridge missing compatibility proxy marker: $token"
    }
  }

  foreach ($token in @("m4_settings_logs_facade", "M4 Settings/Logs")) {
    if ($dashboardPageText -notmatch [regex]::Escape($token)) {
      $failures += "Dashboard page missing settings/logs evidence marker: $token"
    }
  }

  if ($failures.Count -gt 0) {
    return New-LocalGateResult "settings_logs_facade_contract" "missing_coverage" "failed" "M4.8 Settings/logs facade source contract is incomplete" $failures
  }
  return New-LocalGateResult "settings_logs_facade_contract" "passed" "passed" "M4.8 Settings backup and Browser logs use typed desktop service wrappers while preserving the transitional bridge" @()
}

function Test-ProfileFacadeContract {
  $profileApiPath = Join-Path $projectRoot "src\modules\profile\api.ts"
  $desktopServicePath = Join-Path $projectRoot "src\services\desktop.ts"
  $dashboardPagePath = Join-Path $projectRoot "src\modules\dashboard\DashboardPage.tsx"
  $failures = @()

  foreach ($path in @($profileApiPath, $desktopServicePath, $dashboardPagePath)) {
    if (-not (Test-Path $path)) {
      $failures += "missing source file: $path"
    }
  }

  $profileApiText = if (Test-Path $profileApiPath) { Get-Content -LiteralPath $profileApiPath -Raw -Encoding UTF8 } else { "" }
  $desktopServiceText = if (Test-Path $desktopServicePath) { Get-Content -LiteralPath $desktopServicePath -Raw -Encoding UTF8 } else { "" }
  $dashboardPageText = if (Test-Path $dashboardPagePath) { Get-Content -LiteralPath $dashboardPagePath -Raw -Encoding UTF8 } else { "" }

  foreach ($token in @("fetchRemoteAuthorProfileFromDesktop", "Record<string, unknown>", "normalizeChannel(value: unknown)", "errorMessage(error)")) {
    if ($profileApiText -notmatch [regex]::Escape($token)) {
      $failures += "profile API does not use typed profile facade marker: $token"
    }
  }

  foreach ($token in @("const bindings: any", "import('../../wailsjs/go/main/App')", "getBindings", "(window as any).go?.main?.App", "Record<string, any>", "normalizeChannel(value: any)")) {
    if ($profileApiText -match [regex]::Escape($token)) {
      $failures += "profile API still uses raw Wails/any marker: $token"
    }
  }

  foreach ($token in @("fetchRemoteAuthorPayloadViaBrowser", "fetch(authorURL", "AbortController", "Accept: 'application/json'")) {
    if ($profileApiText -notmatch [regex]::Escape($token)) {
      $failures += "profile API browser preview fallback missing marker: $token"
    }
  }

  foreach ($token in @("fetchRemoteAuthorProfileFromDesktop", "FetchRemoteAuthorProfile", "Promise<Record<string, unknown>>")) {
    if ($desktopServiceText -notmatch [regex]::Escape($token)) {
      $failures += "desktop service missing profile typed facade marker: $token"
    }
  }

  foreach ($token in @("m4_profile_facade", "M4 Profile")) {
    if ($dashboardPageText -notmatch [regex]::Escape($token)) {
      $failures += "Dashboard page missing profile facade evidence marker: $token"
    }
  }

  if ($failures.Count -gt 0) {
    return New-LocalGateResult "profile_facade_contract" "missing_coverage" "failed" "M4.8 Profile facade source contract is incomplete" $failures
  }
  return New-LocalGateResult "profile_facade_contract" "passed" "passed" "M4.8 Profile remote author loading uses the typed desktop service wrapper while preserving browser preview fallback" @()
}

function Test-RuntimeAdapterOperatorContract {
  $dashboardPath = Join-Path $projectRoot "src\modules\dashboard\DashboardPage.tsx"
  $dashboardApiPath = Join-Path $projectRoot "src\modules\dashboard\api.ts"
  $desktopServicePath = Join-Path $projectRoot "src\services\desktop.ts"
  $desktopTypesPath = Join-Path $projectRoot "src\types\desktop.ts"
  $failures = @()

  foreach ($path in @($dashboardPath, $dashboardApiPath, $desktopServicePath, $desktopTypesPath)) {
    if (-not (Test-Path $path)) {
      $failures += "missing source file: $path"
    }
  }

  $dashboardText = if (Test-Path $dashboardPath) { Get-Content -LiteralPath $dashboardPath -Raw -Encoding UTF8 } else { "" }
  $dashboardApiText = if (Test-Path $dashboardApiPath) { Get-Content -LiteralPath $dashboardApiPath -Raw -Encoding UTF8 } else { "" }
  $desktopServiceText = if (Test-Path $desktopServicePath) { Get-Content -LiteralPath $desktopServicePath -Raw -Encoding UTF8 } else { "" }
  $desktopTypesText = if (Test-Path $desktopTypesPath) { Get-Content -LiteralPath $desktopTypesPath -Raw -Encoding UTF8 } else { "" }

  foreach ($token in @(
      "Runtime Adapter",
      "fetchReleaseSmokeContract",
      "DesktopRuntimeAdapterContractItem",
      "DesktopReleaseSmokeContract",
      "runtimeAdapterEvidenceScore",
      "rankedRuntimeAdapters",
      "adapter.blockers.slice(0, 2)",
      "profileRuntimeEvidence",
      "fingerprintRuntimeDepth",
      "releaseContract"
    )) {
    if ($dashboardText -notmatch [regex]::Escape($token)) {
      $failures += "Dashboard runtime adapter operator UI missing marker: $token"
    }
  }

  foreach ($token in @("fetchReleaseSmokeContract", "readReleaseSmokeContract", "DesktopReleaseSmokeContract")) {
    if ($dashboardApiText -notmatch [regex]::Escape($token)) {
      $failures += "Dashboard API missing runtime adapter contract marker: $token"
    }
  }

  foreach ($token in @("readReleaseSmokeContract", "read_release_smoke_contract")) {
    if ($desktopServiceText -notmatch [regex]::Escape($token)) {
      $failures += "desktop service missing release smoke contract wrapper: $token"
    }
  }

  foreach ($token in @(
      "DesktopRuntimeAdapterContractItem",
      "DesktopReleaseSmokeContract",
      "adapterContracts",
      "profileRuntimeEvidence",
      "fingerprintRuntimeDepth",
      "blockers"
    )) {
    if ($desktopTypesText -notmatch [regex]::Escape($token)) {
      $failures += "desktop shared type missing runtime adapter marker: $token"
    }
  }

  if ($failures.Count -gt 0) {
    return New-LocalGateResult "runtime_adapter_operator_contract" "missing_coverage" "failed" "M4.7 runtime adapter operator source contract is incomplete" $failures
  }
  return New-LocalGateResult "runtime_adapter_operator_contract" "passed" "passed" "M4.7 runtime adapter evidence is visible and ranked in Dashboard; full headed realism remains externally blocked" @()
}

function Test-SafetyLoggingContract {
  $redactionPath = Join-Path $projectRoot "backend\internal\logger\redaction.go"
  $redactionTestPath = Join-Path $projectRoot "backend\internal\logger\redaction_test.go"
  $loggerPath = Join-Path $projectRoot "backend\internal\logger\logger.go"
  $formatterPath = Join-Path $projectRoot "backend\internal\logger\formatter.go"
  $configPath = Join-Path $projectRoot "backend\internal\config\config.go"
  $failures = @()

  foreach ($path in @($redactionPath, $redactionTestPath, $loggerPath, $formatterPath, $configPath)) {
    if (-not (Test-Path $path)) {
      $failures += "missing source file: $path"
    }
  }

  $redactionText = if (Test-Path $redactionPath) { Get-Content -LiteralPath $redactionPath -Raw -Encoding UTF8 } else { "" }
  $redactionTestText = if (Test-Path $redactionTestPath) { Get-Content -LiteralPath $redactionTestPath -Raw -Encoding UTF8 } else { "" }
  $loggerText = if (Test-Path $loggerPath) { Get-Content -LiteralPath $loggerPath -Raw -Encoding UTF8 } else { "" }
  $formatterText = if (Test-Path $formatterPath) { Get-Content -LiteralPath $formatterPath -Raw -Encoding UTF8 } else { "" }
  $configText = if (Test-Path $configPath) { Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 } else { "" }

  foreach ($token in @(
      "DefaultSensitiveFieldNames",
      "IsSensitiveLogField",
      "RedactText",
      "RedactValueForKey",
      "RedactLogEntry",
      "password",
      "token",
      "api_key",
      "authorization",
      "credential",
      "secret",
      "cookie"
    )) {
    if ($redactionText -notmatch [regex]::Escape($token)) {
      $failures += "logger redaction source missing marker: $token"
    }
  }

  foreach ($token in @("RedactText(msg)", "RedactValueForKey(field.Key, field.Value)", "RedactLogEntry(entry)")) {
    if ($loggerText -notmatch [regex]::Escape($token)) {
      $failures += "logger write path missing redaction marker: $token"
    }
  }

  if (@([regex]::Matches($formatterText, [regex]::Escape("RedactLogEntry(entry)"))).Count -lt 2) {
    $failures += "text/json formatters do not both redact log entries"
  }

  foreach ($token in @(
      "TestM4SafetyLoggingContract_RedactsCredentialFields",
      "TestM4SafetyLoggingContract_FormattersDoNotEmitSecrets",
      "plain-password",
      "plain-api-key",
      "plain-bearer-token",
      "plain-json-token"
    )) {
    if ($redactionTestText -notmatch [regex]::Escape($token)) {
      $failures += "logger redaction test missing marker: $token"
    }
  }

  foreach ($token in @("api_key", "authorization", "credential", "client_secret", "cookie")) {
    if ($configText -notmatch [regex]::Escape($token)) {
      $failures += "default logger sensitive field config missing marker: $token"
    }
  }

  if ($failures.Count -gt 0) {
    return New-LocalGateResult "safety_logging_contract" "missing_coverage" "failed" "M4.9 safety/logging source-test contract is incomplete" $failures
  }
  return New-LocalGateResult "safety_logging_contract" "passed" "passed" "M4.9 logger redaction source/test contract is present; this is local safety evidence, not real credential smoke" @()
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
      } elseif ($status -in @("blocked_missing_validation_report", "blocked_missing_desktop_webview_report", "blocked_missing_profile_browser_report", "blocked_stale_comparison_pair", "partial_comparison_only")) {
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
$gates += Test-SessionBundleOperatorContract
$gates += Test-TypedFacadeShrinkContract
$gates += Test-BrowserPayloadSchemaContract
$gates += Test-DashboardFacadeContract
$gates += Test-SettingsLogsFacadeContract
$gates += Test-ProfileFacadeContract
$gates += Test-RuntimeAdapterOperatorContract
$gates += Test-SafetyLoggingContract

$failedGates = @($gates | Where-Object { $_.classification -eq "failed" })
$expectedBlockedGates = @($gates | Where-Object { $_.classification -eq "expected_blocked" })
$passedGates = @($gates | Where-Object { $_.classification -eq "passed" })

$gateClassificationStatus = if ($failedGates.Count -gt 0) {
  "failed"
} elseif ($expectedBlockedGates.Count -gt 0) {
  "expected_blocked"
} else {
  "passed"
}

$operatorStatus = if ($failedGates.Count -gt 0) {
  "failed"
} elseif ($expectedBlockedGates.Count -gt 0) {
  "passed_with_expected_external_blockers"
} else {
  "passed"
}

$report = [ordered]@{
  schemaVersion = "m4_acceptance_gate_v12"
  generatedAt = (Get-Date).ToString("o")
  status = $operatorStatus
  gateClassificationStatus = $gateClassificationStatus
  operatorStatus = $operatorStatus
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
  m4TotalGate = [ordered]@{
    status = $operatorStatus
    gateClassificationStatus = $gateClassificationStatus
    localContractStatus = if ($failedGates.Count -eq 0) { "passed" } else { "failed" }
    externalBlockerCount = $expectedBlockedGates.Count
    failedGateIds = @($failedGates | ForEach-Object { $_.id })
    expectedBlockedGateIds = @($expectedBlockedGates | ForEach-Object { $_.id })
    passedGateIds = @($passedGates | ForEach-Object { $_.id })
    nextAction = if ($failedGates.Count -gt 0) {
      "Fix failed local gates before promoting M4."
    } elseif ($expectedBlockedGates.Count -gt 0) {
      "M4 local contracts are usable; close expected external blockers with fresh provider, remote proxy/TLS, cross-machine SessionBundle, and same-run profile-browser evidence."
    } else {
      "M4 local and external gates are passed; move to M5 performance and health."
    }
  }
  notes = @(
    "External blockers are allowed only as expected_blocked.",
    "Live truth drift, missing critical reports, unreadable reports, and passed reports with remaining blockers fail this gate.",
    "This gate refreshes live_truth_guard unless -SkipLiveTruthRefresh is set and refreshes provider acceptance preflight unless -SkipProviderPreflightRefresh is set; external gates are aggregated from latest reports.",
    "Local automation primitive contract checks source/test coverage markers only; behavioral proof still comes from go test.",
    "Provider dry-run contract is local report schema evidence only; provider acceptance remains expected_blocked until credential-backed real smoke passes.",
    "SessionBundle operator contract is local UI/API source evidence only; second-machine portability remains expected_blocked until a real target-environment report exists.",
    "Typed facade shrink contract is source-level evidence only; it narrows high-traffic synchronizer DTOs and browser Wails bindings without removing the transitional bridge.",
    "Browser payload schema contract is source-level evidence only; it normalizes browser runtime event payloads and selected browser API normalizer inputs without removing every bridge compatibility path.",
    "Dashboard facade contract is source-level evidence only; it routes Dashboard stats/license/config/CD key calls through typed desktop service wrappers without removing every bridge compatibility path.",
    "Settings/logs facade contract is source-level evidence only; it routes Settings backup and Browser logs through typed desktop service wrappers while preserving the transitional compatibility bridge.",
    "Profile facade contract is source-level evidence only; it routes Profile remote author loading through a typed desktop service wrapper while preserving browser preview fallback.",
    "Runtime adapter operator contract is source-level UI/API evidence only; full headed realism still requires repeatability/coherence, proxy/TLS, provider, portability, and B1-B5 reports.",
    "Safety logging contract is local source/test evidence only; credential-backed provider smoke and external reports still require their own evidence.",
    "M4 total gate reports passed_with_expected_external_blockers when local gates pass and only expected external blockers remain; gateClassificationStatus preserves the lower-level expected_blocked classification."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m4-acceptance-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding UTF8

Write-Host "M4 acceptance gate report: $reportPath"
Write-Host "Status: $operatorStatus"
Write-Host "Gate classification: $gateClassificationStatus"
Write-Host "Summary: passed=$($passedGates.Count), expected_blocked=$($expectedBlockedGates.Count), failed=$($failedGates.Count)"
if ($report.failureReason) { Write-Host "Failure reason: $($report.failureReason)" }
if ($report.expectedBlockedReason) { Write-Host "Expected blocked: $($report.expectedBlockedReason)" }

if ($gateClassificationStatus -eq "failed") { exit 1 }
exit 0
