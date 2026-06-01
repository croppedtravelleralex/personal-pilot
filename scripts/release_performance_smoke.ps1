param(
  [string]$ExePath = "",
  [int]$ReadyWaitSeconds = 8,
  [int]$ReadyPollMilliseconds = 250,
  [int]$IdleSampleSeconds = 5,
  [string]$DriftReason = "",
  [switch]$CleanExistingInstances,
  [string]$OutputDir = "data/reports/release-smoke"
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Resolve-ReleaseExe([string]$ProjectRoot, [string]$RequestedPath) {
  if (-not [string]::IsNullOrWhiteSpace($RequestedPath)) {
    return (Resolve-Path $RequestedPath).Path
  }

  $candidates = @(
    (Join-Path $ProjectRoot "personal-pilot-tauri.exe")
  )
  foreach ($candidate in $candidates) {
    if (Test-Path $candidate) {
      return (Resolve-Path $candidate).Path
    }
  }

  throw "No release executable found. Run pnpm desktop:release first."
}

function Get-ProcessTree([int]$RootPid) {
  $seen = @{}
  $queue = New-Object System.Collections.Queue
  $queue.Enqueue($RootPid)
  while ($queue.Count -gt 0) {
    $processId = [int]$queue.Dequeue()
    if ($seen.ContainsKey($processId)) { continue }
    $seen[$processId] = $true
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$processId" | ForEach-Object {
      $queue.Enqueue([int]$_.ProcessId)
    }
  }
  return @($seen.Keys | ForEach-Object { [int]$_ })
}

function Measure-Tree([int]$RootPid) {
  $pids = Get-ProcessTree $RootPid
  $processes = @()
  foreach ($processId in $pids) {
    $proc = Get-Process -Id $processId -ErrorAction SilentlyContinue
    if ($null -ne $proc) {
      $processes += $proc
    }
  }
  $rssBytes = ($processes | Measure-Object -Property WorkingSet64 -Sum).Sum
  if ($null -eq $rssBytes) { $rssBytes = 0 }
  return [pscustomobject]@{
    processCount = $processes.Count
    rssMb = [int][Math]::Round($rssBytes / 1MB)
    pids = @($processes | Select-Object -ExpandProperty Id)
    processBreakdown = @(
      $processes |
        Group-Object -Property ProcessName |
        Sort-Object -Property Name |
        ForEach-Object {
          [pscustomobject][ordered]@{
            name = $_.Name
            count = $_.Count
            rssMb = [int][Math]::Round((($_.Group | Measure-Object -Property WorkingSet64 -Sum).Sum) / 1MB)
          }
        }
    )
  }
}

function Stop-ExistingInstances([string]$Exe) {
  $exeName = [System.IO.Path]::GetFileNameWithoutExtension($Exe)
  $candidateNames = @($exeName, "personal-pilot-tauri") |
    Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
    Select-Object -Unique

  foreach ($name in $candidateNames) {
    Get-Process -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
      Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
  }
  Start-Sleep -Milliseconds 500
}

function Wait-ReleaseReady([int]$RootPid, [int]$TimeoutSeconds, [int]$PollMilliseconds) {
  $poll = [Math]::Max(100, $PollMilliseconds)
  $deadline = [DateTimeOffset]::Now.AddSeconds($TimeoutSeconds)
  do {
    $root = Get-Process -Id $RootPid -ErrorAction SilentlyContinue
    if ($null -eq $root) {
      return [pscustomobject]@{
        ready = $false
        reason = "release process exited before ready signal"
        tree = $null
      }
    }
    $tree = Measure-Tree $RootPid
    $hasWebViewChild = @($tree.processBreakdown | Where-Object {
      $_.name -like "*WebView*" -or $_.name -like "*msedge*"
    }).Count -gt 0
    if ($root.MainWindowHandle -ne 0 -or $hasWebViewChild -or $tree.processCount -gt 1) {
      return [pscustomobject]@{
        ready = $true
        reason = if ($root.MainWindowHandle -ne 0) {
          "main_window_handle"
        } elseif ($hasWebViewChild) {
          "webview_process_detected"
        } else {
          "child_process_detected"
        }
        tree = $tree
      }
    }
    Start-Sleep -Milliseconds $poll
  } while ([DateTimeOffset]::Now -lt $deadline)

  return [pscustomobject]@{
    ready = $true
    reason = "timeout_survival"
    tree = Measure-Tree $RootPid
  }
}

function Get-DriftTarget([int]$Target) {
  return [int][Math]::Ceiling(($Target * 110) / 100.0)
}

function New-BudgetResult([string]$Id, [string]$Label, [string]$Unit, [int]$Target, [object]$Measured, [string]$MitigationHint, [string]$DriftReason) {
  $driftTarget = Get-DriftTarget $Target
  $measuredValue = if ($null -eq $Measured) { $null } else { [int]$Measured }
  $status = "not_measured"
  $excess = $null
  $excessPercent = $null
  $metricDriftReason = $null

  if ($null -ne $measuredValue) {
    $excess = [int]($measuredValue - $Target)
    $excessPercent = [Math]::Round((($measuredValue - $Target) * 100.0) / $Target, 1)
    if ($measuredValue -le $Target) {
      $status = "passed"
    } elseif ($measuredValue -le $driftTarget) {
      $status = "within_10_percent_drift"
      $metricDriftReason = if ([string]::IsNullOrWhiteSpace($DriftReason)) { $null } else { $DriftReason }
    } else {
      $status = "over_budget"
    }
  }

  return [pscustomobject][ordered]@{
    id = $Id
    label = $Label
    unit = $Unit
    target = $Target
    driftTarget = $driftTarget
    measured = $measuredValue
    status = $status
    excess = $excess
    excessPercent = $excessPercent
    driftReason = $metricDriftReason
    mitigationHint = $MitigationHint
  }
}

function Get-BudgetStatus([array]$BudgetResults, [string]$SmokeStatus, [string]$DriftReason) {
  if ($SmokeStatus -eq "failed") { return "failed_smoke" }
  if (@($BudgetResults | Where-Object { $_.status -eq "over_budget" }).Count -gt 0) {
    return "over_budget"
  }
  if (@($BudgetResults | Where-Object { $_.status -eq "within_10_percent_drift" }).Count -gt 0) {
    if ([string]::IsNullOrWhiteSpace($DriftReason)) {
      return "within_10_percent_drift_requires_reason"
    }
    return "within_10_percent_drift_recorded"
  }
  if (@($BudgetResults | Where-Object { $_.status -eq "not_measured" }).Count -gt 0) {
    return "unmeasured"
  }
  return "within_budget"
}

function Get-PrimaryBottleneck([array]$BudgetResults) {
  $misses = @(
    $BudgetResults |
      Where-Object { $null -ne $_.measured -and $_.measured -gt $_.target } |
      Sort-Object -Property excessPercent -Descending
  )
  if ($misses.Count -eq 0) { return "none" }
  return $misses[0].id
}

function Get-ReleaseMitigationHints([array]$BudgetResults, [object]$Idle) {
  $hints = @()
  foreach ($result in $BudgetResults) {
    if ($result.status -eq "passed" -or $result.status -eq "not_measured") { continue }
    $hints += $result.mitigationHint
  }

  if ($null -ne $Idle) {
    $webview = @($Idle.processBreakdown | Where-Object { $_.name -like "*WebView*" -or $_.name -like "*msedge*" })
    if ($webview.Count -gt 0) {
      $webviewCount = ($webview | Measure-Object -Property count -Sum).Sum
      $webviewRss = ($webview | Measure-Object -Property rssMb -Sum).Sum
      $hints += "WebView2 contributes $webviewCount processes and ${webviewRss}MB RSS; keep this visible as a platform cost and avoid eager report/history loading on first paint."
    }

    $sidecars = @($Idle.processBreakdown | Where-Object {
      $_.name -in @("personal-pilot-core", "xray", "sing-box", "lightpanda")
    })
    if ($sidecars.Count -gt 0) {
      $sidecarSummary = (($sidecars | ForEach-Object { "$($_.name)x$($_.count)" }) -join ",")
      $hints += "Sidecar processes detected: $sidecarSummary; use -CleanExistingInstances for smoke and inspect duplicate sidecars before raising performance targets."
    }
  }

  if ($hints.Count -eq 0) {
    $hints += "All measured release health budgets are within target; keep release smoke in the gate before claiming performance green."
  }

  return @($hints | Select-Object -Unique)
}

$projectRoot = Resolve-ProjectRoot
$exe = Resolve-ReleaseExe $projectRoot $ExePath
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

if ($CleanExistingInstances) {
  Stop-ExistingInstances $exe
}

$startedAt = Get-Date
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$process = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden
$status = "passed"
$failureReason = $null
$readyMs = $null
$idle = $null
$readyReason = $null

try {
  $ready = Wait-ReleaseReady $process.Id $ReadyWaitSeconds $ReadyPollMilliseconds
  $readyReason = $ready.reason
  if (-not $ready.ready) {
    $status = "failed"
    $failureReason = $ready.reason
  } else {
    $readyMs = [int]$sw.ElapsedMilliseconds
    Start-Sleep -Seconds $IdleSampleSeconds
    $idle = Measure-Tree $process.Id
  }
}
catch {
  $status = "failed"
  $failureReason = $_.Exception.Message
}
finally {
  $pids = Get-ProcessTree $process.Id
  foreach ($processId in $pids | Sort-Object -Descending) {
    Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
  }
}

$coldStartTargetMs = 2000
$idleRssTargetMb = 220
$processCountTarget = 4

$budgetResults = @(
  New-BudgetResult `
    -Id "cold_start" `
    -Label "Cold start" `
    -Unit "ms" `
    -Target $coldStartTargetMs `
    -Measured $readyMs `
    -MitigationHint "Delay heavy startup reads until after first paint, especially report history, logs, and large profile/proxy lists." `
    -DriftReason $DriftReason
  New-BudgetResult `
    -Id "idle_rss" `
    -Label "Idle RSS" `
    -Unit "MB" `
    -Target $idleRssTargetMb `
    -Measured $(if ($null -ne $idle) { $idle.rssMb } else { $null }) `
    -MitigationHint "Break down WebView2 and sidecar RSS, then lazy-load non-critical dashboard evidence and long logs." `
    -DriftReason $DriftReason
  New-BudgetResult `
    -Id "process_count" `
    -Label "Process count" `
    -Unit "processes" `
    -Target $processCountTarget `
    -Measured $(if ($null -ne $idle) { $idle.processCount } else { $null }) `
    -MitigationHint "Check for duplicate sidecars and stale release instances; record explicit sidecar exceptions instead of treating process overrun as green." `
    -DriftReason $DriftReason
)

if ($status -eq "passed") {
  $misses = @()
  foreach ($result in $budgetResults) {
    if ($null -ne $result.measured -and $result.measured -gt $result.target) {
      $misses += "$($result.id)_target_exceeded"
    }
  }
  if ($misses.Count -gt 0) {
    $status = "warning"
    $failureReason = ($misses -join ",")
  }
}

$budgetStatus = Get-BudgetStatus $budgetResults $status $DriftReason
$primaryBottleneck = Get-PrimaryBottleneck $budgetResults
$exceededMetricIds = @($budgetResults | Where-Object { $null -ne $_.measured -and $_.measured -gt $_.target } | ForEach-Object { $_.id })
$withinDriftMetricIds = @($budgetResults | Where-Object { $_.status -eq "within_10_percent_drift" } | ForEach-Object { $_.id })
$hardOverBudgetMetricIds = @($budgetResults | Where-Object { $_.status -eq "over_budget" } | ForEach-Object { $_.id })
$mitigationHints = Get-ReleaseMitigationHints $budgetResults $idle
$effectiveDriftReason = if ([string]::IsNullOrWhiteSpace($DriftReason)) {
  if ($withinDriftMetricIds.Count -gt 0) { "missing_operator_reason" } else { "not_applicable_no_metrics_within_drift" }
} else {
  $DriftReason
}
$healthStatus = if ($status -eq "failed") {
  "failed_smoke"
} elseif ($budgetStatus -eq "within_budget") {
  "healthy"
} elseif ($budgetStatus -eq "within_10_percent_drift_recorded") {
  "drift_tolerated_with_reason"
} elseif ($budgetStatus -eq "within_10_percent_drift_requires_reason") {
  "drift_reason_required"
} else {
  "budget_overrun"
}
$healthNextAction = if ($status -eq "failed") {
  "Fix release launch/readiness failure, then rerun this smoke."
} elseif ($budgetStatus -eq "within_budget") {
  "Keep this report as the current release health baseline and rerun after meaningful startup changes."
} elseif ($budgetStatus -eq "within_10_percent_drift_recorded") {
  "Record the drift reason, rerun once, and only tolerate this as short-term variance."
} elseif ($budgetStatus -eq "within_10_percent_drift_requires_reason") {
  "Rerun with -DriftReason before accepting any 10 percent release budget drift."
} else {
  "Use processBreakdown and mitigationHints to reduce cold start, RSS, or process count before claiming release performance green."
}
$healthSummaryText = "Release health $healthStatus; budget=$budgetStatus; primaryBottleneck=$primaryBottleneck; exceeded=$($exceededMetricIds -join ',')."

$report = [ordered]@{
  schemaVersion = "release_performance_smoke_v2"
  generatedAt = (Get-Date).ToString("o")
  startedAt = $startedAt.ToString("o")
  projectRoot = $projectRoot
  executablePath = $exe
  status = $status
  failureReason = $failureReason
  readyReason = $readyReason
  budgetStatus = $budgetStatus
  coldStartTargetMs = $coldStartTargetMs
  measuredColdStartMs = $readyMs
  idleRssTargetMb = $idleRssTargetMb
  measuredIdleRssMb = if ($null -ne $idle) { $idle.rssMb } else { $null }
  processCountTarget = $processCountTarget
  measuredProcessCount = if ($null -ne $idle) { $idle.processCount } else { $null }
  budgetResults = $budgetResults
  healthSummary = [ordered]@{
    status = $healthStatus
    budgetStatus = $budgetStatus
    primaryBottleneck = $primaryBottleneck
    exceededMetricIds = $exceededMetricIds
    withinDriftMetricIds = $withinDriftMetricIds
    hardOverBudgetMetricIds = $hardOverBudgetMetricIds
    driftReason = $effectiveDriftReason
    driftRule = "Each hard budget allows at most 10 percent temporary drift only when the reason is recorded; safety and live-truth fields allow no drift."
    nextAction = $healthNextAction
    summary = $healthSummaryText
  }
  mitigationHints = $mitigationHints
  pids = if ($null -ne $idle) { $idle.pids } else { @() }
  processBreakdown = if ($null -ne $idle) { $idle.processBreakdown } else { @() }
  cleanExistingInstances = [bool]$CleanExistingInstances
  notes = @(
    "Measures a release executable, not Vite/dev mode.",
    "Ready time is a local operator smoke approximation based on main window, WebView/child process detection, or timeout survival.",
    "Use this as evidence input for release smoke, not as a full UX startup profiler.",
    "budgetStatus and healthSummary classify local release health; warning still means performance green is not allowed."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("release-performance-smoke-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Release performance smoke report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "failed") { exit 1 }
