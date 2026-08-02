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
    $CandidateAppPath = Join-Path $ProjectRoot "personal-pilot-tauri.exe"
}
$BaselineAppPath = [System.IO.Path]::GetFullPath($BaselineAppPath)
$CandidateAppPath = [System.IO.Path]::GetFullPath($CandidateAppPath)
$BaseUrl = $BaseUrl.TrimEnd("/")
$RunId = "fp-{0}-{1}" -f ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()), ([Guid]::NewGuid().ToString("N").Substring(0, 8))
$ApiKey = $env:PERSONAL_PILOT_API_KEY
$ApiKeyHeader = if ([string]::IsNullOrWhiteSpace($env:PERSONAL_PILOT_API_KEY_HEADER)) { "X-Personal-Pilot-Api-Key" } else { $env:PERSONAL_PILOT_API_KEY_HEADER }
$ExpectedProcessPaths = @(
    $BaselineAppPath,
    $CandidateAppPath,
    (Join-Path (Split-Path -Parent $BaselineAppPath) "personal-pilot-core.exe"),
    (Join-Path (Split-Path -Parent $CandidateAppPath) "personal-pilot-core.exe"),
    (Join-Path $ProjectRoot "bin\personal-pilot-core.exe"),
    (Join-Path $ProjectRoot "bin\personal-pilot-core-x86_64-pc-windows-msvc.exe")
) | ForEach-Object { [System.IO.Path]::GetFullPath($_) } | Select-Object -Unique
$PreviousAppRootEnv = $env:PERSONAL_PILOT_APP_ROOT
$env:PERSONAL_PILOT_APP_ROOT = $ProjectRoot

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

function Invoke-PersonalPilotApi {
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

function Test-PersonalPilotHealth {
    try {
        $health = Invoke-PersonalPilotApi -Method "GET" -Path "/api/health" -TimeoutSec 3
        return [bool]$health.ok
    } catch {
        return $false
    }
}

function Wait-PersonalPilotHealth {
    param([int]$TimeoutSec)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        if (Test-PersonalPilotHealth) { return }
        Start-Sleep -Milliseconds 500
    }
    throw "LaunchServer health check timed out at $BaseUrl/api/health"
}

function Get-PersonalPilotProfiles {
    $resp = Invoke-PersonalPilotApi -Method "GET" -Path "/api/profiles" -TimeoutSec 20
    if (-not $resp.ok) { throw "GET /api/profiles returned ok=false" }
    return @($resp.items)
}

function Wait-ProfileReady {
    param([Parameter(Mandatory = $true)][string]$TargetProfileId)
    $deadline = (Get-Date).AddSeconds($ReadyTimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $profile = Get-PersonalPilotProfiles | Where-Object { $_.profileId -eq $TargetProfileId } | Select-Object -First 1
        if ($null -ne $profile -and $profile.running -and $profile.debugReady -and [int]$profile.debugPort -gt 0 -and [int]$profile.pid -gt 0) {
            return $profile
        }
        Start-Sleep -Milliseconds 750
    }
    throw "Profile $TargetProfileId did not become Running+DebugReady within $ReadyTimeoutSec seconds"
}

function New-TemporaryProfile {
    $relativeUserData = "verification/$RunId/profile"
    $resp = Invoke-PersonalPilotApi -Method "POST" -Path "/api/profiles" -Body @{
        profile = @{
            profileName = "fingerprint-$RunId"
            userDataDir = $relativeUserData
            launchArgs = @("--window-size=1200,800")
            tags = @("personal-pilot-fingerprint-verify", $RunId)
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
        [void](Invoke-PersonalPilotApi -Method "POST" -Path "/api/instances/stop" -Body @{ profileId = $TargetProfileId } -TimeoutSec 30)
    } catch {}
}

function Remove-Profile {
    param([Parameter(Mandatory = $true)][string]$TargetProfileId)
    try {
        [void](Invoke-PersonalPilotApi -Method "DELETE" -Path "/api/profiles/$TargetProfileId" -TimeoutSec 30)
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

function Get-CanonicalUserDataDir {
    param([Parameter(Mandatory = $true)][string]$UserDataDir)
    $trimmed = $UserDataDir.Trim()
    if ([System.IO.Path]::IsPathRooted($trimmed)) {
        return [System.IO.Path]::GetFullPath($trimmed)
    }
    return [System.IO.Path]::GetFullPath((Join-Path (Join-Path $ProjectRoot "data") $trimmed))
}

function Get-Sha256Text {
    param([string]$Text)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes([string]$Text)
        $hash = $sha.ComputeHash($bytes)
        return ([BitConverter]::ToString($hash) -replace "-", "").ToLowerInvariant()
    } finally {
        $sha.Dispose()
    }
}

function Get-ValueHash {
    param($Value)
    return Get-Sha256Text -Text (StableJson $Value)
}

function Get-CommandLineUserDataDir {
    param([string]$CommandLine)
    if ([string]::IsNullOrWhiteSpace($CommandLine)) {
        return ""
    }
    if ($CommandLine -match '--user-data-dir=(?:"([^"]+)"|([^\s]+))') {
        $value = if (-not [string]::IsNullOrWhiteSpace($Matches[1])) { $Matches[1] } else { $Matches[2] }
        return [System.IO.Path]::GetFullPath($value)
    }
    return ""
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
        [int]$TimeoutSec = 10
    )

    $wsUrl = Get-CDPPageWebSocketUrl -DebugPort $DebugPort
    $socket = [System.Net.WebSockets.ClientWebSocket]::new()
    $cts = [System.Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds($TimeoutSec))
    $messageId = [Math]::Abs([Guid]::NewGuid().GetHashCode())
    try {
        $socket.ConnectAsync([Uri]$wsUrl, $cts.Token).GetAwaiter().GetResult()
        $payload = @{ id = $messageId; method = $Method; params = $Params } | ConvertTo-Json -Depth 30 -Compress
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

function Invoke-CDPEvaluateJson {
    param(
        [Parameter(Mandatory = $true)][int]$DebugPort,
        [Parameter(Mandatory = $true)][string]$Expression,
        [int]$TimeoutSec = 10
    )
    $result = Invoke-CDPCommand -DebugPort $DebugPort -Method "Runtime.evaluate" -Params @{
        expression = $Expression
        returnByValue = $true
        awaitPromise = $true
    } -TimeoutSec $TimeoutSec
    if ($null -ne $result.exceptionDetails) {
        throw "CDP Runtime.evaluate exception: $($result.exceptionDetails.text)"
    }
    $value = [string]$result.result.value
    if ([string]::IsNullOrWhiteSpace($value)) {
        throw "CDP Runtime.evaluate returned empty value"
    }
    return ($value | ConvertFrom-Json)
}

function Set-CookieMarker {
    param(
        [Parameter(Mandatory = $true)][int]$DebugPort,
        [Parameter(Mandatory = $true)][string]$MarkerValue
    )
    [void](Invoke-CDPCommand -DebugPort $DebugPort -Method "Network.enable")
    $result = Invoke-CDPCommand -DebugPort $DebugPort -Method "Network.setCookie" -Params @{
        name = "personal_pilot_fingerprint_marker"
        value = $MarkerValue
        url = "https://fingerprint-gate.invalid/"
        path = "/"
        expires = [double]([DateTimeOffset]::UtcNow.AddDays(2).ToUnixTimeSeconds())
        sameSite = "Lax"
    }
    if (-not [bool]$result.success) {
        throw "CDP Network.setCookie did not accept fingerprint marker"
    }
}

function Get-CookieMarkerValues {
    param([Parameter(Mandatory = $true)][int]$DebugPort)
    [void](Invoke-CDPCommand -DebugPort $DebugPort -Method "Network.enable")
    $result = Invoke-CDPCommand -DebugPort $DebugPort -Method "Network.getAllCookies"
    return @($result.cookies | Where-Object {
        $_.name -eq "personal_pilot_fingerprint_marker" -and ([string]$_.domain).TrimStart(".") -eq "fingerprint-gate.invalid"
    } | ForEach-Object { [string]$_.value })
}

function Get-AdvancedFingerprint {
    param([Parameter(Mandatory = $true)][int]$DebugPort)

    $script = @'
(async function() {
  function hashString(s) {
    var h = 2166136261;
    for (var i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h += (h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24);
    }
    return (h >>> 0).toString(16);
  }
  var info = {
    webRTC: { supported: false, candidateTypes: [], hasHostCandidate: false, hasSrflxCandidate: false, hasRelayCandidate: false, error: '' },
    canvasHash: '',
    canvasError: '',
    fontHash: '',
    fontError: '',
    audioHash: '',
    audioError: ''
  };
  try {
    var canvas = document.createElement('canvas');
    canvas.width = 280;
    canvas.height = 60;
    var ctx = canvas.getContext('2d');
    ctx.textBaseline = 'top';
    ctx.font = '14px Arial';
    ctx.fillStyle = '#069';
    ctx.fillText('Cwm fjordbank glyphs vext quiz 123', 4, 4);
    ctx.fillStyle = '#c00';
    ctx.font = 'bold 16px "Times New Roman"';
    ctx.fillText('The quick brown fox jumps', 2, 24);
    ctx.fillStyle = '#080';
    ctx.font = 'italic 12px "Courier New"';
    ctx.fillText('Sphinx of black quartz, judge my vow', 2, 44);
    info.canvasHash = hashString(canvas.toDataURL());
  } catch (e) {
    info.canvasError = String(e && e.message ? e.message : e);
  }
  try {
    var testFonts = ['Arial','Helvetica','Times New Roman','Courier New','Georgia','Verdana','SimSun','Microsoft YaHei','PingFang SC','Hiragino Sans GB'];
    var fontCanvas = document.createElement('canvas');
    var fontCtx = fontCanvas.getContext('2d');
    var testStr = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    var widths = [];
    for (var f = 0; f < testFonts.length; f++) {
      fontCtx.font = '16px "' + testFonts[f] + '"';
      widths.push(fontCtx.measureText(testStr).width.toFixed(2));
    }
    info.fontHash = hashString(widths.join(','));
  } catch (e) {
    info.fontError = String(e && e.message ? e.message : e);
  }
  try {
    var Ctor = window.RTCPeerConnection || window.webkitRTCPeerConnection;
    info.webRTC.supported = !!Ctor;
    if (Ctor) {
      var pc = new Ctor({ iceServers: [] });
      var candidates = [];
      pc.onicecandidate = function(e) {
        if (e && e.candidate && e.candidate.candidate) candidates.push(e.candidate.candidate);
      };
      pc.createDataChannel('personal-pilot');
      await pc.setLocalDescription(await pc.createOffer());
      await new Promise(function(resolve) { setTimeout(resolve, 900); });
      pc.close();
      var types = {};
      candidates.forEach(function(c) {
        var m = c.match(/ typ ([a-zA-Z0-9]+)/);
        if (m) types[m[1]] = true;
      });
      info.webRTC.candidateTypes = Object.keys(types).sort();
      info.webRTC.hasHostCandidate = !!types.host;
      info.webRTC.hasSrflxCandidate = !!types.srflx;
      info.webRTC.hasRelayCandidate = !!types.relay;
    }
  } catch (e) {
    info.webRTC.error = String(e && e.message ? e.message : e);
  }
  try {
    var AudioCtor = window.OfflineAudioContext || window.webkitOfflineAudioContext;
    if (!AudioCtor) {
      info.audioError = 'OfflineAudioContext unavailable';
    } else {
      var ctx = new AudioCtor(1, 44100, 44100);
      var osc = ctx.createOscillator();
      var comp = ctx.createDynamicsCompressor();
      osc.type = 'triangle';
      osc.frequency.value = 10000;
      comp.threshold.value = -50;
      comp.knee.value = 40;
      comp.ratio.value = 12;
      comp.attack.value = 0;
      comp.release.value = 0.25;
      osc.connect(comp);
      comp.connect(ctx.destination);
      osc.start(0);
      var buffer = await ctx.startRendering();
      var data = buffer.getChannelData(0);
      var sample = '';
      for (var i = 0; i < data.length; i += 97) sample += data[i].toFixed(6) + ',';
      info.audioHash = hashString(sample);
    }
  } catch (e) {
    info.audioError = String(e && e.message ? e.message : e);
  }
  return JSON.stringify(info);
})()
'@
    return Invoke-CDPEvaluateJson -DebugPort $DebugPort -Expression $script -TimeoutSec 15
}

function Collect-FingerprintRun {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string]$AppPath,
        [Parameter(Mandatory = $true)][string]$TargetProfileId,
        [Parameter(Mandatory = $true)][string]$CookieMarkerValue,
        [switch]$RequireExistingCookieMarker
    )
    if (-not (Test-Path -LiteralPath $AppPath)) {
        throw "$Label app executable not found: $AppPath"
    }
    if (Test-PersonalPilotHealth) {
        throw "LaunchServer is already running before $Label run; stop the app first to avoid mixing processes."
    }

    $appProcess = Start-Process -FilePath $AppPath -WorkingDirectory $ProjectRoot -PassThru
    try {
        Wait-PersonalPilotHealth -TimeoutSec $ReadyTimeoutSec
        [void](Invoke-PersonalPilotApi -Method "POST" -Path "/api/launch" -Body @{
            profileId = $TargetProfileId
            startUrls = @("about:blank")
            skipDefaultStartUrls = $true
        } -TimeoutSec 60)
        $profile = Wait-ProfileReady -TargetProfileId $TargetProfileId
        $verificationRoot = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot "data\verification"))
        $navRoot = [System.IO.Path]::GetFullPath((Join-Path $verificationRoot $RunId))
        New-Item -ItemType Directory -Path $navRoot -Force | Out-Null
        $navPagePath = Join-Path $navRoot "fingerprint.html"
        Set-Content -LiteralPath $navPagePath -Value "<!doctype html><title>personal-pilot-fingerprint</title><body>personal-pilot-fingerprint-$RunId</body>" -Encoding UTF8
        $navURL = "file:///" + ([System.IO.Path]::GetFullPath($navPagePath) -replace "\\", "/")
        [void](Invoke-PersonalPilotApi -Method "POST" -Path "/api/workbench/navigate" -Body @{
            profileId = $TargetProfileId
            url = $navURL
        } -TimeoutSec 30)
        Start-Sleep -Milliseconds 750
        $existingCookieMarkers = @(Get-CookieMarkerValues -DebugPort ([int]$profile.debugPort))
        if ($RequireExistingCookieMarker -and $existingCookieMarkers -notcontains $CookieMarkerValue) {
            throw "$Label did not find persisted Cookie marker $CookieMarkerValue before writing; got $($existingCookieMarkers -join ',')"
        }
        if (-not $RequireExistingCookieMarker) {
            Set-CookieMarker -DebugPort ([int]$profile.debugPort) -MarkerValue $CookieMarkerValue
        }
        $cookieMarkers = @(Get-CookieMarkerValues -DebugPort ([int]$profile.debugPort))
        if ($cookieMarkers -notcontains $CookieMarkerValue) {
            throw "$Label Cookie marker verification failed; got $($cookieMarkers -join ',')"
        }
        $fingerprintResp = Invoke-PersonalPilotApi -Method "POST" -Path "/api/workbench/fingerprint" -Body @{ profileId = $TargetProfileId } -TimeoutSec 30
        if (-not $fingerprintResp.ok) {
            throw "$Label fingerprint capture returned ok=false"
        }
        $advancedFingerprint = Get-AdvancedFingerprint -DebugPort ([int]$profile.debugPort)
        $shot = Invoke-PersonalPilotApi -Method "POST" -Path "/api/workbench/screenshot" -Body @{ profileId = $TargetProfileId } -TimeoutSec 30
        $browserCommandLine = Get-CommandLine -TargetProcessId ([int]$profile.pid)
        $canonicalUserDataDir = Get-CanonicalUserDataDir -UserDataDir ([string]$profile.userDataDir)
        $browserUserDataDir = Get-CommandLineUserDataDir -CommandLine $browserCommandLine
        if ([string]::IsNullOrWhiteSpace($browserUserDataDir)) {
            throw "$Label command line is missing --user-data-dir"
        }
        if (-not [string]::Equals($canonicalUserDataDir, $browserUserDataDir, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "$Label command line user-data-dir mismatch: profile=$canonicalUserDataDir browser=$browserUserDataDir"
        }
        $launchArgs = @($profile.launchArgs | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
        $fingerprintArgs = @($profile.fingerprintArgs | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
        $launchArgsHash = Get-ValueHash -Value $launchArgs
        $fingerprintArgsHash = Get-ValueHash -Value $fingerprintArgs
        $stableFields = [ordered]@{
            userAgent = $fingerprintResp.fingerprint.userAgent
            webRTC = $advancedFingerprint.webRTC
            canvas = $advancedFingerprint.canvasHash
            audio = $advancedFingerprint.audioHash
            webGL = [ordered]@{
                vendor = $fingerprintResp.fingerprint.webglVendor
                renderer = $fingerprintResp.fingerprint.webglRenderer
            }
            fonts = $advancedFingerprint.fontHash
            timezone = $fingerprintResp.fingerprint.timezone
            language = [ordered]@{
                language = $fingerprintResp.fingerprint.language
                languages = @($fingerprintResp.fingerprint.languages)
            }
            screen = [ordered]@{
                width = $fingerprintResp.fingerprint.screenWidth
                height = $fingerprintResp.fingerprint.screenHeight
                availWidth = $fingerprintResp.fingerprint.availWidth
                availHeight = $fingerprintResp.fingerprint.availHeight
                colorDepth = $fingerprintResp.fingerprint.colorDepth
                pixelDepth = $fingerprintResp.fingerprint.pixelDepth
                devicePixelRatio = $fingerprintResp.fingerprint.devicePixelRatio
            }
            hardwareConcurrency = $fingerprintResp.fingerprint.hardwareConcurrency
            deviceMemory = $fingerprintResp.fingerprint.deviceMemory
            proxy = [ordered]@{
                proxyId = $profile.proxyId
                proxyConfig = $profile.proxyConfig
                proxyBindSourceId = $profile.proxyBindSourceId
                proxyBindSourceUrl = $profile.proxyBindSourceUrl
                proxyBindName = $profile.proxyBindName
            }
            cookieMarker = [ordered]@{
                name = "personal_pilot_fingerprint_marker"
                value = $CookieMarkerValue
                present = ($cookieMarkers -contains $CookieMarkerValue)
            }
            userDataDir = $canonicalUserDataDir
            browserUserDataDir = $browserUserDataDir
            launchArgsHash = $launchArgsHash
            fingerprintArgsHash = $fingerprintArgsHash
        }
        Stop-Profile -TargetProfileId $TargetProfileId
        return [pscustomobject]@{
            label = $Label
            appPath = $AppPath
            profileId = $TargetProfileId
            profileName = $profile.profileName
            userDataDir = $profile.userDataDir
            canonicalUserDataDir = $canonicalUserDataDir
            browserUserDataDir = $browserUserDataDir
            proxyConfig = $profile.proxyConfig
            fingerprintArgs = $fingerprintArgs
            launchArgs = $launchArgs
            fingerprintArgsHash = $fingerprintArgsHash
            launchArgsHash = $launchArgsHash
            debugPort = [int]$profile.debugPort
            pid = [int]$profile.pid
            commandLine = $browserCommandLine
            commandLineHash = Get-Sha256Text -Text $browserCommandLine
            cookieMarker = [pscustomobject]@{
                name = "personal_pilot_fingerprint_marker"
                value = $CookieMarkerValue
                observedValues = $cookieMarkers
                requiredExisting = [bool]$RequireExistingCookieMarker
            }
            fingerprint = $fingerprintResp.fingerprint
            advancedFingerprint = $advancedFingerprint
            stableFields = [pscustomobject]$stableFields
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
    foreach ($field in @("canonicalUserDataDir", "browserUserDataDir", "proxyConfig", "fingerprintArgsHash", "launchArgsHash")) {
        if ((StableJson $Baseline.$field) -ne (StableJson $Candidate.$field)) {
            $mismatches.Add($field)
        }
    }
    foreach ($field in @(
        "userAgent", "webRTC", "canvas", "audio", "webGL", "fonts", "timezone",
        "language", "screen", "hardwareConcurrency", "deviceMemory", "proxy",
        "cookieMarker", "userDataDir", "browserUserDataDir", "launchArgsHash",
        "fingerprintArgsHash"
    )) {
        if ((StableJson $Baseline.stableFields.$field) -ne (StableJson $Candidate.stableFields.$field)) {
            $mismatches.Add("stableFields.$field")
        }
    }
    return @($mismatches)
}

$createdProfile = $false
$targetProfileId = $ProfileId.Trim()
$baseline = $null
$candidate = $null
$failure = $null
$cookieMarkerValue = "$RunId-cookie-marker"

try {
    if ([string]::IsNullOrWhiteSpace($targetProfileId)) {
        if (Test-PersonalPilotHealth) {
            throw "LaunchServer is already running; stop the app before temporary profile setup."
        }
        if (-not (Test-Path -LiteralPath $CandidateAppPath)) {
            throw "Candidate app executable not found: $CandidateAppPath"
        }
        $setupProcess = Start-Process -FilePath $CandidateAppPath -WorkingDirectory $ProjectRoot -PassThru
        try {
            Wait-PersonalPilotHealth -TimeoutSec $ReadyTimeoutSec
            $targetProfileId = New-TemporaryProfile
            $createdProfile = $true
        } finally {
            Stop-AppProcess -Process $setupProcess
            Start-Sleep -Milliseconds 500
        }
    }

    if (-not $SkipBaseline) {
        if (Test-Path -LiteralPath $BaselineAppPath) {
            $baseline = Collect-FingerprintRun -Label "baseline" -AppPath $BaselineAppPath -TargetProfileId $targetProfileId -CookieMarkerValue $cookieMarkerValue
        } else {
            Write-Warning "Baseline app not found; candidate-only verification will run."
        }
    }
    $candidate = Collect-FingerprintRun -Label "candidate" -AppPath $CandidateAppPath -TargetProfileId $targetProfileId -CookieMarkerValue $cookieMarkerValue -RequireExistingCookieMarker:($null -ne $baseline)

    $mismatches = @()
    if ($null -ne $baseline) {
        $mismatches = Compare-FingerprintRuns -Baseline $baseline -Candidate $candidate
    }
    [pscustomobject]@{
        ok = ($mismatches.Count -eq 0)
        compared = ($null -ne $baseline)
        runId = $RunId
        profileId = $targetProfileId
        coveredFields = @(
            "UA", "WebRTC", "Canvas", "Audio", "WebGL", "Fonts", "timezone",
            "language", "screen", "hardwareConcurrency", "deviceMemory", "proxy",
            "Cookie marker", "user-data-dir", "launch args hash", "fingerprint args hash"
        )
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
        if (-not (Test-PersonalPilotHealth)) {
            $cleanupProcess = Start-Process -FilePath $CandidateAppPath -WorkingDirectory $ProjectRoot -PassThru
            try {
                Wait-PersonalPilotHealth -TimeoutSec $ReadyTimeoutSec
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
    $env:PERSONAL_PILOT_APP_ROOT = $PreviousAppRootEnv
}
