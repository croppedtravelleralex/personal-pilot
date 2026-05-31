param(
  [string]$ProjectRoot = "",
  [string]$BinaryPath = "",
  [string]$ConfigPath = "",
  [string]$ProfileDir = "",
  [string]$Url = "https://example.com",
  [string]$Action = "get_title",
  [int]$TimeoutSeconds = 30,
  [string[]]$ExtraArgs = @(),
  [string]$ProxyServer = "",
  [string]$OutputDir = "data/reports/camoufox-binary-task",
  [switch]$UseCdpRunner,
  [switch]$AllowBlocked
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  if (-not [string]::IsNullOrWhiteSpace($ProjectRoot)) {
    return (Resolve-Path $ProjectRoot).Path
  }
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Write-Report([string]$Path, [object]$Report) {
  $dir = Split-Path -Parent $Path
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  Write-JsonNoBom $Path $Report 10
}

function Write-JsonNoBom([string]$Path, [object]$Value, [int]$Depth = 8) {
  $json = $Value | ConvertTo-Json -Depth $Depth
  $encoding = [System.Text.UTF8Encoding]::new($false)
  [System.IO.File]::WriteAllText($Path, $json, $encoding)
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

function Find-CamoufoxBinary([string]$Root) {
  $candidates = @()
  $command = Get-Command camoufox* -ErrorAction SilentlyContinue | Where-Object { $_.CommandType -eq "Application" } | Select-Object -First 1
  if ($null -ne $command -and -not [string]::IsNullOrWhiteSpace($command.Source)) {
    $candidates += $command.Source
  }
  $candidates += @(Get-ChildItem -Path (Join-Path $Root "bin") -Recurse -Filter "*camoufox*.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
  $localCamoufoxRoot = Join-Path $env:LOCALAPPDATA "camoufox"
  if ($env:LOCALAPPDATA -and (Test-Path $localCamoufoxRoot)) {
    $candidates += @(Get-ChildItem -Path $localCamoufoxRoot -Recurse -Filter "*camoufox*.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
  }
  foreach ($candidate in $candidates) {
    if (-not [string]::IsNullOrWhiteSpace($candidate) -and (Test-Path $candidate)) {
      return (Resolve-Path $candidate).Path
    }
  }
  return ""
}

function Invoke-Binary([string]$FilePath, [string[]]$Arguments, [int]$TimeoutSeconds = 30) {
  $psi = [System.Diagnostics.ProcessStartInfo]::new()
  $psi.FileName = $FilePath
  $psi.Arguments = [string]::Join(' ', @($Arguments | ForEach-Object { '"' + ($_ -replace '"', '\"') + '"' }))
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.UseShellExecute = $false
  $psi.CreateNoWindow = $true
  $process = [System.Diagnostics.Process]::new()
  $process.StartInfo = $psi
  $startedAt = Get-Date
  [void]$process.Start()
  $stdoutTask = $process.StandardOutput.ReadToEndAsync()
  $stderrTask = $process.StandardError.ReadToEndAsync()
  $finished = $process.WaitForExit($TimeoutSeconds * 1000)
  if (-not $finished) {
    try { $process.Kill($true) } catch { try { $process.Kill() } catch {} }
  }
  $stdout = $stdoutTask.GetAwaiter().GetResult()
  $stderr = $stderrTask.GetAwaiter().GetResult()
  $exitCode = if ($finished) { $process.ExitCode } else { -1000 }
  return [ordered]@{
    command = "$FilePath $($Arguments -join ' ')"
    pid = $process.Id
    exitCode = $exitCode
    timedOut = (-not $finished)
    stdout = $stdout.Trim()
    stderr = $stderr.Trim()
    durationMs = [int]((Get-Date) - $startedAt).TotalMilliseconds
  }
}

$root = Resolve-ProjectRoot
$timestamp = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
$absoluteOutputDir = Join-Path $root $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null
$reportPath = Join-Path $absoluteOutputDir ("camoufox-binary-task-smoke-{0}.json" -f $timestamp)

$effectiveConfigPath = $ConfigPath
if ([string]::IsNullOrWhiteSpace($effectiveConfigPath)) {
  $effectiveConfigPath = $env:PERSONA_PILOT_CAMOUFOX_CONFIG
}

$effectiveBinaryPath = $BinaryPath
if ([string]::IsNullOrWhiteSpace($effectiveBinaryPath)) {
  $effectiveBinaryPath = Get-BinaryFromConfig $effectiveConfigPath
}
if ([string]::IsNullOrWhiteSpace($effectiveBinaryPath)) {
  $effectiveBinaryPath = Find-CamoufoxBinary $root
}

if ([string]::IsNullOrWhiteSpace($effectiveBinaryPath) -or -not (Test-Path $effectiveBinaryPath)) {
  $report = [ordered]@{
    schemaVersion = "camoufox_binary_task_smoke_v1"
    generatedAt = (Get-Date).ToString("o")
    projectRoot = $root
    status = "blocked_missing_camoufox_binary_or_config"
    failureReason = "No Camoufox binary was provided, configured, found on PATH, or found under bin/."
    requested = [ordered]@{ url = $Url; action = $Action; timeoutSeconds = $TimeoutSeconds }
    configPath = $effectiveConfigPath
    binaryPath = $effectiveBinaryPath
    evidence = [ordered]@{
      envConfig = $env:PERSONA_PILOT_CAMOUFOX_CONFIG
      envEnabled = $env:PERSONA_PILOT_CAMOUFOX_ENABLED
      pathSearch = "Get-Command camoufox* and bin/**/*camoufox*.exe"
    }
    notes = @(
      "Real Camoufox page-open evidence requires a local Camoufox binary path.",
      "This blocked report is intentional; no fake browser success is recorded."
    )
  }
  Write-Report $reportPath $report
  Write-Host "Camoufox binary task smoke report: $reportPath"
  Write-Host "Status: blocked_missing_camoufox_binary_or_config"
  if ($AllowBlocked) { exit 0 }
  exit 2
}

$workDir = Join-Path $root (Join-Path ".codex_tmp\camoufox-binary-task" $timestamp)
New-Item -ItemType Directory -Force -Path $workDir | Out-Null
if ([string]::IsNullOrWhiteSpace($ProfileDir)) {
  $ProfileDir = Join-Path $workDir "profile"
}
New-Item -ItemType Directory -Force -Path $ProfileDir | Out-Null

if (-not $UseCdpRunner) {
  $pagePath = Join-Path $workDir "camoufox-smoke-page.html"
  $screenshotPath = Join-Path $workDir "camoufox-smoke.png"
  $html = @"
<!doctype html>
<html>
<head><meta charset="utf-8"><title>PersonaPilot Camoufox Smoke</title></head>
<body><h1>PersonaPilot Camoufox Smoke</h1><p>timestamp=$timestamp</p></body>
</html>
"@
  [System.IO.File]::WriteAllText($pagePath, $html, [System.Text.UTF8Encoding]::new($false))
  $fileUrl = ([System.Uri]::new((Resolve-Path $pagePath).Path)).AbsoluteUri
  $actualUrl = if ([string]::IsNullOrWhiteSpace($Url)) { $fileUrl } else { $Url }
  $version = Invoke-Binary (Resolve-Path $effectiveBinaryPath).Path @("--version") 10
  $screenshot = Invoke-Binary (Resolve-Path $effectiveBinaryPath).Path @(
    "--headless",
    "--new-instance",
    "--profile", $ProfileDir,
    "--window-size", "1200,800",
    "--screenshot", $screenshotPath,
    $actualUrl
  ) $TimeoutSeconds
  $screenshotPresent = Test-Path $screenshotPath
  $screenshotSize = if ($screenshotPresent) { (Get-Item $screenshotPath).Length } else { 0 }
  $status = if ($version.exitCode -eq 0 -and $screenshot.exitCode -eq 0 -and $screenshotPresent -and $screenshotSize -gt 0) { "passed" } else { "failed" }
  $report = [ordered]@{
    schemaVersion = "camoufox_binary_page_open_smoke_v1"
    generatedAt = (Get-Date).ToString("o")
    projectRoot = $root
    status = $status
    validationScope = "real_camoufox_binary_headless_screenshot_page_open"
    binaryPath = (Resolve-Path $effectiveBinaryPath).Path
    profileDir = $ProfileDir
    requestedUrl = $Url
    fallbackLocalUrl = $fileUrl
    actualUrl = $actualUrl
    action = "headless_screenshot_open_page"
    version = $version
    run = $screenshot
    screenshotPath = $screenshotPath
    screenshotPresent = $screenshotPresent
    screenshotSizeBytes = $screenshotSize
    cdpBoundary = "This Camoufox binary exposes Firefox/Juggler/BiDi style runtime; Chromium /json/version CDP attach is not claimed by this smoke."
    notes = @(
      "This is real Camoufox binary page-open evidence through Firefox-compatible headless screenshot.",
      "It proves binary launch/open/render/output/exit for a local page; it does not prove Chromium CDP task execution."
    )
  }
  Write-Report $reportPath $report
  Write-Host "Camoufox binary task smoke report: $reportPath"
  Write-Host "Status: $status"
  Write-Host "Screenshot: $screenshotPath ($screenshotSize bytes)"
  if ($status -ne "passed") { exit 1 }
  exit 0
}

if ([string]::IsNullOrWhiteSpace($effectiveConfigPath) -or -not (Test-Path $effectiveConfigPath)) {
  $effectiveConfigPath = Join-Path $workDir "camoufox-config.json"
  $config = [ordered]@{
    binary_path = (Resolve-Path $effectiveBinaryPath).Path
    profile_dir = $ProfileDir
    extra_args = @($ExtraArgs)
    proxy_server = if ([string]::IsNullOrWhiteSpace($ProxyServer)) { $null } else { $ProxyServer }
  }
  Write-JsonNoBom $effectiveConfigPath $config 6
}

$previousEnabled = $env:PERSONA_PILOT_CAMOUFOX_ENABLED
$previousConfig = $env:PERSONA_PILOT_CAMOUFOX_CONFIG
$env:PERSONA_PILOT_CAMOUFOX_ENABLED = "true"
$env:PERSONA_PILOT_CAMOUFOX_CONFIG = $effectiveConfigPath

Push-Location $root
try {
  & cargo run --quiet --bin camoufox_task_smoke -- --url $Url --action $Action --timeout-seconds $TimeoutSeconds --report $reportPath
  $exitCode = $LASTEXITCODE
} finally {
  Pop-Location
  $env:PERSONA_PILOT_CAMOUFOX_ENABLED = $previousEnabled
  $env:PERSONA_PILOT_CAMOUFOX_CONFIG = $previousConfig
}

if ($exitCode -ne 0) {
  if (-not (Test-Path $reportPath)) {
    $report = [ordered]@{
      schemaVersion = "camoufox_binary_task_smoke_v1"
      generatedAt = (Get-Date).ToString("o")
      projectRoot = $root
      status = "failed"
      failureReason = "cargo camoufox_task_smoke exited with code $exitCode before writing a report"
      configPath = $effectiveConfigPath
      binaryPath = $effectiveBinaryPath
    }
    Write-Report $reportPath $report
  }
  Write-Host "Camoufox binary task smoke report: $reportPath"
  Write-Host "Status: failed"
  exit $exitCode
}

Write-Host "Camoufox binary task smoke report: $reportPath"
Write-Host "Status: passed"
