# Bootstrap XHS profile with stealth fingerprint + proxy, launch headed browser, fill phone login.
param(
  [string]$Phone = "19202757042",
  [switch]$SkipStart
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")

$root = Get-CapabilityProjectRoot
Set-Location $root
$proxy = Import-CapabilityProxyConfig -Root $root

$phone = ($Phone -replace '\D', '').Trim()
if ($phone.Length -lt 11) { throw "invalid phone: $Phone" }

$userDataDir = "browser/user-data/xhs-$phone"
$sessionFile = Join-Path $root "data/scenarios/fixtures/xhs-login.session.json"
New-Item -ItemType Directory -Force -Path (Split-Path $sessionFile) | Out-Null

Write-Host "=== XHS login bootstrap ===" -ForegroundColor Cyan
Write-Host "Phone: $phone"
Write-Host "UserDataDir: data/$userDataDir (cookie persistence)"

$harness = Start-CapabilityCoreHarness -Root $root
Write-Host "Core ready: $($harness.BridgeUrl)"

function Invoke-Core {
  param([string]$Method, [object[]]$RpcArgs = @())
  Invoke-CapabilityCoreRpc -BridgeUrl $harness.BridgeUrl -BridgeToken $harness.BridgeToken -Method $Method -RpcArgList $RpcArgs
}

$fpArgs = @(
  "--fingerprint-brand=Chrome",
  "--fingerprint-platform=windows",
  "--fingerprint-platform-version=10.0.0",
  "--fingerprint-hardware-concurrency=8",
  "--fingerprint-device-memory=8",
  "--fingerprint-canvas-noise=true",
  "--fingerprint-audio-noise=true",
  "--fingerprint-audio-noise-value=0.00003",
  "--fingerprint-webgl-vendor=Google Inc. (Intel)",
  "--fingerprint-webgl-renderer=ANGLE (Intel, Intel(R) UHD Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)",
  "--lang=zh-CN",
  "--accept-lang=zh-CN,zh;q=0.9,en;q=0.8",
  "--timezone=Asia/Shanghai",
  "--window-size=1920,1080",
  "--force-device-scale-factor=1",
  "--webrtc-ip-handling-policy=disable_non_proxied_udp"
)

$launchArgs = @(
  "--disable-blink-features=AutomationControlled",
  "--disable-features=IsolateOrigins,site-per-process"
)

$profileInput = @{
  profileName          = "小红书 $phone"
  userDataDir          = $userDataDir
  coreId               = "core-fingerprint-chromium-139-0-7258-154"
  fingerprintArgs      = $fpArgs
  launchArgs           = $launchArgs
  tags                 = @("xhs", "nurture", "creator", "phone-login")
  keywords             = @("xhs", $phone, "creator-center")
  humanizeSeed         = "xhs-$phone"
  behaviorProfileId    = "xhs-nurture-conservative"
}

if ($proxy) {
  $profileInput.proxyConfig = $proxy.ProxyServer
  Write-Host "Proxy: $($proxy.Protocol) $($proxy.Host):$($proxy.Port)"
} else {
  Write-Host "WARN: no proxy.local.env — launching direct (not recommended for XHS)" -ForegroundColor Yellow
}

$list = @(Invoke-Core "BrowserProfileList")
$existing = $list | Where-Object { $_.userDataDir -eq $userDataDir -or $_.profileName -eq $profileInput.profileName } | Select-Object -First 1
if ($existing) {
  $profileId = [string]$existing.profileId
  Write-Host "Reusing profile: $profileId"
  $null = Invoke-Core "BrowserProfileUpdate" @($profileId, @{ profileName = $profileInput.profileName; userDataDir = $userDataDir; coreId = $profileInput.coreId; fingerprintArgs = $fpArgs; launchArgs = $launchArgs; tags = $profileInput.tags; keywords = $profileInput.keywords; humanizeSeed = $profileInput.humanizeSeed; behaviorProfileId = $profileInput.behaviorProfileId; proxyConfig = $profileInput.proxyConfig })
} else {
  $created = Invoke-Core "BrowserProfileCreate" @($profileInput)
  $profileId = [string]$created.profileId
  Write-Host "Created profile: $profileId"
}

if (-not $SkipStart) {
  try {
    Invoke-Core "BrowserInstanceStop" @($profileId) | Out-Null
    Start-Sleep -Seconds 2
  } catch {}

  Write-Host "Starting headed browser..."
  $started = Invoke-Core "BrowserInstanceStart" @($profileId)

  $deadline = (Get-Date).AddSeconds(90)
  $ready = $false
  while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 2
    try {
      $st = Invoke-Core "BrowserInstanceStatus" @($profileId)
      if ($st.running -and $st.debugReady) { $ready = $true; break }
      if ($st.running -and $st.debugPort -gt 0) { $ready = $true; break }
    } catch {}
  }
  if (-not $ready) { throw "browser did not become ready within 90s" }
  Write-Host "Browser ready debugPort=$($started.debugPort)"
  Start-Sleep -Seconds 5

  $navActions = @(
    @{ type = "navigate"; url = "https://www.xiaohongshu.com/login"; humanizationLevel = "high"; postWaitMs = 4000 }
  )
  $null = Invoke-Core "WorkbenchExecuteActions" @($profileId, $navActions)

  $phoneScript = @"
(function(){
  function clickText(re){
    var els=[].slice.call(document.querySelectorAll('button,a,div,span,p'));
    for(var i=0;i<els.length;i++){
      var t=(els[i].textContent||'').trim();
      if(t.length>0&&t.length<24&&re.test(t)&&els[i].offsetParent!==null){ els[i].click(); return t; }
    }
    return '';
  }
  clickText(/手机|短信/);
  var inputs=[].slice.call(document.querySelectorAll('input'));
  var phoneInput=null;
  for(var j=0;j<inputs.length;j++){
    var inp=inputs[j];
    var ph=(inp.placeholder||'')+(inp.name||'')+(inp.getAttribute('aria-label')||'');
    if(inp.type==='tel'||/手机|phone|mobile/i.test(ph)){ phoneInput=inp; break; }
  }
  if(!phoneInput){
    for(var k=0;k<inputs.length;k++){ if(inputs[k].type!=='hidden'&&inputs[k].type!=='password'){ phoneInput=inputs[k]; break; } }
  }
  if(!phoneInput) return JSON.stringify({ok:false,step:'phone',error:'input not found'});
  phoneInput.focus();
  phoneInput.value='$phone';
  phoneInput.dispatchEvent(new Event('input',{bubbles:true}));
  phoneInput.dispatchEvent(new Event('change',{bubbles:true}));
  var sent=clickText(/获取验证码|发送验证码/);
  return JSON.stringify({ok:true,step:'phone',phone:'$phone',sendClicked:!!sent,sendLabel:sent||''});
})()
"@

  $actions = @(
    @{ type = "wait"; distance = 3000; humanizationLevel = "high" },
    @{ type = "script"; script = $phoneScript; humanizationLevel = "high"; postWaitMs = 1500 }
  )

  $actionResults = Invoke-Core "WorkbenchExecuteActions" @($profileId, $actions)
  Write-Host "Phone fill result: $($actionResults | ConvertTo-Json -Compress -Depth 4)"
}

$session = @{
  schema       = "xhs_login_session_v1"
  createdAt    = (Get-Date).ToString("o")
  phone        = $phone
  profileId    = $profileId
  userDataDir  = "data/$userDataDir"
  bridgeUrl    = $harness.BridgeUrl
  bridgeToken  = $harness.BridgeToken
  corePid      = $harness.Process.Id
  status       = "awaiting_otp"
  note         = "Send OTP via scripts/xhs_submit_otp.ps1 -Code <6digits>"
}
$session | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 $sessionFile

Write-Host ""
Write-Host "Session saved: $sessionFile" -ForegroundColor Green
Write-Host "ProfileId: $profileId"
Write-Host "Cookies persist under: data/$userDataDir"
Write-Host "Core PID $($harness.Process.Id) — keep running for OTP step"
Write-Host ""
Write-Host "Next: reply with verification code, or run:" -ForegroundColor Yellow
Write-Host "  .\scripts\xhs_submit_otp.ps1 -Code <验证码>"
