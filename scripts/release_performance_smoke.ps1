param(
  [string]$ExePath = "",
  [int]$ReadyWaitSeconds = 8,
  [int]$IdleSampleSeconds = 5,
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
    (Join-Path $ProjectRoot "src-tauri\target\release\personal-pilot-tauri.exe"),
    (Join-Path $ProjectRoot "src-tauri\target\release\persona-pilot-desktop.exe"),
    (Join-Path $ProjectRoot "src-tauri\target\release\PersonaPilot.exe")
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
  }
}

$projectRoot = Resolve-ProjectRoot
$exe = Resolve-ReleaseExe $projectRoot $ExePath
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$startedAt = Get-Date
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$process = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden
$status = "passed"
$failureReason = $null
$readyMs = $null
$idle = $null

try {
  Start-Sleep -Seconds $ReadyWaitSeconds
  $refreshed = Get-Process -Id $process.Id -ErrorAction SilentlyContinue
  if ($null -eq $refreshed) {
    $status = "failed"
    $failureReason = "release process exited before ready wait completed"
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
  coldStartTargetMs = $coldStartTargetMs
  measuredColdStartMs = $readyMs
  idleRssTargetMb = $idleRssTargetMb
  measuredIdleRssMb = if ($null -ne $idle) { $idle.rssMb } else { $null }
  processCountTarget = $processCountTarget
  measuredProcessCount = if ($null -ne $idle) { $idle.processCount } else { $null }
  pids = if ($null -ne $idle) { $idle.pids } else { @() }
  notes = @(
    "Measures a release executable, not Vite/dev mode.",
    "Ready time is a local operator smoke approximation based on process survival after wait window.",
    "Use this as evidence input for release smoke, not as a full UX startup profiler."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("release-performance-smoke-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Release performance smoke report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "failed") { exit 1 }
