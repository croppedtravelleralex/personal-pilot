# Submit XHS SMS OTP using saved login session.
param(
  [Parameter(Mandatory = $true)]
  [string]$Code,
  [string]$SessionFile = ""
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if (-not $SessionFile) {
  $SessionFile = Join-Path $root "data/scenarios/fixtures/xhs-login.session.json"
}
if (-not (Test-Path $SessionFile)) {
  throw "session file not found: $SessionFile"
}

$session = Get-Content $SessionFile -Raw -Encoding UTF8 | ConvertFrom-Json
$otp = ($Code -replace '\D', '').Trim()
if ($otp.Length -lt 4) { throw "invalid OTP code" }

$profileId = [string]$session.profileId
$launchBase = [string]$session.launchApiBase

$otpScript = @"
(function(){
  var code='$otp';
  var inputs=[].slice.call(document.querySelectorAll('input'));
  var codeInput=null;
  for(var i=0;i<inputs.length;i++){
    var inp=inputs[i];
    var ph=(inp.placeholder||'')+(inp.name||'')+(inp.getAttribute('aria-label')||'');
    if(inp.type==='password'||/验证码|code|captcha/i.test(ph)){ codeInput=inp; break; }
  }
  if(!codeInput){
    for(var j=0;j<inputs.length;j++){
      if(inputs[j].type!=='hidden'&&inputs[j].type!=='tel'&&inputs[j].value===''){ codeInput=inputs[j]; break; }
    }
  }
  if(!codeInput) return JSON.stringify({ok:false,step:'otp',error:'code input not found'});
  codeInput.focus();
  codeInput.value=code;
  codeInput.dispatchEvent(new Event('input',{bubbles:true}));
  codeInput.dispatchEvent(new Event('change',{bubbles:true}));
  var els=[].slice.call(document.querySelectorAll('button,a,div,span'));
  var login='';
  for(var k=0;k<els.length;k++){
    var t=(els[k].textContent||'').trim();
    if(/^(登录|登 录|确认|提交)$/.test(t)&&els[k].offsetParent!==null){ els[k].click(); login=t; break; }
  }
  return JSON.stringify({ok:true,step:'otp',loginClicked:!!login,loginLabel:login});
})()
"@

$actions = @(@{ type = "script"; script = $otpScript; humanizationLevel = "high"; postWaitMs = 4000 })

if ($launchBase) {
  $body = @{ profileId = $profileId; actions = $actions } | ConvertTo-Json -Depth 6
  $results = Invoke-RestMethod -Method Post -Uri "$launchBase/api/workbench/actions" -ContentType "application/json" -Body $body
} else {
  . (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")
  $results = Invoke-CapabilityCoreRpc -BridgeUrl $session.bridgeUrl -BridgeToken $session.bridgeToken -Method "WorkbenchExecuteActions" -RpcArgList @($profileId, $actions)
}

Write-Host "OTP submit: $($results | ConvertTo-Json -Compress -Depth 5)"
Start-Sleep -Seconds 5

$session.status = "otp_submitted"
$session.otpSubmittedAt = (Get-Date).ToString("o")
$session | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 $SessionFile
Write-Host "Done. Cookies persist in $($session.userDataDir)" -ForegroundColor Green
