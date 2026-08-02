param(
  [string]$ProjectRoot = "",
  [string]$OutputDir = "data/reports/transport-binary-smoke",
  [string]$WorkDir = ".codex_tmp/transport-binary-smoke"
)

$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
  if (-not [string]::IsNullOrWhiteSpace($ProjectRoot)) {
    return (Resolve-Path $ProjectRoot).Path
  }
  return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Get-FreeTcpPort {
  $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 0)
  $listener.Start()
  try {
    return $listener.LocalEndpoint.Port
  } finally {
    $listener.Stop()
  }
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
    exitCode = $exitCode
    timedOut = (-not $finished)
    stdout = $stdout.Trim()
    stderr = $stderr.Trim()
    durationMs = [int]((Get-Date) - $startedAt).TotalMilliseconds
  }
}

function New-XrayConfig([int]$Port) {
  return [ordered]@{
    log = [ordered]@{ loglevel = "warning" }
    inbounds = @(
      [ordered]@{
        tag = "socks-in"
        listen = "127.0.0.1"
        port = $Port
        protocol = "socks"
        settings = [ordered]@{
          auth = "noauth"
          udp = $false
        }
      }
    )
    outbounds = @(
      [ordered]@{
        tag = "direct"
        protocol = "freedom"
        settings = [ordered]@{}
      }
    )
  }
}

function New-SingBoxConfig([int]$Port) {
  return [ordered]@{
    log = [ordered]@{ disabled = $true }
    inbounds = @(
      [ordered]@{
        type = "socks"
        tag = "socks-in"
        listen = "127.0.0.1"
        listen_port = $Port
      }
    )
    outbounds = @(
      [ordered]@{
        type = "direct"
        tag = "direct"
      }
    )
    route = [ordered]@{ final = "direct" }
  }
}

function New-BinaryResult([string]$Id, [string]$Path, [object]$Version, [object]$Check, [string]$ConfigPath) {
  $present = Test-Path $Path
  $versionOk = $present -and $Version.exitCode -eq 0
  $checkOk = $present -and $Check.exitCode -eq 0
  return [ordered]@{
    id = $Id
    binaryPath = $Path
    binaryPresent = $present
    versionStatus = if ($versionOk) { "passed" } elseif ($present) { "failed" } else { "missing" }
    configCheckStatus = if ($checkOk) { "passed" } elseif ($present) { "failed" } else { "missing" }
    configPath = $ConfigPath
    version = $Version
    configCheck = $Check
  }
}

function Write-JsonNoBom([string]$Path, [object]$Value, [int]$Depth = 8) {
  $json = $Value | ConvertTo-Json -Depth $Depth
  $encoding = [System.Text.UTF8Encoding]::new($false)
  [System.IO.File]::WriteAllText($Path, $json, $encoding)
}

$root = Resolve-ProjectRoot
$timestamp = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
$absoluteWorkDir = Join-Path $root (Join-Path $WorkDir $timestamp)
New-Item -ItemType Directory -Force -Path $absoluteWorkDir | Out-Null

$xrayPath = Join-Path $root "bin\xray.exe"
$singBoxPath = Join-Path $root "bin\sing-box.exe"
$xrayConfigPath = Join-Path $absoluteWorkDir "xray-direct-socks.json"
$singBoxConfigPath = Join-Path $absoluteWorkDir "sing-box-direct-socks.json"

$xrayPort = Get-FreeTcpPort
$singBoxPort = Get-FreeTcpPort
Write-JsonNoBom $xrayConfigPath (New-XrayConfig $xrayPort) 8
Write-JsonNoBom $singBoxConfigPath (New-SingBoxConfig $singBoxPort) 8

$xrayVersion = if (Test-Path $xrayPath) { Invoke-Binary $xrayPath @("version") } else { [ordered]@{ command = "$xrayPath version"; exitCode = -1; timedOut = $false; stdout = ""; stderr = "binary missing"; durationMs = 0 } }
$xrayCheck = if (Test-Path $xrayPath) { Invoke-Binary $xrayPath @("run", "-test", "-config=$xrayConfigPath") } else { [ordered]@{ command = "$xrayPath run -test -config=$xrayConfigPath"; exitCode = -1; timedOut = $false; stdout = ""; stderr = "binary missing"; durationMs = 0 } }
$singBoxVersion = if (Test-Path $singBoxPath) { Invoke-Binary $singBoxPath @("version") } else { [ordered]@{ command = "$singBoxPath version"; exitCode = -1; timedOut = $false; stdout = ""; stderr = "binary missing"; durationMs = 0 } }
$singBoxCheck = if (Test-Path $singBoxPath) { Invoke-Binary $singBoxPath @("check", "-c", $singBoxConfigPath) } else { [ordered]@{ command = "$singBoxPath check -c $singBoxConfigPath"; exitCode = -1; timedOut = $false; stdout = ""; stderr = "binary missing"; durationMs = 0 } }

$items = @(
  (New-BinaryResult "xray" $xrayPath $xrayVersion $xrayCheck $xrayConfigPath),
  (New-BinaryResult "sing-box" $singBoxPath $singBoxVersion $singBoxCheck $singBoxConfigPath)
)
$failed = @($items | Where-Object { $_.binaryPresent -ne $true -or $_.versionStatus -ne "passed" -or $_.configCheckStatus -ne "passed" })
$status = if ($failed.Count -eq 0) { "passed" } else { "failed" }
$failureReason = if ($status -eq "passed") { "" } else { "one or more transport binaries failed presence/version/config-check validation" }

$report = [ordered]@{
  schemaVersion = "transport_binary_smoke_v1"
  generatedAt = (Get-Date).ToString("o")
  projectRoot = $root
  status = $status
  failureReason = $failureReason
  validationScope = "local_binary_version_and_direct_socks_config_check"
  egressScope = "direct_outbound_config_only_no_remote_proxy_node"
  workDir = $absoluteWorkDir
  items = $items
  notes = @(
    "Xray is validated with: xray run -test -config=<generated direct socks config>.",
    "SingBox is validated with: sing-box check -c <generated direct socks config>.",
    "This proves local binaries accept generated direct outbound configs; it is not remote proxy egress evidence."
  )
}

$absoluteOutputDir = Join-Path $root $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null
$reportPath = Join-Path $absoluteOutputDir ("transport-binary-smoke-{0}.json" -f $timestamp)
$report | ConvertTo-Json -Depth 10 | Set-Content -Path $reportPath -Encoding UTF8

Write-Host "Transport binary smoke report: $reportPath"
Write-Host "Status: $status"
foreach ($item in $items) {
  Write-Host "$($item.id): version=$($item.versionStatus), config=$($item.configCheckStatus), binary=$($item.binaryPresent)"
}
if ($failureReason) { Write-Host "Failure reason: $failureReason" }

if ($status -ne "passed") { exit 1 }
