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

function Get-NewResidualProcesses {
    param(
        [string[]]$Paths,
        [hashtable]$PreExistingIds
    )

    $pathSet = @{}
    foreach ($path in $Paths) {
        $pathSet[[System.IO.Path]::GetFullPath($path).ToLowerInvariant()] = $true
    }
    $items = New-Object System.Collections.Generic.List[object]
    Get-Process | ForEach-Object {
        try {
            if (-not $_.Path) { return }
            if ($PreExistingIds.ContainsKey([int]$_.Id)) { return }
            $procPath = [System.IO.Path]::GetFullPath($_.Path).ToLowerInvariant()
            if ($pathSet.ContainsKey($procPath)) {
                $owner = Get-CimInstance Win32_Process -Filter "ProcessId = $($_.Id)"
                $commandLine = [string]$owner.CommandLine
                if ($commandLine.IndexOf($ProjectRoot, [System.StringComparison]::OrdinalIgnoreCase) -lt 0) { return }
                $items.Add([pscustomobject]@{
                    id = [int]$_.Id
                    path = $_.Path
                    commandLine = $commandLine
                })
            }
        } catch {}
    }
    return @($items.ToArray())
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
                $processId = [int]$_.ProcessId
                $parent = [int]$_.ParentProcessId
                if ($ids.ContainsKey($parent) -and -not $ids.ContainsKey($processId)) {
                    $ids[$processId] = $true
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

function Get-VerificationResidualProcesses {
    $tokens = @($RunId, $TempUserDataRoot) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    if ($tokens.Count -eq 0) {
        return @()
    }
    $items = New-Object System.Collections.Generic.List[object]
    Get-CimInstance Win32_Process | ForEach-Object {
        try {
            $commandLine = [string]$_.CommandLine
            if ([string]::IsNullOrWhiteSpace($commandLine)) { return }
            foreach ($token in $tokens) {
                if ($commandLine.IndexOf($token, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                    $items.Add([pscustomobject]@{
                        id = [int]$_.ProcessId
                        name = [string]$_.Name
                        commandLine = $commandLine
                    })
                    break
                }
            }
        } catch {}
    }
    return @($items.ToArray())
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

function Invoke-AntApiRaw {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][string]$Path,
        [object]$Body = $null,
        [int]$TimeoutSec = 20
    )

    $request = [System.Net.HttpWebRequest]::Create("$BaseUrl$Path")
    $request.Method = $Method
    $request.Timeout = $TimeoutSec * 1000
    if (-not [string]::IsNullOrWhiteSpace($ApiKey)) {
        $request.Headers[$ApiKeyHeader] = $ApiKey
    }
    if ($null -ne $Body) {
        $json = $Body | ConvertTo-Json -Depth 12 -Compress
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
        $request.ContentType = "application/json"
        $request.ContentLength = $bytes.Length
        $stream = $request.GetRequestStream()
        try {
            $stream.Write($bytes, 0, $bytes.Length)
        } finally {
            $stream.Close()
        }
    }

    $response = $null
    try {
        $response = $request.GetResponse()
    } catch [System.Net.WebException] {
        $response = $_.Exception.Response
        if ($null -eq $response) {
            return [pscustomobject]@{ statusCode = 0; body = ""; json = $null; error = $_.Exception.Message }
        }
    }

    $bodyText = ""
    try {
        $reader = [System.IO.StreamReader]::new($response.GetResponseStream())
        try {
            $bodyText = $reader.ReadToEnd()
        } finally {
            $reader.Close()
        }
    } finally {
        $response.Close()
    }

    $parsed = $null
    if (-not [string]::IsNullOrWhiteSpace($bodyText)) {
        try {
            $parsed = $bodyText | ConvertFrom-Json
        } catch {}
    }
    return [pscustomobject]@{ statusCode = [int]$response.StatusCode; body = $bodyText; json = $parsed; error = "" }
}

function Assert-AntApiBlocked {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][string]$Path,
        [object]$Body = $null
    )

    $resp = Invoke-AntApiRaw -Method $Method -Path $Path -Body $Body -TimeoutSec 30
    $ok = $false
    if ($null -ne $resp.json -and ($resp.json.PSObject.Properties.Name -contains "ok")) {
        $ok = [bool]$resp.json.ok
    } elseif ($resp.statusCode -ge 200 -and $resp.statusCode -lt 300) {
        $ok = $true
    }
    if ($resp.statusCode -ge 200 -and $resp.statusCode -lt 300 -and $ok) {
        return [pscustomobject]@{ blocked = $false; statusCode = $resp.statusCode; error = ""; json = $resp.json }
    }
    $errText = if ($null -ne $resp.json -and ($resp.json.PSObject.Properties.Name -contains "error")) { [string]$resp.json.error } else { $resp.body }
    return [pscustomobject]@{ blocked = $true; statusCode = $resp.statusCode; error = $errText; json = $resp.json }
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

function Wait-ProfileStopped {
    param(
        [Parameter(Mandatory = $true)][string]$ProfileId,
        [int]$TimeoutSec = 20
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $profile = Get-AntProfiles | Where-Object { $_.profileId -eq $ProfileId } | Select-Object -First 1
        if ($null -ne $profile -and -not [bool]$profile.running) {
            return $profile
        }
        Start-Sleep -Milliseconds 500
    }
    throw "Profile $ProfileId did not stop within $TimeoutSec seconds"
}

function Get-AntProfileById {
    param([Parameter(Mandatory = $true)][string]$ProfileId)

    $profile = Get-AntProfiles | Where-Object { $_.profileId -eq $ProfileId } | Select-Object -First 1
    if ($null -eq $profile) {
        throw "Profile not found: $ProfileId"
    }
    return $profile
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

function Get-CanonicalUserDataDir {
    param([Parameter(Mandatory = $true)][string]$UserDataDir)

    $trimmed = $UserDataDir.Trim()
    if ([System.IO.Path]::IsPathRooted($trimmed)) {
        return [System.IO.Path]::GetFullPath($trimmed)
    }
    return [System.IO.Path]::GetFullPath((Join-Path (Join-Path $ProjectRoot "data") $trimmed))
}

function Assert-ProfileBinding {
    param(
        [Parameter(Mandatory = $true)][object]$Profile,
        [string]$ExpectedUserDataDir = ""
    )

    if ([string]::IsNullOrWhiteSpace([string]$Profile.profileId)) {
        throw "Profile snapshot is missing profileId"
    }
    if ([string]::IsNullOrWhiteSpace([string]$Profile.userDataDir)) {
        throw "Profile $($Profile.profileId) is missing userDataDir"
    }
    $canonical = Get-CanonicalUserDataDir -UserDataDir ([string]$Profile.userDataDir)
    if (-not [string]::IsNullOrWhiteSpace($ExpectedUserDataDir)) {
        $expected = [System.IO.Path]::GetFullPath($ExpectedUserDataDir)
        if (-not [string]::Equals($canonical, $expected, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Profile $($Profile.profileId) user-data-dir drift: got $canonical; expected $expected"
        }
    }
    return $canonical
}

function Assert-UniqueRuntimeBinding {
    param([Parameter(Mandatory = $true)][object[]]$ProfileSnapshots)

    $dirs = @{}
    $ports = @{}
    $pids = @{}
    foreach ($profile in $ProfileSnapshots) {
        $dir = (Assert-ProfileBinding -Profile $profile).ToLowerInvariant()
        if ($dirs.ContainsKey($dir)) {
            throw "Profiles $($dirs[$dir]) and $($profile.profileId) share user-data-dir $dir"
        }
        $dirs[$dir] = [string]$profile.profileId

        $port = [int]$profile.debugPort
        if ($port -le 0) {
            throw "Profile $($profile.profileId) has invalid debugPort $port"
        }
        if ($ports.ContainsKey($port)) {
            throw "Profiles $($ports[$port]) and $($profile.profileId) share debugPort $port"
        }
        $ports[$port] = [string]$profile.profileId

        $processId = [int]$profile.pid
        if ($processId -gt 0) {
            if ($pids.ContainsKey($processId)) {
                throw "Profiles $($pids[$processId]) and $($profile.profileId) share pid $processId"
            }
            $pids[$processId] = [string]$profile.profileId
        }
    }
}

function Get-CDPPageWebSocketUrl {
    param([Parameter(Mandatory = $true)][int]$DebugPort)

    $targets = Invoke-RestMethod -Method "GET" -Uri "http://127.0.0.1:$DebugPort/json" -TimeoutSec 5
    $target = @($targets | Where-Object {
        $_.type -eq "page" -and
        -not [string]::IsNullOrWhiteSpace([string]$_.webSocketDebuggerUrl) -and
        -not ([string]$_.url).StartsWith("devtools://")
    } | Select-Object -First 1)
    if ($target.Count -eq 0) {
        throw "No page CDP target found on debugPort $DebugPort"
    }
    return [string]$target[0].webSocketDebuggerUrl
}

function Invoke-CDPCommand {
    param(
        [Parameter(Mandatory = $true)][int]$DebugPort,
        [Parameter(Mandatory = $true)][string]$Method,
        [object]$Params = @{},
        [int]$TimeoutSec = 8
    )

    $wsUrl = Get-CDPPageWebSocketUrl -DebugPort $DebugPort
    $socket = [System.Net.WebSockets.ClientWebSocket]::new()
    $cts = [System.Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds($TimeoutSec))
    $messageId = [Math]::Abs([Guid]::NewGuid().GetHashCode())
    try {
        $socket.ConnectAsync([Uri]$wsUrl, $cts.Token).GetAwaiter().GetResult()
        $payload = @{ id = $messageId; method = $Method; params = $Params } | ConvertTo-Json -Depth 20 -Compress
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($payload)
        $socket.SendAsync([ArraySegment[byte]]::new($bytes), [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cts.Token).GetAwaiter().GetResult()

        while ($true) {
            $buffer = New-Object byte[] 65536
            $memory = [System.IO.MemoryStream]::new()
            do {
                $result = $socket.ReceiveAsync([ArraySegment[byte]]::new($buffer), $cts.Token).GetAwaiter().GetResult()
                if ($result.Count -gt 0) {
                    $memory.Write($buffer, 0, $result.Count)
                }
            } while (-not $result.EndOfMessage)

            $text = [System.Text.Encoding]::UTF8.GetString($memory.ToArray())
            $resp = $text | ConvertFrom-Json
            if (-not ($resp.PSObject.Properties.Name -contains "id") -or [int]$resp.id -ne $messageId) {
                continue
            }
            if ($null -ne $resp.error) {
                throw "CDP $Method failed: $($resp.error.message)"
            }
            return $resp.result
        }
    } finally {
        if ($socket.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
            $socket.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "done", [System.Threading.CancellationToken]::None).GetAwaiter().GetResult()
        }
        $socket.Dispose()
        $cts.Dispose()
    }
}

function Set-CookieMarker {
    param(
        [Parameter(Mandatory = $true)][object]$Profile,
        [Parameter(Mandatory = $true)][string]$MarkerValue
    )

    [void](Invoke-CDPCommand -DebugPort ([int]$Profile.debugPort) -Method "Network.enable")
    $result = Invoke-CDPCommand -DebugPort ([int]$Profile.debugPort) -Method "Network.setCookie" -Params @{
        name = "antbrowser_identity_gate"
        value = $MarkerValue
        url = "https://identity-gate.invalid/"
        path = "/"
        expires = [double]([DateTimeOffset]::UtcNow.AddDays(2).ToUnixTimeSeconds())
        sameSite = "Lax"
    }
    if (-not [bool]$result.success) {
        throw "CDP Network.setCookie did not accept marker for $($Profile.profileId)"
    }
}

function Get-CookieMarkerValues {
    param([Parameter(Mandatory = $true)][object]$Profile)

    [void](Invoke-CDPCommand -DebugPort ([int]$Profile.debugPort) -Method "Network.enable")
    $result = Invoke-CDPCommand -DebugPort ([int]$Profile.debugPort) -Method "Network.getAllCookies"
    return @($result.cookies | Where-Object {
        $_.name -eq "antbrowser_identity_gate" -and ([string]$_.domain).TrimStart(".") -eq "identity-gate.invalid"
    } | ForEach-Object { [string]$_.value })
}

function Assert-CookieMarker {
    param(
        [Parameter(Mandatory = $true)][object]$Profile,
        [Parameter(Mandatory = $true)][string]$ExpectedValue,
        [string[]]$ForbiddenValues = @()
    )

    $values = @(Get-CookieMarkerValues -Profile $Profile)
    if ($values -notcontains $ExpectedValue) {
        throw "Profile $($Profile.profileId) is missing Cookie marker $ExpectedValue; got $($values -join ',')"
    }
    foreach ($forbidden in $ForbiddenValues) {
        if ($values -contains $forbidden) {
            throw "Profile $($Profile.profileId) contains cross-profile Cookie marker $forbidden"
        }
    }
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
        UserDataDir = Get-CanonicalUserDataDir -UserDataDir $relativeUserData
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

function Assert-DuplicateUserDataDirBlocked {
    param([Parameter(Mandatory = $true)][object]$ReferenceProfile)

    $resp = Assert-AntApiBlocked -Method "POST" -Path "/api/profiles" -Body @{
        profile = @{
            profileName = "verify-$RunId-duplicate-user-data-dir"
            userDataDir = [string]$ReferenceProfile.userDataDir
            launchArgs = @("--window-size=1200,800")
            tags = @("antbrowser-verify", $RunId, "negative")
            keywords = @($RunId, "duplicate-user-data-dir")
        }
    }
    if (-not $resp.blocked) {
        $createdId = if ($null -ne $resp.json -and ($resp.json.PSObject.Properties.Name -contains "profileId")) { [string]$resp.json.profileId } else { "" }
        if (-not [string]::IsNullOrWhiteSpace($createdId)) {
            Remove-VerificationProfile -ProfileId $createdId
        }
        throw "Duplicate user-data-dir was accepted for $($ReferenceProfile.profileId)"
    }
    return $resp
}

function Assert-PathMismatchBlocked {
    param([Parameter(Mandatory = $true)][object]$ReferenceProfile)

    $originalDir = [string]$ReferenceProfile.userDataDir
    $mismatchDir = "verification/$RunId/path-mismatch-$($ReferenceProfile.profileId)"
    $resp = Assert-AntApiBlocked -Method "PUT" -Path "/api/profiles/$($ReferenceProfile.profileId)" -Body @{
        profile = @{
            profileName = [string]$ReferenceProfile.profileName
            userDataDir = $mismatchDir
            coreId = [string]$ReferenceProfile.coreId
            fingerprintArgs = @($ReferenceProfile.fingerprintArgs | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
            proxyId = [string]$ReferenceProfile.proxyId
            proxyConfig = [string]$ReferenceProfile.proxyConfig
            launchArgs = @($ReferenceProfile.launchArgs | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
            tags = @($ReferenceProfile.tags | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
            keywords = @($ReferenceProfile.keywords | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
            groupId = [string]$ReferenceProfile.groupId
        }
    }
    if (-not $resp.blocked) {
        throw "Running profile path mismatch update was accepted for $($ReferenceProfile.profileId)"
    }

    $after = Get-AntProfileById -ProfileId ([string]$ReferenceProfile.profileId)
    $beforeCanonical = Get-CanonicalUserDataDir -UserDataDir $originalDir
    $afterCanonical = Get-CanonicalUserDataDir -UserDataDir ([string]$after.userDataDir)
    if (-not [string]::Equals($beforeCanonical, $afterCanonical, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Blocked path mismatch changed profile $($ReferenceProfile.profileId) from $beforeCanonical to $afterCanonical"
    }
    return $resp
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
$cleanupFailure = $null
$cleanupReport = $null
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
    $expectedDirsByProfile = @{}
    foreach ($temp in $temporaryProfiles) {
        $expectedDirsByProfile[$temp.ProfileId] = $temp.UserDataDir
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
        if ($expectedDirsByProfile.ContainsKey($id)) {
            [void](Assert-ProfileBinding -Profile $profile -ExpectedUserDataDir $expectedDirsByProfile[$id])
        } else {
            [void](Assert-ProfileBinding -Profile $profile)
        }
    }

    $readyProfiles = @{}
    foreach ($id in $selected) {
        [void](Invoke-AntApi -Method "POST" -Path "/api/launch" -Body @{
            profileId = $id
            startUrls = @("about:blank")
            skipDefaultStartUrls = $true
        } -TimeoutSec 60)
        $readyProfiles[$id] = Wait-ProfileReady -ProfileId $id -TimeoutSec $ReadyTimeoutSec
        if ($expectedDirsByProfile.ContainsKey($id)) {
            [void](Assert-ProfileBinding -Profile $readyProfiles[$id] -ExpectedUserDataDir $expectedDirsByProfile[$id])
        }
    }
    Assert-UniqueRuntimeBinding -ProfileSnapshots @($selected | ForEach-Object { $readyProfiles[$_] })

    $assetSafetyChecks = [ordered]@{
        profileIsolation = $true
        stopOneKeepOne = $true
        cookieMarker = "skipped-existing-profiles"
        duplicateUserDataDirBlocked = "skipped-existing-profiles"
        pathMismatchBlocked = "skipped-existing-profiles"
    }
    $cookieMarkers = @{}
    if ($temporaryProfiles.Count -ge 2) {
        for ($idx = 0; $idx -lt $selected.Count; $idx++) {
            $marker = "$RunId-profile-$($idx + 1)"
            $cookieMarkers[$selected[$idx]] = $marker
            Set-CookieMarker -Profile $readyProfiles[$selected[$idx]] -MarkerValue $marker
        }
        foreach ($id in $selected) {
            $forbidden = @($cookieMarkers.Values | Where-Object { $_ -ne $cookieMarkers[$id] })
            Assert-CookieMarker -Profile $readyProfiles[$id] -ExpectedValue $cookieMarkers[$id] -ForbiddenValues $forbidden
        }
        $assetSafetyChecks.cookieMarker = "set-and-isolated"
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
    [void](Wait-ProfileStopped -ProfileId $first)

    [void](Invoke-AntApi -Method "POST" -Path "/api/workbench/refresh" -Body @{ profileId = $second } -TimeoutSec 30)
    $secondShot = Invoke-AntApi -Method "POST" -Path "/api/workbench/screenshot" -Body @{ profileId = $second } -TimeoutSec 30
    if (-not $secondShot.ok -or [string]::IsNullOrWhiteSpace($secondShot.screenshot)) {
        throw "Second profile did not remain controllable after stopping first profile"
    }
    if ($temporaryProfiles.Count -ge 2) {
        Assert-CookieMarker -Profile (Get-AntProfileById -ProfileId $second) -ExpectedValue $cookieMarkers[$second] -ForbiddenValues @($cookieMarkers[$first])
    }

    [void]$stopped.Remove($first)
    [void](Invoke-AntApi -Method "POST" -Path "/api/launch" -Body @{
        profileId = $first
        startUrls = @("about:blank")
        skipDefaultStartUrls = $true
    } -TimeoutSec 60)
    $readyProfiles[$first] = Wait-ProfileReady -ProfileId $first -TimeoutSec $ReadyTimeoutSec
    if ($expectedDirsByProfile.ContainsKey($first)) {
        [void](Assert-ProfileBinding -Profile $readyProfiles[$first] -ExpectedUserDataDir $expectedDirsByProfile[$first])
    }
    if ($temporaryProfiles.Count -ge 2) {
        Assert-CookieMarker -Profile $readyProfiles[$first] -ExpectedValue $cookieMarkers[$first] -ForbiddenValues @($cookieMarkers[$second])
    }

    $secondStopped = Stop-SelectedProfile -ProfileId $second -RunningBefore $runningBefore -Stopped $stopped
    if (-not $secondStopped) {
        throw "Failed to stop second profile during workbench verification: $second"
    }
    [void](Wait-ProfileStopped -ProfileId $second)

    [void](Invoke-AntApi -Method "POST" -Path "/api/workbench/refresh" -Body @{ profileId = $first } -TimeoutSec 30)
    $firstShot = Invoke-AntApi -Method "POST" -Path "/api/workbench/screenshot" -Body @{ profileId = $first } -TimeoutSec 30
    if (-not $firstShot.ok -or [string]::IsNullOrWhiteSpace($firstShot.screenshot)) {
        throw "First profile did not remain controllable after stopping second profile"
    }

    [void]$stopped.Remove($second)
    [void](Invoke-AntApi -Method "POST" -Path "/api/launch" -Body @{
        profileId = $second
        startUrls = @("about:blank")
        skipDefaultStartUrls = $true
    } -TimeoutSec 60)
    $readyProfiles[$second] = Wait-ProfileReady -ProfileId $second -TimeoutSec $ReadyTimeoutSec
    if ($expectedDirsByProfile.ContainsKey($second)) {
        [void](Assert-ProfileBinding -Profile $readyProfiles[$second] -ExpectedUserDataDir $expectedDirsByProfile[$second])
    }
    Assert-UniqueRuntimeBinding -ProfileSnapshots @($selected | ForEach-Object { $readyProfiles[$_] })
    if ($temporaryProfiles.Count -ge 2) {
        Assert-CookieMarker -Profile $readyProfiles[$second] -ExpectedValue $cookieMarkers[$second] -ForbiddenValues @($cookieMarkers[$first])
        $assetSafetyChecks.cookieMarker = "set-isolated-and-persisted-after-restart"

        $dupResp = Assert-DuplicateUserDataDirBlocked -ReferenceProfile (Get-AntProfileById -ProfileId $first)
        $pathResp = Assert-PathMismatchBlocked -ReferenceProfile (Get-AntProfileById -ProfileId $first)
        $assetSafetyChecks.duplicateUserDataDirBlocked = "status:$($dupResp.statusCode)"
        $assetSafetyChecks.pathMismatchBlocked = "status:$($pathResp.statusCode)"
    }

    $summary = [pscustomobject]@{
        ok = $true
        baseUrl = $BaseUrl
        runId = $RunId
        selectedProfileIds = $selected
        temporaryProfileIds = @($temporaryProfiles | ForEach-Object { $_.ProfileId })
        userDataDirs = @($selected | ForEach-Object {
            $p = Get-AntProfileById -ProfileId $_
            [pscustomobject]@{
                profileId = $_
                userDataDir = [string]$p.userDataDir
                canonicalUserDataDir = Get-CanonicalUserDataDir -UserDataDir ([string]$p.userDataDir)
            }
        })
        assetSafetyChecks = $assetSafetyChecks
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
    Stop-VerificationBrowserProcesses
    Remove-VerificationUserData

    Stop-VerificationBrowserProcesses
    if ($startedApp -and $null -ne $appProcess -and -not $KeepAppRunning) {
        try {
            if (-not $appProcess.HasExited) {
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
    if (-not $KeepAppRunning) {
        Stop-VerificationBrowserProcesses
        $verificationResiduals = @(Get-VerificationResidualProcesses)
        $appResiduals = @(Get-NewResidualProcesses -Paths $ExpectedProcessPaths -PreExistingIds $preExistingProcessIds)
        $cleanupReport = [pscustomobject]@{
            verificationResidualCount = $verificationResiduals.Count
            appResidualCount = $appResiduals.Count
        }
        if ($null -ne $summary) {
            $summary | Add-Member -NotePropertyName cleanup -NotePropertyValue $cleanupReport -Force
        }
        if ($verificationResiduals.Count -gt 0 -or ($startedApp -and $appResiduals.Count -gt 0)) {
            $cleanupFailure = "Residual verification processes remain: verification=$($verificationResiduals.Count), app=$($appResiduals.Count)"
        }
    }
    $env:ANTBROWSER_APP_ROOT = $PreviousAppRootEnv
}

if ($null -ne $failure) {
    Write-Error $failure
    exit 1
}
if ($null -ne $cleanupFailure) {
    Write-Error $cleanupFailure
    exit 1
}

$summary | ConvertTo-Json -Depth 12
