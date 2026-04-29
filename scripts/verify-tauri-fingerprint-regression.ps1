param(
    [string]$BaseUrl = "http://127.0.0.1:19876",
    [string]$BaselineAppPath = "",
    [string]$CandidateAppPath = "",
    [string]$ProfileId = "",
    [int]$ReadyTimeoutSec = 90,
    [switch]$SkipBaseline,
    [switch]$KeepAppsRunning
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
if ([string]::IsNullOrWhiteSpace($BaselineAppPath)) {
    $BaselineAppPath = Join-Path $ProjectRoot "build\bin\personal-pilot.exe"
}
if ([string]::IsNullOrWhiteSpace($CandidateAppPath)) {
    $CandidateAppPath = Join-Path $ProjectRoot "src-tauri\target\release\antbrowser-tauri.exe"
}
$BaselineAppPath = [System.IO.Path]::GetFullPath($BaselineAppPath)
$CandidateAppPath = [System.IO.Path]::GetFullPath($CandidateAppPath)
$BaseUrl = $BaseUrl.TrimEnd("/")
$RunId = "fp-{0}-{1}" -f ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()), ([Guid]::NewGuid().ToString("N").Substring(0, 8))
$ApiKey = $env:ANTBROWSER_API_KEY
$ApiKeyHeader = if ([string]::IsNullOrWhiteSpace($env:ANTBROWSER_API_KEY_HEADER)) { "X-Ant-Api-Key" } else { $env:ANTBROWSER_API_KEY_HEADER }
$ExpectedProcessPaths = @(
    $BaselineAppPath,
    $CandidateAppPath,
    (Join-Path (Split-Path -Parent $BaselineAppPath) "antbrowser-core.exe"),
    (Join-Path (Split-Path -Parent $CandidateAppPath) "antbrowser-core.exe"),
    (Join-Path $ProjectRoot "bin\antbrowser-core.exe"),
    (Join-Path $ProjectRoot "bin\antbrowser-core-x86_64-pc-windows-msvc.exe")
) | ForEach-Object { [System.IO.Path]::GetFullPath($_) } | Select-Object -Unique
$PreviousAppRootEnv = $env:ANTBROWSER_APP_ROOT
$env:ANTBROWSER_APP_ROOT = $ProjectRoot

function Get-MatchingProcessIds {
    param([string[]]$Paths)
    $pathSet = @{}
    foreach ($path in $Paths) {
        $pathSet[[System.IO.Path]::GetFullPath($path).ToLowerInvariant()] = $true
    }
    $ids = @{}
    Get-Process | ForEach-Object {
        try {
            if ($_.Path) {
                $procPath = [System.IO.Path]::GetFullPath($_.Path).ToLowerInvariant()
                if ($pathSet.ContainsKey($procPath)) {
                    $ids[[int]$_.Id] = $true
                }
            }
        } catch {}
    }
    return $ids
}

function Stop-NewResidualProcesses {
    param(
        [string[]]$Paths,
        [hashtable]$PreExistingIds
    )
    $pathSet = @{}
    foreach ($path in $Paths) {
        $pathSet[[System.IO.Path]::GetFullPath($path).ToLowerInvariant()] = $true
    }
    Get-Process | ForEach-Object {
        try {
            if (-not $_.Path) { return }
            if ($PreExistingIds.ContainsKey([int]$_.Id)) { return }
            $procPath = [System.IO.Path]::GetFullPath($_.Path).ToLowerInvariant()
            if ($pathSet.ContainsKey($procPath)) {
                $owner = Get-CimInstance Win32_Process -Filter "ProcessId = $($_.Id)"
                $commandLine = [string]$owner.CommandLine
                if ($commandLine.IndexOf($ProjectRoot, [System.StringComparison]::OrdinalIgnoreCase) -lt 0) { return }
                Stop-Process -Id $_.Id -Force
            }
        } catch {}
    }
}
$PreExistingProcessIds = Get-MatchingProcessIds -Paths $ExpectedProcessPaths

function Invoke-AntApi {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][string]$Path,
        [object]$Body = $null,
        [int]$TimeoutSec = 20
    )
    $headers = @{}
    if (-not [string]::IsNullOrWhiteSpace($ApiKey)) {
        $headers[$ApiKeyHeader] = $ApiKey
    }
    $uri = "$BaseUrl$Path"
    if ($null -eq $Body) {
        return Invoke-RestMethod -Method $Method -Uri $uri -Headers $headers -TimeoutSec $TimeoutSec
    }
    $json = $Body | ConvertTo-Json -Depth 20
    return Invoke-RestMethod -Method $Method -Uri $uri -Headers $headers -ContentType "application/json" -Body $json -TimeoutSec $TimeoutSec
}

function Test-AntHealth {
    try {
        $health = Invoke-AntApi -Method "GET" -Path "/api/health" -TimeoutSec 3
        return [bool]$health.ok
    } catch {
        return $false
    }
}

function Wait-AntHealth {
    param([int]$TimeoutSec)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        if (Test-AntHealth) { return }
        Start-Sleep -Milliseconds 500
    }
    throw "LaunchServer health check timed out at $BaseUrl/api/health"
}

function Get-AntProfiles {
    $resp = Invoke-AntApi -Method "GET" -Path "/api/profiles" -TimeoutSec 20
    if (-not $resp.ok) { throw "GET /api/profiles returned ok=false" }
    return @($resp.items)
}

function Wait-ProfileReady {
    param([Parameter(Mandatory = $true)][string]$TargetProfileId)
    $deadline = (Get-Date).AddSeconds($ReadyTimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $profile = Get-AntProfiles | Where-Object { $_.profileId -eq $TargetProfileId } | Select-Object -First 1
        if ($null -ne $profile -and $profile.running -and $profile.debugReady -and [int]$profile.debugPort -gt 0) {
            return $profile
        }
        Start-Sleep -Milliseconds 750
    }
    throw "Profile $TargetProfileId did not become Running+DebugReady within $ReadyTimeoutSec seconds"
}

function New-TemporaryProfile {
    $relativeUserData = "verification/$RunId/profile"
    $resp = Invoke-AntApi -Method "POST" -Path "/api/profiles" -Body @{
        profile = @{
            profileName = "fingerprint-$RunId"
            userDataDir = $relativeUserData
            launchArgs = @("--window-size=1200,800")
            tags = @("antbrowser-fingerprint-verify", $RunId)
            keywords = @($RunId)
        }
    } -TimeoutSec 30
    if (-not $resp.ok -or [string]::IsNullOrWhiteSpace($resp.profileId)) {
        throw "Failed to create temporary fingerprint profile"
    }
    return [string]$resp.profileId
}

function Stop-Profile {
    param([Parameter(Mandatory = $true)][string]$TargetProfileId)
    try {
        [void](Invoke-AntApi -Method "POST" -Path "/api/instances/stop" -Body @{ profileId = $TargetProfileId } -TimeoutSec 30)
    } catch {}
}

function Remove-Profile {
    param([Parameter(Mandatory = $true)][string]$TargetProfileId)
    try {
        [void](Invoke-AntApi -Method "DELETE" -Path "/api/profiles/$TargetProfileId" -TimeoutSec 30)
    } catch {
        Write-Warning "Failed to delete temporary profile ${TargetProfileId}: $($_.Exception.Message)"
    }
}

function Remove-TemporaryUserData {
    $verificationRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot "data\verification")).TrimEnd('\')
    $target = [System.IO.Path]::GetFullPath((Join-Path $verificationRoot $RunId)).TrimEnd('\')
    if ((Test-Path -LiteralPath $target) -and $target.StartsWith($verificationRoot + "\", [System.StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $target -Recurse -Force
    }
}

function Stop-AppProcess {
    param([object]$Process)
    if ($KeepAppsRunning -or $null -eq $Process) { return }
    try {
        if (-not $Process.HasExited) {
            [void]$Process.CloseMainWindow()
            if (-not $Process.WaitForExit(5000)) {
                Stop-Process -Id $Process.Id -Force
            }
        }
    } catch {
        Write-Warning "Failed to close app process $($Process.Id): $($_.Exception.Message)"
    }
    Stop-NewResidualProcesses -Paths $ExpectedProcessPaths -PreExistingIds $PreExistingProcessIds
}

function Get-CommandLine {
    param([int]$TargetProcessId)
    try {
        $proc = Get-CimInstance Win32_Process -Filter "ProcessId = $TargetProcessId"
        return [string]$proc.CommandLine
    } catch {
        return ""
    }
}

function Collect-FingerprintRun {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string]$AppPath,
        [Parameter(Mandatory = $true)][string]$TargetProfileId
    )
    if (-not (Test-Path -LiteralPath $AppPath)) {
        throw "$Label app executable not found: $AppPath"
    }
    if (Test-AntHealth) {
        throw "LaunchServer is already running before $Label run; stop the app first to avoid mixing processes."
    }

    $appProcess = Start-Process -FilePath $AppPath -WorkingDirectory $ProjectRoot -PassThru
    try {
        Wait-AntHealth -TimeoutSec $ReadyTimeoutSec
        [void](Invoke-AntApi -Method "POST" -Path "/api/launch" -Body @{
            profileId = $TargetProfileId
            startUrls = @("about:blank")
            skipDefaultStartUrls = $true
        } -TimeoutSec 60)
        $profile = Wait-ProfileReady -TargetProfileId $TargetProfileId
        $verificationRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot "data\verification"))
        $navRoot = [System.IO.Path]::GetFullPath((Join-Path $verificationRoot $RunId))
        New-Item -ItemType Directory -Path $navRoot -Force | Out-Null
        $navPagePath = Join-Path $navRoot "fingerprint.html"
        Set-Content -LiteralPath $navPagePath -Value "<!doctype html><title>antbrowser-fingerprint</title><body>antbrowser-fingerprint-$RunId</body>" -Encoding UTF8
        $navURL = "file:///" + ([System.IO.Path]::GetFullPath($navPagePath) -replace "\\", "/")
        [void](Invoke-AntApi -Method "POST" -Path "/api/workbench/navigate" -Body @{
            profileId = $TargetProfileId
            url = $navURL
        } -TimeoutSec 30)
        Start-Sleep -Milliseconds 750
        $fingerprintResp = Invoke-AntApi -Method "POST" -Path "/api/workbench/fingerprint" -Body @{ profileId = $TargetProfileId } -TimeoutSec 30
        if (-not $fingerprintResp.ok) {
            throw "$Label fingerprint capture returned ok=false"
        }
        $shot = Invoke-AntApi -Method "POST" -Path "/api/workbench/screenshot" -Body @{ profileId = $TargetProfileId } -TimeoutSec 30
        $browserCommandLine = Get-CommandLine -TargetProcessId ([int]$profile.pid)
        Stop-Profile -TargetProfileId $TargetProfileId
        return [pscustomobject]@{
            label = $Label
            appPath = $AppPath
            profileId = $TargetProfileId
            profileName = $profile.profileName
            userDataDir = $profile.userDataDir
            proxyConfig = $profile.proxyConfig
            fingerprintArgs = @($profile.fingerprintArgs)
            launchArgs = @($profile.launchArgs)
            debugPort = [int]$profile.debugPort
            pid = [int]$profile.pid
            commandLine = $browserCommandLine
            fingerprint = $fingerprintResp.fingerprint
            screenshotLength = if ($shot.ok -and $shot.screenshot) { [string]$shot.screenshot.Length } else { "0" }
        }
    } finally {
        Stop-Profile -TargetProfileId $TargetProfileId
        Stop-AppProcess -Process $appProcess
        Start-Sleep -Milliseconds 500
    }
}

function StableJson {
    param($Value)
    return ($Value | ConvertTo-Json -Depth 20 -Compress)
}

function Compare-FingerprintRuns {
    param($Baseline, $Candidate)
    $mismatches = New-Object System.Collections.Generic.List[string]
    foreach ($field in @("userDataDir", "proxyConfig", "fingerprintArgs", "launchArgs")) {
        if ((StableJson $Baseline.$field) -ne (StableJson $Candidate.$field)) {
            $mismatches.Add($field)
        }
    }
    foreach ($field in @(
        "userAgent", "platform", "hardwareConcurrency", "deviceMemory", "colorDepth",
        "pixelDepth", "screenWidth", "screenHeight", "availWidth", "availHeight",
        "devicePixelRatio", "maxTouchPoints", "vendor", "timezone", "language",
        "languages", "canvasHash", "webglVendor", "webglRenderer", "fontHash"
    )) {
        if ((StableJson $Baseline.fingerprint.$field) -ne (StableJson $Candidate.fingerprint.$field)) {
            $mismatches.Add("fingerprint.$field")
        }
    }
    return @($mismatches)
}

$createdProfile = $false
$targetProfileId = $ProfileId.Trim()
$baseline = $null
$candidate = $null
$failure = $null

try {
    if ([string]::IsNullOrWhiteSpace($targetProfileId)) {
        if (Test-AntHealth) {
            throw "LaunchServer is already running; stop the app before temporary profile setup."
        }
        if (-not (Test-Path -LiteralPath $CandidateAppPath)) {
            throw "Candidate app executable not found: $CandidateAppPath"
        }
        $setupProcess = Start-Process -FilePath $CandidateAppPath -WorkingDirectory $ProjectRoot -PassThru
        try {
            Wait-AntHealth -TimeoutSec $ReadyTimeoutSec
            $targetProfileId = New-TemporaryProfile
            $createdProfile = $true
        } finally {
            Stop-AppProcess -Process $setupProcess
            Start-Sleep -Milliseconds 500
        }
    }

    if (-not $SkipBaseline) {
        if (Test-Path -LiteralPath $BaselineAppPath) {
            $baseline = Collect-FingerprintRun -Label "baseline" -AppPath $BaselineAppPath -TargetProfileId $targetProfileId
        } else {
            Write-Warning "Baseline app not found; candidate-only verification will run."
        }
    }
    $candidate = Collect-FingerprintRun -Label "candidate" -AppPath $CandidateAppPath -TargetProfileId $targetProfileId

    $mismatches = @()
    if ($null -ne $baseline) {
        $mismatches = Compare-FingerprintRuns -Baseline $baseline -Candidate $candidate
    }
    [pscustomobject]@{
        ok = ($mismatches.Count -eq 0)
        compared = ($null -ne $baseline)
        runId = $RunId
        profileId = $targetProfileId
        mismatches = $mismatches
        baseline = $baseline
        candidate = $candidate
    } | ConvertTo-Json -Depth 30
    if ($mismatches.Count -gt 0) {
        exit 1
    }
} catch {
    $failure = $_
    Write-Error $failure
    exit 1
} finally {
    if ($createdProfile -and -not [string]::IsNullOrWhiteSpace($targetProfileId)) {
        if (-not (Test-AntHealth)) {
            $cleanupProcess = Start-Process -FilePath $CandidateAppPath -WorkingDirectory $ProjectRoot -PassThru
            try {
                Wait-AntHealth -TimeoutSec $ReadyTimeoutSec
                Stop-Profile -TargetProfileId $targetProfileId
                Remove-Profile -TargetProfileId $targetProfileId
            } finally {
                Stop-AppProcess -Process $cleanupProcess
            }
        } else {
            Stop-Profile -TargetProfileId $targetProfileId
            Remove-Profile -TargetProfileId $targetProfileId
        }
        Remove-TemporaryUserData
    }
    Remove-TemporaryUserData
    $env:ANTBROWSER_APP_ROOT = $PreviousAppRootEnv
}
