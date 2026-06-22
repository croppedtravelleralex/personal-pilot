param(
  [string]$ProjectRoot = "",
  [string]$BinaryPath = "",
  [string]$ConfigPath = "",
  [string]$ProfileDir = "",
  [string]$Url = "https://example.com",
  [string]$Action = "get_title",
  [int]$TimeoutSeconds = 30,
  [int]$RepeatValidationProbeCount = 1,
  [string[]]$ExtraArgs = @(),
  [string]$ProxyServer = "",
  [string]$OutputDir = "data/reports/headed-external-smoke",
  [switch]$NoReport,
  [switch]$AllowBlocked,
  [switch]$SkipRealBinaryTask,
  [switch]$RunRustTests
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  if (-not [string]::IsNullOrWhiteSpace($ProjectRoot)) {
    return (Resolve-Path $ProjectRoot).Path
  }
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Read-TextFile([string]$Path) {
  if (-not (Test-Path $Path)) { return $null }
  return Get-Content -Path $Path -Raw
}

function New-Check([string]$Id, [bool]$Passed, [string]$Evidence, [bool]$Required = $true) {
  return [ordered]@{
    id = $Id
    status = if ($Passed) { "passed" } elseif ($Required) { "failed" } else { "not_run" }
    required = $Required
    evidence = $Evidence
  }
}

function Write-JsonNoBom([string]$Path, [object]$Value, [int]$Depth = 8) {
  $json = $Value | ConvertTo-Json -Depth $Depth
  $encoding = [System.Text.UTF8Encoding]::new($false)
  [System.IO.File]::WriteAllText($Path, $json, $encoding)
}

function Read-JsonFileCompat([string]$Path) {
  return Get-Content -Path $Path -Raw | ConvertFrom-Json
}

function Get-ObjectField([object]$Value, [string]$Name) {
  if ($null -eq $Value) { return $null }
  if ($Value -is [System.Collections.IDictionary]) { return $Value[$Name] }
  $property = $Value.PSObject.Properties[$Name]
  if ($null -eq $property) { return $null }
  return $property.Value
}

function Set-ObjectField([object]$Value, [string]$Name, [object]$FieldValue) {
  if ($Value -is [System.Collections.IDictionary]) {
    $Value[$Name] = $FieldValue
    return
  }
  $Value | Add-Member -NotePropertyName $Name -NotePropertyValue $FieldValue -Force
}

function Get-BinaryFromConfig([string]$Path) {
  if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path $Path)) { return "" }
  try {
    $json = Get-Content -Path $Path -Raw | ConvertFrom-Json
    foreach ($key in @("binary_path", "binary", "executable")) {
      if ($null -ne $json.$key -and -not [string]::IsNullOrWhiteSpace([string]$json.$key)) {
        return [string]$json.$key
      }
    }
  } catch {
    return ""
  }
  return ""
}

function Find-HeadedBrowserBinary([string]$Root) {
  $candidates = @()
  $candidates += @(Get-ChildItem -Path (Join-Path $Root "chrome") -Recurse -Filter "chrome.exe" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -ExpandProperty FullName)
  foreach ($knownPath in @(
    (Join-Path $env:ProgramFiles "Google\Chrome\Application\chrome.exe"),
    (Join-Path ${env:ProgramFiles(x86)} "Google\Chrome\Application\chrome.exe"),
    (Join-Path $env:ProgramFiles "Microsoft\Edge\Application\msedge.exe"),
    (Join-Path ${env:ProgramFiles(x86)} "Microsoft\Edge\Application\msedge.exe")
  )) {
    if (-not [string]::IsNullOrWhiteSpace($knownPath)) { $candidates += $knownPath }
  }
  foreach ($commandName in @("chrome.exe", "msedge.exe")) {
    $command = Get-Command $commandName -ErrorAction SilentlyContinue | Where-Object { $_.CommandType -eq "Application" } | Select-Object -First 1
    if ($null -ne $command -and -not [string]::IsNullOrWhiteSpace($command.Source)) {
      $candidates += $command.Source
    }
  }
  foreach ($candidate in $candidates) {
    if (-not [string]::IsNullOrWhiteSpace($candidate) -and (Test-Path $candidate)) {
      return (Resolve-Path $candidate).Path
    }
  }
  return ""
}

function New-BlockedRealBinaryTask([string]$Reason, [string]$EffectiveConfigPath, [string]$EffectiveBinaryPath) {
  return [ordered]@{
    status = "blocked_real_binary_missing"
    failureReason = $Reason
    requested = [ordered]@{ url = $Url; action = $Action; timeoutSeconds = $TimeoutSeconds }
    configPath = $EffectiveConfigPath
    binaryPath = $EffectiveBinaryPath
    evidence = [ordered]@{
      envConfig = $env:PERSONA_PILOT_HEADED_EXTERNAL_CONFIG
      envEnabled = $env:PERSONA_PILOT_HEADED_EXTERNAL_ENABLED
      pathSearch = "chrome/**/chrome.exe, installed Chrome, installed Edge, PATH chrome.exe/msedge.exe"
    }
  }
}

function Test-ContainsAll([string]$Text, [string[]]$Needles) {
  if ($null -eq $Text) { return $false }
  foreach ($needle in $Needles) {
    if (-not $Text.Contains($needle)) { return $false }
  }
  return $true
}

function Get-SignalsFromTask([object]$Task) {
  $result = Get-ObjectField $Task "result"
  if ($null -eq $result) { return @() }
  $signals = Get-ObjectField $result "validation_signals"
  if ($null -eq $signals) { return @() }
  return @($signals)
}

function Get-SignalCategories([object[]]$Signals) {
  $categories = @()
  foreach ($signal in @($Signals)) {
    $category = [string](Get-ObjectField $signal "category")
    if (-not [string]::IsNullOrWhiteSpace($category)) { $categories += $category }
  }
  return @($categories | Sort-Object -Unique)
}

function Count-SignalStatus([object[]]$Signals, [string]$Status) {
  return @(@($Signals) | Where-Object { [string](Get-ObjectField $_ "status") -eq $Status }).Count
}

function Get-SignalStatusSignature([object[]]$Signals) {
  $items = @()
  foreach ($signal in @($Signals)) {
    $id = [string](Get-ObjectField $signal "id")
    $category = [string](Get-ObjectField $signal "category")
    $status = [string](Get-ObjectField $signal "status")
    if ([string]::IsNullOrWhiteSpace($id)) { $id = $category }
    if (-not [string]::IsNullOrWhiteSpace($id) -or -not [string]::IsNullOrWhiteSpace($category)) {
      $items += ("{0}|{1}|{2}" -f $id, $category, $status)
    }
  }
  return (@($items | Sort-Object) -join ";")
}

function New-RepeatabilitySummary([object[]]$Tasks, [int]$RequestedCount) {
  $attempts = @()
  $categorySignatures = @()
  $statusSignatures = @()
  $signalCounts = @()
  $passedCount = 0
  $durations = @()

  for ($i = 0; $i -lt @($Tasks).Count; $i++) {
    $task = @($Tasks)[$i]
    $result = Get-ObjectField $task "result"
    $signals = Get-SignalsFromTask $task
    $categories = Get-SignalCategories $signals
    $signalCount = @($signals).Count
    $warningCount = Count-SignalStatus $signals "warning"
    $failedSignalCount = Count-SignalStatus $signals "failed"
    $taskStatus = [string](Get-ObjectField $task "status")
    $action = [string](Get-ObjectField $task "action")
    if ([string]::IsNullOrWhiteSpace($action)) { $action = [string](Get-ObjectField $result "action") }
    $durationMs = Get-ObjectField $task "durationMs"
    if ($null -ne $durationMs) { $durations += [int]$durationMs }
    $passed = $taskStatus -eq "passed" -and $action -eq "validation_probe" -and $signalCount -gt 0
    if ($passed) {
      $passedCount += 1
      $categorySignatures += (@($categories) -join "|")
      $statusSignatures += (Get-SignalStatusSignature $signals)
      $signalCounts += $signalCount
    }
    $attempts += [ordered]@{
      attempt = $i + 1
      status = if ([string]::IsNullOrWhiteSpace($taskStatus)) { "unknown" } else { $taskStatus }
      action = if ([string]::IsNullOrWhiteSpace($action)) { "unknown" } else { $action }
      signalCount = $signalCount
      warningCount = $warningCount
      failedSignalCount = $failedSignalCount
      categories = $categories
      durationMs = $durationMs
      finalUrl = Get-ObjectField $task "finalUrl"
      reportPath = Get-ObjectField $task "reportPath"
      failureReason = Get-ObjectField $task "failureReason"
      errorMessage = Get-ObjectField $task "errorMessage"
      exitCode = Get-ObjectField $task "exitCode"
    }
  }

  $uniqueCategorySignatures = @($categorySignatures | Sort-Object -Unique)
  $uniqueStatusSignatures = @($statusSignatures | Sort-Object -Unique)
  $uniqueSignalCounts = @($signalCounts | Sort-Object -Unique)
  $allAttemptsPassed = $passedCount -eq $RequestedCount -and @($Tasks).Count -eq $RequestedCount
  $stableCategories = $passedCount -gt 0 -and $uniqueCategorySignatures.Count -eq 1
  $stableSignalStatuses = $passedCount -gt 0 -and $uniqueStatusSignatures.Count -eq 1
  $stableSignalCount = $passedCount -gt 0 -and $uniqueSignalCounts.Count -eq 1
  $status = if ($allAttemptsPassed -and $stableCategories -and $stableSignalStatuses -and $stableSignalCount) {
    "passed_repeatability_partial_coherence"
  } elseif ($passedCount -gt 0) {
    "partial_repeatability_or_coherence"
  } else {
    "failed_repeatability"
  }
  $durationStats = if ($durations.Count -gt 0) {
    [ordered]@{
      minMs = ($durations | Measure-Object -Minimum).Minimum
      maxMs = ($durations | Measure-Object -Maximum).Maximum
      averageMs = [math]::Round(($durations | Measure-Object -Average).Average, 2)
    }
  } else {
    [ordered]@{ minMs = $null; maxMs = $null; averageMs = $null }
  }
  $categories = @()
  if ($uniqueCategorySignatures.Count -gt 0 -and -not [string]::IsNullOrWhiteSpace($uniqueCategorySignatures[0])) {
    $categories = @($uniqueCategorySignatures[0].Split("|") | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
  }

  return [ordered]@{
    schemaVersion = "headed_external_repeatability_v1"
    status = $status
    requestedCount = $RequestedCount
    executedCount = @($Tasks).Count
    passedCount = $passedCount
    allAttemptsPassed = $allAttemptsPassed
    stableSignalCount = $stableSignalCount
    stableCategories = $stableCategories
    stableSignalStatuses = $stableSignalStatuses
    categories = $categories
    signalCount = if ($uniqueSignalCounts.Count -eq 1) { $uniqueSignalCounts[0] } else { $null }
    durationStats = $durationStats
    attempts = $attempts
    failureReason = if ($status -eq "passed_repeatability_partial_coherence") { "" } else { "not all headed_external validation_probe attempts produced the same signal/category/status shape" }
    evidenceBoundary = "multi-run headed_external profile-browser validation probe shape stability for local self-use evidence"
  }
}

function Invoke-HeadedExternalTaskSmoke(
  [string]$Root,
  [string]$TaskUrl,
  [string]$TaskAction,
  [int]$TaskTimeoutSeconds,
  [string]$TaskReportPath,
  [string]$EffectiveConfigPath
) {
  $previousEnabled = $env:PERSONA_PILOT_HEADED_EXTERNAL_ENABLED
  $previousConfig = $env:PERSONA_PILOT_HEADED_EXTERNAL_CONFIG
  $previousRunner = $env:PERSONA_PILOT_RUNNER
  $env:PERSONA_PILOT_HEADED_EXTERNAL_ENABLED = "true"
  $env:PERSONA_PILOT_HEADED_EXTERNAL_CONFIG = $EffectiveConfigPath
  $env:PERSONA_PILOT_RUNNER = "headed_external"

  Push-Location $Root
  try {
    $commandOutput = & cargo run --quiet --bin headed_external_task_smoke -- --url $TaskUrl --action $TaskAction --timeout-seconds $TaskTimeoutSeconds --report $TaskReportPath 2>&1
    $taskExitCode = $LASTEXITCODE
    foreach ($line in @($commandOutput)) {
      if ($null -ne $line -and -not [string]::IsNullOrWhiteSpace([string]$line)) {
        Write-Host $line
      }
    }
  } finally {
    Pop-Location
    $env:PERSONA_PILOT_HEADED_EXTERNAL_ENABLED = $previousEnabled
    $env:PERSONA_PILOT_HEADED_EXTERNAL_CONFIG = $previousConfig
    $env:PERSONA_PILOT_RUNNER = $previousRunner
  }

  if (Test-Path $TaskReportPath) {
    try {
      $task = Read-JsonFileCompat $TaskReportPath
      Set-ObjectField $task "reportPath" $TaskReportPath
      Set-ObjectField $task "exitCode" $taskExitCode
      return $task
    } catch {
      return [ordered]@{
        status = "failed"
        failureReason = "headed_external binary task report could not be parsed: $($_.Exception.Message)"
        reportPath = $TaskReportPath
        exitCode = $taskExitCode
      }
    }
  }

  return [ordered]@{
    status = "failed"
    failureReason = "headed_external_task_smoke exited with code $taskExitCode before writing a report"
    configPath = $EffectiveConfigPath
    reportPath = $TaskReportPath
    exitCode = $taskExitCode
  }
}

$root = Resolve-ProjectRoot
$runnerMod = Read-TextFile (Join-Path $root "src\runner\mod.rs")
$runner = Read-TextFile (Join-Path $root "src\runner\headed_external.rs")
$main = Read-TextFile (Join-Path $root "src\main.rs")
$state = Read-TextFile (Join-Path $root "src-tauri\src\state.rs")
$desktop = Read-TextFile (Join-Path $root "src\desktop\mod.rs")
$timestamp = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()

$checks = @()
$checks += New-Check "runner_kind_registered" (Test-ContainsAll $runnerMod @("pub mod headed_external;", "HeadedExternal", '"headed" | "headed_external" => RunnerKind::HeadedExternal')) "src/runner/mod.rs registers RunnerKind::HeadedExternal and env mapping."
$checks += New-Check "main_selects_headed_runner" (Test-ContainsAll $main @("headed_external::HeadedExternalRunner", "RunnerKind::HeadedExternal", "Arc::new(HeadedExternalRunner)")) "src/main.rs can select headed_external runner."
$checks += New-Check "tauri_state_selects_headed_runner" (Test-ContainsAll $state @("headed_external::HeadedExternalRunner", "RunnerKind::HeadedExternal", "Arc::new(HeadedExternalRunner)")) "src-tauri/src/state.rs can select headed_external runner."
$checks += New-Check "configuration_guardrails" (Test-ContainsAll $runner @("PERSONA_PILOT_HEADED_EXTERNAL_ENABLED", "PERSONA_PILOT_HEADED_EXTERNAL_CONFIG", "runner_disabled", "runner_config_missing", "binary_not_found")) "headed_external runner blocks unconfigured launch with explicit failure reasons."
$checks += New-Check "generic_cdp_args" (Test-ContainsAll $runner @("remote_debugging_arg_template", "profile_arg_name", "startup_url", "--remote-debugging-port={port}", "--user-data-dir", "append_profile_args")) "headed_external runner exposes generic CDP/profile/startup argument controls."
$checks += New-Check "minimal_cdp_actions" (Test-ContainsAll $runner @("headed_external_minimal_cdp_v1", "spawn_headed_external", "wait_for_ws_endpoint", "Target.createTarget", "Page.navigate", "validation_probe")) "headed_external runner implements minimal CDP actions and validation probe."
$checks += New-Check "desktop_contract_updated" (Test-ContainsAll $desktop @('adapter_id: "headed_external"', 'runner_kind: "headed_external"', 'minimal_cdp_runner_source_test_backed', 'PERSONA_PILOT_HEADED_EXTERNAL_CONFIG real binary task-run smoke')) "desktop release contract exposes headed_external as implemented but real binary evidence-gated."

$optionalResults = @()
if ($RunRustTests) {
  Push-Location $root
  try {
    & cargo test --quiet headed_external
    $optionalResults += [ordered]@{ id = "cargo_test_headed_external"; status = "passed"; command = "cargo test --quiet headed_external" }
  } catch {
    $optionalResults += [ordered]@{ id = "cargo_test_headed_external"; status = "failed"; command = "cargo test --quiet headed_external"; error = $_.Exception.Message }
  } finally {
    Pop-Location
  }
} else {
  $optionalResults += [ordered]@{ id = "cargo_test_headed_external"; status = "not_run"; command = "cargo test --quiet headed_external" }
}

$realBinaryTask = [ordered]@{ status = "not_run"; reason = "SkipRealBinaryTask was set" }
$repeatability = $null
if (-not $SkipRealBinaryTask) {
  $effectiveConfigPath = $ConfigPath
  if ([string]::IsNullOrWhiteSpace($effectiveConfigPath)) {
    $effectiveConfigPath = $env:PERSONA_PILOT_HEADED_EXTERNAL_CONFIG
  }

  $effectiveBinaryPath = $BinaryPath
  if ([string]::IsNullOrWhiteSpace($effectiveBinaryPath)) {
    $effectiveBinaryPath = Get-BinaryFromConfig $effectiveConfigPath
  }
  if ([string]::IsNullOrWhiteSpace($effectiveBinaryPath)) {
    $effectiveBinaryPath = Find-HeadedBrowserBinary $root
  }

  if ([string]::IsNullOrWhiteSpace($effectiveBinaryPath) -or -not (Test-Path $effectiveBinaryPath)) {
    $realBinaryTask = New-BlockedRealBinaryTask "No CDP-capable headed browser binary was provided, configured, found under chrome/, installed Chrome/Edge, or PATH." $effectiveConfigPath $effectiveBinaryPath
  } else {
    $workDir = Join-Path $root (Join-Path ".codex_tmp\headed-external-smoke" $timestamp)
    New-Item -ItemType Directory -Force -Path $workDir | Out-Null
    if ([string]::IsNullOrWhiteSpace($ProfileDir)) {
      $ProfileDir = Join-Path $workDir "profile"
    }
    New-Item -ItemType Directory -Force -Path $ProfileDir | Out-Null

    if ([string]::IsNullOrWhiteSpace($effectiveConfigPath) -or -not (Test-Path $effectiveConfigPath)) {
      $effectiveConfigPath = Join-Path $workDir "headed-external-config.json"
      $defaultArgs = @(
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-background-networking",
        "--disable-sync",
        "--disable-features=Translate,OptimizationHints,MediaRouter",
        "--window-size=1200,800"
      )
      $config = [ordered]@{
        binary_path = (Resolve-Path $effectiveBinaryPath).Path
        profile_dir = $ProfileDir
        extra_args = @($defaultArgs + $ExtraArgs)
        proxy_server = if ([string]::IsNullOrWhiteSpace($ProxyServer)) { $null } else { $ProxyServer }
        remote_debugging_arg_template = "--remote-debugging-port={port}"
        profile_arg_name = "--user-data-dir"
        startup_url = "about:blank"
      }
      Write-JsonNoBom $effectiveConfigPath $config 8
    }

    $runCount = 1
    if ($Action.Trim().ToLowerInvariant() -eq "validation_probe" -and $RepeatValidationProbeCount -gt 1) {
      $runCount = $RepeatValidationProbeCount
    }

    $tasks = @()
    for ($attempt = 1; $attempt -le $runCount; $attempt++) {
      $suffix = if ($runCount -gt 1) { "-$attempt" } else { "" }
      $binaryReportPath = Join-Path $workDir ("headed-external-binary-task{0}.json" -f $suffix)
      $task = Invoke-HeadedExternalTaskSmoke $root $Url $Action $TimeoutSeconds $binaryReportPath $effectiveConfigPath
      if ($null -eq (Get-ObjectField $task "binaryPath")) {
        Set-ObjectField $task "binaryPath" $effectiveBinaryPath
      }
      $tasks += $task
      if ([string](Get-ObjectField $task "status") -eq "failed") { break }
    }

    if ($tasks.Count -gt 0) {
      $realBinaryTask = $tasks[0]
    }

    if ($runCount -gt 1) {
      $repeatability = New-RepeatabilitySummary $tasks $runCount
    }
  }
}

$failedRequired = @($checks | Where-Object { $_.required -and $_.status -ne "passed" })
$failedOptional = @($optionalResults | Where-Object { $_.status -eq "failed" })
$realBinaryStatus = [string](Get-ObjectField $realBinaryTask "status")
$requestedAction = $Action.Trim().ToLowerInvariant()
$status = if ($failedRequired.Count -gt 0 -or $failedOptional.Count -gt 0 -or $realBinaryStatus -eq "failed") {
  "failed"
} elseif ($realBinaryStatus -eq "passed" -and $requestedAction -eq "validation_probe") {
  if ($null -ne $repeatability -and [string](Get-ObjectField $repeatability "status") -eq "passed_repeatability_partial_coherence") {
    "passed_real_binary_repeatability"
  } else {
    "passed_real_binary_validation_probe"
  }
} elseif ($realBinaryStatus -eq "passed") {
  "passed_real_binary_task"
} elseif ($realBinaryStatus -eq "blocked_real_binary_missing") {
  "blocked_real_binary_missing"
} else {
  "passed_source_contract"
}

$report = [ordered]@{
  schemaVersion = "headed_external_smoke_v2"
  generatedAt = [DateTimeOffset]::Now.ToString("o")
  projectRoot = $root
  status = $status
  checks = $checks
  optionalResults = $optionalResults
  realBinaryTask = $realBinaryTask
  repeatability = $repeatability
  requiredRuntimeProof = @(
    "set PERSONA_PILOT_RUNNER=headed_external",
    "set PERSONA_PILOT_HEADED_EXTERNAL_CONFIG to a real CDP-capable browser binary",
    "run an open/get_title/get_html/validation_probe task and preserve report",
    "verify timeout cleanup leaves no child process residue"
  )
  notes = @(
    "This is source/contract smoke for the generic headed_external runtime.",
    "Real binary task-run evidence is separate and must not be implied by this report."
  )
}

if (-not $NoReport) {
  $absoluteOutputDir = Join-Path $root $OutputDir
  New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null
  $reportPath = Join-Path $absoluteOutputDir ("headed-external-smoke-{0}.json" -f $timestamp)
  Write-JsonNoBom $reportPath $report 12
  Write-Host "Headed external smoke report: $reportPath"
}

Write-Host "Headed external smoke status: $status"
if ($status -eq "failed") { exit 1 }
if ($status -like "blocked_*" -and -not $AllowBlocked) { exit 2 }
