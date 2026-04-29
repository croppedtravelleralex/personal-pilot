param(
    [string]$BaseUrl = "http://127.0.0.1:19876",
    [string]$AppPath = "",
    [string[]]$ProfileIds = @(),
    [int]$ReadyTimeoutSec = 60,
    [switch]$StopPreExisting,
    [switch]$KeepAppRunning
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$ProjectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
if ([string]::IsNullOrWhiteSpace($AppPath)) {
    $AppPath = Join-Path $ProjectRoot "build\bin\personal-pilot.exe"
}
$AppPath = [System.IO.Path]::GetFullPath($AppPath)
$BaseUrl = $BaseUrl.TrimEnd("/")
$ApiKey = $env:ANTBROWSER_API_KEY
$ApiKeyHeader = if ([string]::IsNullOrWhiteSpace($env:ANTBROWSER_API_KEY_HEADER)) { "X-Ant-Api-Key" } else { $env:ANTBROWSER_API_KEY_HEADER }
$RunId = "verify-{0}-{1}" -f ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()), ([Guid]::NewGuid().ToString("N").Substring(0, 8))
$VerificationRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot "data\verification"))
$TempUserDataRoot = [System.IO.Path]::GetFullPath((Join-Path $VerificationRoot $RunId))
$ExpectedProcessPaths = @(
    $AppPath,
    (Join-Path (Split-Path -Parent $AppPath) "antbrowser-core.exe"),
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
        [hashtable]$PreExistingIds,
        [int[]]$RootProcessIds = @()
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
        } catch {
            Write-Warning "Failed to stop residual process $($_.Id): $($_.Exception.Message)"
        }
    }
}

function Get-DescendantProcessIds {
    param([int[]]$RootProcessIds = @())

    $ids = @{}
    foreach ($id in $RootProcessIds) {
        if ($id -gt 0) {
            $ids[[int]$id] = $true
        }
    }
    if ($ids.Count -eq 0) {
        return $ids
    }

    $changed = $true
    while ($changed) {
        $changed = $false
        Get-CimInstance Win32_Process | ForEach-Object {
            try {
                $pid = [int]$_.ProcessId
                $parent = [int]$_.ParentProcessId
                if ($ids.ContainsKey($parent) -and -not $ids.ContainsKey($pid)) {
                    $ids[$pid] = $true
                    $changed = $true
                }
            } catch {}
        }
    }
    return $ids
}

function Stop-VerificationBrowserProcesses {
    $tokens = @($RunId, $TempUserDataRoot) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    if ($tokens.Count -eq 0) {
        return
    }
    Get-CimInstance Win32_Process | ForEach-Object {
        try {
            $commandLine = [string]$_.CommandLine
            if ([string]::IsNullOrWhiteSpace($commandLine)) { return }
            foreach ($token in $tokens) {
                if ($commandLine.IndexOf($token, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                    Stop-Process -Id ([int]$_.ProcessId) -Force
                    break
                }
            }
        } catch {
            Write-Warning "Failed to stop verification browser process $($_.ProcessId): $($_.Exception.Message)"
        }
    }
}

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

    $json = $Body | ConvertTo-Json -Depth 12
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
        if (Test-AntHealth) {
            return
        }
        Start-Sleep -Milliseconds 500
    }
    throw "LaunchServer health check timed out at $BaseUrl/api/health"
}

function Get-AntProfiles {
    $resp = Invoke-AntApi -Method "GET" -Path "/api/profiles"
    if (-not $resp.ok) {
        throw "GET /api/profiles returned ok=false"
    }
    return @($resp.items)
}

function Wait-ProfileReady {
    param(
        [Parameter(Mandatory = $true)][string]$ProfileId,
        [int]$TimeoutSec
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $profiles = Get-AntProfiles
        $profile = $profiles | Where-Object { $_.profileId -eq $ProfileId } | Select-Object -First 1
        if ($null -ne $profile -and $profile.running -and $profile.debugReady -and [int]$profile.debugPort -gt 0) {
            return $profile
        }
        Start-Sleep -Milliseconds 750
    }
    throw "Profile $ProfileId did not become Running+DebugReady within $TimeoutSec seconds"
}

function Select-TwoProfiles {
    param([object[]]$Profiles)

    if ($ProfileIds.Count -gt 0) {
        if ($ProfileIds.Count -lt 2) {
            throw "-ProfileIds requires at least 2 ids"
        }
        return @($ProfileIds[0], $ProfileIds[1])
    }

    return @()
}

function New-VerificationProfile {
    param([Parameter(Mandatory = $true)][int]$Index)

    $relativeUserData = "verification/$RunId/profile-$Index"
    $resp = Invoke-AntApi -Method "POST" -Path "/api/profiles" -Body @{
        profile = @{
            profileName = "verify-$RunId-$Index"
            userDataDir = $relativeUserData
            launchArgs = @("--window-size=1200,800")
            tags = @("antbrowser-verify", $RunId)
            keywords = @($RunId)
        }
    } -TimeoutSec 30
    if (-not $resp.ok -or [string]::IsNullOrWhiteSpace($resp.profileId)) {
        throw "Failed to create temporary verification profile $Index"
    }
    return [pscustomobject]@{
        ProfileId = [string]$resp.profileId
        UserDataDir = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot $relativeUserData))
    }
}

function Remove-VerificationProfile {
    param([Parameter(Mandatory = $true)][string]$ProfileId)

    try {
        [void](Invoke-AntApi -Method "DELETE" -Path "/api/profiles/$ProfileId" -TimeoutSec 30)
    } catch {
        Write-Warning "Cleanup delete profile failed for ${ProfileId}: $($_.Exception.Message)"
    }
}

function Remove-VerificationUserData {
    if (-not (Test-Path -LiteralPath $TempUserDataRoot)) {
        return
    }
    $safeRoot = [System.IO.Path]::GetFullPath($VerificationRoot).TrimEnd('\')
    $target = [System.IO.Path]::GetFullPath($TempUserDataRoot).TrimEnd('\')
    if (-not $target.StartsWith($safeRoot + "\", [System.StringComparison]::OrdinalIgnoreCase)) {
        Write-Warning "Skip unsafe verification cleanup path: $target"
        return
    }
    Remove-Item -LiteralPath $target -Recurse -Force
}

function Stop-SelectedProfile {
    param(
        [Parameter(Mandatory = $true)][string]$ProfileId,
        [hashtable]$RunningBefore,
        [System.Collections.Generic.HashSet[string]]$Stopped
    )

    if ($Stopped.Contains($ProfileId)) {
        return $true
    }
    if (-not $StopPreExisting -and $RunningBefore.ContainsKey($ProfileId) -and $RunningBefore[$ProfileId]) {
        Write-Warning "Skip stopping pre-existing running profile $ProfileId. Re-run with -StopPreExisting to stop it."
        return $false
    }
    try {
        [void](Invoke-AntApi -Method "POST" -Path "/api/instances/stop" -Body @{ profileId = $ProfileId } -TimeoutSec 30)
        [void]$Stopped.Add($ProfileId)
        return $true
    } catch {
        Write-Warning "Cleanup stop failed for ${ProfileId}: $($_.Exception.Message)"
        return $false
    }
}

$startedApp = $false
$appProcess = $null
$selected = @()
$temporaryProfiles = @()
$runningBefore = @{}
$stopped = [System.Collections.Generic.HashSet[string]]::new()
$failure = $null
$summary = $null
$preExistingProcessIds = Get-MatchingProcessIds -Paths $ExpectedProcessPaths

try {
    if (-not (Test-AntHealth)) {
        if (-not (Test-Path -LiteralPath $AppPath)) {
            throw "App executable not found: $AppPath. Build release first."
        }
        $appProcess = Start-Process -FilePath $AppPath -WorkingDirectory $ProjectRoot -PassThru
        $startedApp = $true
    }

    Wait-AntHealth -TimeoutSec $ReadyTimeoutSec

    $profiles = Get-AntProfiles
    $selected = Select-TwoProfiles -Profiles $profiles
    if ($selected.Count -lt 2) {
        $temporaryProfiles = @(
            New-VerificationProfile -Index 1
            New-VerificationProfile -Index 2
        )
        $selected = @($temporaryProfiles[0].ProfileId, $temporaryProfiles[1].ProfileId)
        $profiles = Get-AntProfiles
    }
    foreach ($id in $selected) {
        $profile = $profiles | Where-Object { $_.profileId -eq $id } | Select-Object -First 1
        if ($null -eq $profile) {
            throw "Selected profile not found: $id"
        }
        $runningBefore[$id] = [bool]$profile.running
        if (-not $StopPreExisting -and $runningBefore[$id]) {
            throw "Selected profile $id is already running. Re-run with -StopPreExisting or omit -ProfileIds to use temporary verification profiles."
        }
    }

    foreach ($id in $selected) {
        [void](Invoke-AntApi -Method "POST" -Path "/api/launch" -Body @{
            profileId = $id
            startUrls = @("about:blank")
            skipDefaultStartUrls = $true
        } -TimeoutSec 60)
        [void](Wait-ProfileReady -ProfileId $id -TimeoutSec $ReadyTimeoutSec)
    }

    New-Item -ItemType Directory -Path $TempUserDataRoot -Force | Out-Null
    $navPagePath = Join-Path $TempUserDataRoot "workbench.html"
    Set-Content -LiteralPath $navPagePath -Value "<!doctype html><title>antbrowser-workbench</title><body>antbrowser-workbench-$RunId</body>" -Encoding UTF8
    $testUrl = "file:///" + ([System.IO.Path]::GetFullPath($navPagePath) -replace "\\", "/")
    $screenshots = @{}
    foreach ($id in $selected) {
        [void](Invoke-AntApi -Method "POST" -Path "/api/workbench/navigate" -Body @{ profileId = $id; url = $testUrl } -TimeoutSec 30)
        Start-Sleep -Milliseconds 750
        [void](Invoke-AntApi -Method "POST" -Path "/api/workbench/refresh" -Body @{ profileId = $id } -TimeoutSec 30)
        Start-Sleep -Milliseconds 750
        $shot = Invoke-AntApi -Method "POST" -Path "/api/workbench/screenshot" -Body @{ profileId = $id } -TimeoutSec 30
        if (-not $shot.ok -or [string]::IsNullOrWhiteSpace($shot.screenshot) -or -not $shot.screenshot.StartsWith("data:image/")) {
            throw "Workbench screenshot failed for $id"
        }
        $screenshots[$id] = $shot.screenshot.Length
    }

    $arrange = Invoke-AntApi -Method "POST" -Path "/api/workbench/arrange" -Body @{ profileIds = $selected; layout = "grid" } -TimeoutSec 30
    if (-not $arrange.ok) {
        throw "Workbench arrange returned ok=false"
    }

    $first = $selected[0]
    $second = $selected[1]
    $firstStopped = Stop-SelectedProfile -ProfileId $first -RunningBefore $runningBefore -Stopped $stopped
    if (-not $firstStopped) {
        throw "Failed to stop first profile during workbench verification: $first"
    }
    Start-Sleep -Seconds 1

    [void](Invoke-AntApi -Method "POST" -Path "/api/workbench/refresh" -Body @{ profileId = $second } -TimeoutSec 30)
    $secondShot = Invoke-AntApi -Method "POST" -Path "/api/workbench/screenshot" -Body @{ profileId = $second } -TimeoutSec 30
    if (-not $secondShot.ok -or [string]::IsNullOrWhiteSpace($secondShot.screenshot)) {
        throw "Second profile did not remain controllable after stopping first profile"
    }

    $secondStopped = Stop-SelectedProfile -ProfileId $second -RunningBefore $runningBefore -Stopped $stopped
    if (-not $secondStopped) {
        throw "Failed to stop second profile during workbench verification: $second"
    }

    $summary = [pscustomobject]@{
        ok = $true
        baseUrl = $BaseUrl
        runId = $RunId
        selectedProfileIds = $selected
        temporaryProfileIds = @($temporaryProfiles | ForEach-Object { $_.ProfileId })
        navigatedUrl = $testUrl
        screenshotLengths = $screenshots
        arrangePlacementCount = @($arrange.placements).Count
        stoppedProfileIds = @($stopped)
        appStartedByScript = $startedApp
    }
} catch {
    $failure = $_
} finally {
    foreach ($id in $selected) {
        [void](Stop-SelectedProfile -ProfileId $id -RunningBefore $runningBefore -Stopped $stopped)
    }
    foreach ($temp in $temporaryProfiles) {
        if ($temp.ProfileId) {
            Remove-VerificationProfile -ProfileId $temp.ProfileId
        }
    }
    Remove-VerificationUserData

    if ($startedApp -and $null -ne $appProcess -and -not $KeepAppRunning) {
        try {
            if (-not $appProcess.HasExited) {
                Stop-VerificationBrowserProcesses
                [void]$appProcess.CloseMainWindow()
                if (-not $appProcess.WaitForExit(5000)) {
                    Stop-Process -Id $appProcess.Id -Force
                }
            }
        } catch {
            Write-Warning "Failed to close app process $($appProcess.Id): $($_.Exception.Message)"
        }
        Stop-VerificationBrowserProcesses
        Stop-NewResidualProcesses -Paths $ExpectedProcessPaths -PreExistingIds $preExistingProcessIds -RootProcessIds @([int]$appProcess.Id)
    }
    $env:ANTBROWSER_APP_ROOT = $PreviousAppRootEnv
}

if ($null -ne $failure) {
    Write-Error $failure
    exit 1
}

$summary | ConvertTo-Json -Depth 12
