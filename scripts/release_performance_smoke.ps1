param(
  [string]$ExePath = "",
  [int]$ReadyWaitSeconds = 8,
  [int]$ReadyPollMilliseconds = 250,
  [int]$IdleSampleSeconds = 5,
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
          [ordered]@{
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
if ($status -eq "passed") {
  $misses = @()
  if ($readyMs -gt $coldStartTargetMs) { $misses += "cold_start_target_exceeded" }
  if ($idle.rssMb -gt $idleRssTargetMb) { $misses += "idle_rss_target_exceeded" }
  if ($idle.processCount -gt $processCountTarget) { $misses += "process_count_target_exceeded" }
  if ($misses.Count -gt 0) {
    $status = "warning"
    $failureReason = ($misses -join ",")
  }
}

$report = [ordered]@{
  schemaVersion = "release_performance_smoke_v1"
  generatedAt = (Get-Date).ToString("o")
  startedAt = $startedAt.ToString("o")
  projectRoot = $projectRoot
  executablePath = $exe
  status = $status
  failureReason = $failureReason
  readyReason = $readyReason
  coldStartTargetMs = $coldStartTargetMs
  measuredColdStartMs = $readyMs
  idleRssTargetMb = $idleRssTargetMb
  measuredIdleRssMb = if ($null -ne $idle) { $idle.rssMb } else { $null }
  processCountTarget = $processCountTarget
  measuredProcessCount = if ($null -ne $idle) { $idle.processCount } else { $null }
  pids = if ($null -ne $idle) { $idle.pids } else { @() }
  processBreakdown = if ($null -ne $idle) { $idle.processBreakdown } else { @() }
  cleanExistingInstances = [bool]$CleanExistingInstances
  notes = @(
    "Measures a release executable, not Vite/dev mode.",
    "Ready time is a local operator smoke approximation based on main window, WebView/child process detection, or timeout survival.",
    "Use this as evidence input for release smoke, not as a full UX startup profiler."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("release-performance-smoke-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Release performance smoke report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "failed") { exit 1 }
