param(
  [string]$OutputDir = "data/reports/m15-browser-process",
  [string]$BrowserPath = "",
  [int]$CdpReadyTimeoutSeconds = 20
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Find-BrowserBinary([string]$Root, [string]$RequestedPath) {
  if (-not [string]::IsNullOrWhiteSpace($RequestedPath) -and (Test-Path $RequestedPath)) {
    return (Resolve-Path $RequestedPath).Path
  }

  $candidates = @()
  $candidates += @(Get-ChildItem -Path (Join-Path $Root "chrome") -Filter "chrome.exe" -Recurse -File -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -ExpandProperty FullName)
  foreach ($knownPath in @(
    (Join-Path $env:ProgramFiles "Google\Chrome\Application\chrome.exe"),
    (Join-Path ${env:ProgramFiles(x86)} "Google\Chrome\Application\chrome.exe"),
    (Join-Path $env:ProgramFiles "Microsoft\Edge\Application\msedge.exe"),
    (Join-Path ${env:ProgramFiles(x86)} "Microsoft\Edge\Application\msedge.exe")
  )) {
    if (-not [string]::IsNullOrWhiteSpace($knownPath)) { $candidates += $knownPath }
  }
  foreach ($name in @("chrome.exe", "msedge.exe")) {
    $command = Get-Command $name -ErrorAction SilentlyContinue | Where-Object { $_.CommandType -eq "Application" } | Select-Object -First 1
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

function Get-FreeTcpPort {
  $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 0)
  $listener.Start()
  try {
    return ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
  } finally {
    $listener.Stop()
  }
}

function Get-ChildProcessIds([int]$ParentPid) {
  $seen = @{}
  $result = New-Object System.Collections.Generic.List[int]
  $queue = New-Object System.Collections.Generic.Queue[int]
  $queue.Enqueue($ParentPid)

  while ($queue.Count -gt 0) {
    $currentPid = $queue.Dequeue()
    $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$currentPid" -ErrorAction SilentlyContinue)
    foreach ($child in $children) {
      $childPid = [int]$child.ProcessId
      if (-not $seen.ContainsKey($childPid)) {
        $seen[$childPid] = $true
        $result.Add($childPid) | Out-Null
        $queue.Enqueue($childPid)
      }
    }
  }
  return @($result)
}

function Get-ProcessSnapshot([int[]]$Pids) {
  $items = @()
  foreach ($processId in @($Pids | Sort-Object -Unique)) {
    try {
      $process = Get-Process -Id $processId -ErrorAction Stop
      $items += [ordered]@{
        pid = $processId
        processName = $process.ProcessName
        workingSetMb = [math]::Round($process.WorkingSet64 / 1MB, 2)
        privateMemoryMb = [math]::Round($process.PrivateMemorySize64 / 1MB, 2)
        startTime = try { $process.StartTime.ToString("o") } catch { $null }
      }
    } catch {}
  }
  return $items
}

function Wait-CdpVersion([int]$Port, [int]$TimeoutSeconds) {
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  $lastError = ""
  while ((Get-Date) -lt $deadline) {
    try {
      $value = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/version" -TimeoutSec 2 -UseBasicParsing
      return [ordered]@{
        ready = $true
        version = $value
        error = ""
      }
    } catch {
      $lastError = $_.Exception.Message
      Start-Sleep -Milliseconds 300
    }
  }
  return [ordered]@{
    ready = $false
    version = $null
    error = $lastError
  }
}

function Stop-OwnedProcessTree([int]$RootPid, [int[]]$ObservedChildPids) {
  $allPids = @($RootPid) + @($ObservedChildPids)
  $allPids = @($allPids | Sort-Object -Unique)

  foreach ($processId in @($allPids | Sort-Object -Descending)) {
    try {
      Stop-Process -Id $processId -ErrorAction SilentlyContinue
    } catch {}
  }

  $deadline = (Get-Date).AddSeconds(8)
  while ((Get-Date) -lt $deadline) {
    $remaining = @()
    foreach ($processId in $allPids) {
      try {
        $null = Get-Process -Id $processId -ErrorAction Stop
        $remaining += $processId
      } catch {}
    }
    if ($remaining.Count -eq 0) { break }
    Start-Sleep -Milliseconds 250
  }

  foreach ($processId in $allPids) {
    try {
      Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
    } catch {}
  }

  $finalRemaining = @()
  foreach ($processId in $allPids) {
    try {
      $null = Get-Process -Id $processId -ErrorAction Stop
      $finalRemaining += $processId
    } catch {}
  }
  return [ordered]@{
    observedPids = $allPids
    remainingPids = $finalRemaining
    cleanupComplete = $finalRemaining.Count -eq 0
  }
}

function New-GateResult([string]$Id, [bool]$Passed, [string]$Evidence) {
  return [ordered]@{
    id = $Id
    status = if ($Passed) { "passed" } else { "failed" }
    evidence = $Evidence
  }
}

$projectRoot = Resolve-ProjectRoot
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null
$timestamp = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
$workRoot = Join-Path $projectRoot ".codex_tmp\m15-browser-process"
$workDir = Join-Path $workRoot $timestamp
$profileDir = Join-Path $workDir "profile"
New-Item -ItemType Directory -Force -Path $profileDir | Out-Null

$browser = Find-BrowserBinary $projectRoot $BrowserPath
$rootProcess = $null
$rootPid = $null
$port = $null
$cdp = [ordered]@{ ready = $false; version = $null; error = "browser not started" }
$prewarmSnapshot = @()
$cleanup = [ordered]@{ observedPids = @(); remainingPids = @(); cleanupComplete = $false }
$launchError = ""
$profileCleanup = [ordered]@{ attempted = $false; removed = $false; error = "" }

if (-not [string]::IsNullOrWhiteSpace($browser) -and (Test-Path $browser)) {
  $port = Get-FreeTcpPort
  $args = @(
    "--remote-debugging-port=$port",
    "--user-data-dir=$profileDir",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    "--disable-sync",
    "--disable-features=Translate,OptimizationHints,MediaRouter",
    "--headless=new",
    "about:blank"
  )

  try {
    $rootProcess = Start-Process -FilePath $browser -ArgumentList $args -PassThru -WindowStyle Hidden
    $rootPid = [int]$rootProcess.Id
    $cdp = Wait-CdpVersion $port $CdpReadyTimeoutSeconds
    $childPids = Get-ChildProcessIds $rootPid
    $prewarmSnapshot = Get-ProcessSnapshot (@($rootPid) + @($childPids))
    $cleanup = Stop-OwnedProcessTree $rootPid $childPids
  } catch {
    $launchError = $_.Exception.Message
    if ($null -ne $rootPid) {
      $childPids = Get-ChildProcessIds $rootPid
      $cleanup = Stop-OwnedProcessTree $rootPid $childPids
    }
  }
}

try {
  $profileCleanup.attempted = $true
  $resolvedWorkRoot = (Resolve-Path $workRoot).Path
  $resolvedWorkDir = (Resolve-Path $workDir).Path
  if ($resolvedWorkDir.StartsWith($resolvedWorkRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    Remove-Item -LiteralPath $resolvedWorkDir -Recurse -Force -ErrorAction Stop
    $profileCleanup.removed = -not (Test-Path $resolvedWorkDir)
  } else {
    $profileCleanup.error = "resolved temp path escaped work root"
  }
} catch {
  $profileCleanup.error = $_.Exception.Message
}

$processCount = @($prewarmSnapshot).Count
$totalWorkingSetMb = 0.0
foreach ($item in @($prewarmSnapshot)) { $totalWorkingSetMb += [double]$item.workingSetMb }
$browserPresent = -not [string]::IsNullOrWhiteSpace($browser) -and (Test-Path $browser)
$started = $null -ne $rootPid
$cdpReady = [bool]$cdp.ready
$cleanupComplete = [bool]$cleanup.cleanupComplete

$checks = @(
  (New-GateResult "browser_binary_found" $browserPresent $browser),
  (New-GateResult "browser_process_started" $started "rootPid=$rootPid"),
  (New-GateResult "cdp_json_version_ready" $cdpReady "port=$port error=$($cdp.error)"),
  (New-GateResult "process_snapshot_recorded" ($processCount -gt 0) "processCount=$processCount workingSetMb=$([math]::Round($totalWorkingSetMb, 2))"),
  (New-GateResult "owned_process_tree_cleaned" $cleanupComplete "remainingPids=$(@($cleanup.remainingPids) -join ',')"),
  (New-GateResult "temp_profile_cleanup_attempted" ([bool]$profileCleanup.attempted) "removed=$($profileCleanup.removed) error=$($profileCleanup.error)")
)

$status = if (-not $browserPresent) {
  "blocked_browser_binary_missing"
} elseif ($started -and $cdpReady -and $processCount -gt 0 -and $cleanupComplete) {
  "passed_real_browser_process_prewarm_cleanup"
} elseif ($started -and -not $cleanupComplete) {
  "failed_browser_process_cleanup"
} else {
  "failed_browser_process_prewarm"
}

$failureReason = if ($status -eq "passed_real_browser_process_prewarm_cleanup") {
  ""
} elseif ($status -eq "blocked_browser_binary_missing") {
  "no chrome.exe or msedge.exe browser binary was found"
} elseif ($status -eq "failed_browser_process_cleanup") {
  "owned browser process tree did not exit after cleanup"
} elseif (-not [string]::IsNullOrWhiteSpace($launchError)) {
  $launchError
} else {
  "browser did not reach CDP readiness or process snapshot was empty"
}

$nextAction = if ($status -eq "passed_real_browser_process_prewarm_cleanup") {
  "Keep this M15 real process proof attached; integrate the same process/RSS cleanup checks into pool acquire/release before claiming full M15 complete."
} elseif ($status -eq "blocked_browser_binary_missing") {
  "Install Chrome/Edge or pass -BrowserPath to a local CDP-capable browser binary, then rerun this gate."
} else {
  "Inspect launch/CDP/cleanup fields in this report, fix process lifecycle handling, then rerun scripts/m15_browser_process_gate.ps1."
}

$report = [ordered]@{
  schemaVersion = "m15_browser_process_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $projectRoot
  status = $status
  browserPath = $browser
  rootPid = $rootPid
  remoteDebuggingPort = $port
  cdpReady = $cdpReady
  cdpVersion = $cdp.version
  processSnapshot = $prewarmSnapshot
  processSummary = [ordered]@{
    processCount = $processCount
    totalWorkingSetMb = [math]::Round($totalWorkingSetMb, 2)
    rootPid = $rootPid
    childPids = if ($null -ne $rootPid) { @(Get-ChildProcessIds $rootPid) } else { @() }
  }
  cleanupProof = $cleanup
  tempProfile = [ordered]@{
    path = $profileDir
    cleanup = $profileCleanup
  }
  checks = $checks
  summary = [ordered]@{
    failed = @($checks | Where-Object { $_.status -ne "passed" -and $_.id -ne "temp_profile_cleanup_attempted" }).Count
    browserProcess = if ($started) { "started" } else { "missing" }
    cdpReady = $cdpReady
    processCount = $processCount
    totalWorkingSetMb = [math]::Round($totalWorkingSetMb, 2)
    cleanupComplete = $cleanupComplete
    nextAction = $nextAction
  }
  failureReason = $failureReason
  liveTruthBoundary = @(
    "This gate starts a real local browser process with a temp profile and CDP remote debugging.",
    "It only kills the process tree spawned by this script.",
    "CAPTCHA/SMS/Email credential-backed provider smoke remains separate from this local browser process proof."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("m15-browser-process-gate-{0}.json" -f $timestamp)
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding UTF8

Write-Host "M15 browser process gate report: $reportPath"
Write-Host "Status: $status"
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -eq "passed_real_browser_process_prewarm_cleanup") { exit 0 }
if ($status -eq "blocked_browser_binary_missing") { exit 2 }
exit 1
