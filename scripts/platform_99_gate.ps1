param(
    [ValidateSet("PGS", "XBS", "all")]
    [string]$Track = "all",
    [string]$Platform = "xhs",
    [int]$LaunchPort = 0
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Results = @()

function Add-Result {
    param(
        [string]$Id,
        [string]$Name,
        [bool]$Pass,
        [string]$Detail
    )
    $script:Results += [pscustomobject]@{
        Id     = $Id
        Name   = $Name
        Pass   = $Pass
        Detail = $Detail
    }
}

function Test-SourceContains {
    param(
        [string]$Id,
        [string]$Name,
        [string]$RelativePath,
        [string]$Pattern
    )
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path)) {
        Add-Result -Id $Id -Name $Name -Pass $false -Detail "missing file: $RelativePath"
        return
    }
    $content = Get-Content -Raw -Path $path
    $ok = $content -match $Pattern
    if ($ok) {
        Add-Result -Id $Id -Name $Name -Pass $true -Detail "pattern ok"
    } else {
        Add-Result -Id $Id -Name $Name -Pass $false -Detail "pattern not found in $RelativePath"
    }
}

Write-Host "=== Platform 99+ Gate ($Track) ===" -ForegroundColor Cyan

Test-SourceContains -Id "H-P1" -Name "sing-box port hold-until-bind" -RelativePath "backend/internal/proxy/port_reserve.go" -Pattern "func reservePortNumber"
Test-SourceContains -Id "H-P1b" -Name "sing-box uses reservePortNumber" -RelativePath "backend/internal/proxy/singbox.go" -Pattern "reservePortNumber\(\)"
Test-SourceContains -Id "H-P2" -Name "direct fallback validation" -RelativePath "backend/internal/proxy/direct_fallback.go" -Pattern "ValidateDirectFallbackSwitch"
Test-SourceContains -Id "H-P3" -Name "InjectionReady profile field" -RelativePath "backend/internal/browser/types.go" -Pattern "InjectionReady"
Test-SourceContains -Id "H-P3b" -Name "workbench injection gate" -RelativePath "backend/app_synchronizer.go" -Pattern "profileInjectionNotReadyError"
Test-SourceContains -Id "H-P4" -Name "API audit middleware" -RelativePath "backend/internal/launchcode/server_audit.go" -Pattern "apiAuditMiddleware"
Test-SourceContains -Id "H-P4b" -Name "audit logs endpoint" -RelativePath "backend/internal/launchcode/server.go" -Pattern "/api/audit/logs"
Test-SourceContains -Id "H-P5" -Name "script policy validation" -RelativePath "backend/app_launchcode.go" -Pattern "validateWorkbenchScriptPolicy"

if ($Track -in @("PGS", "all")) {
    Write-Host ""
    Write-Host "--- go test (PGS gates) ---" -ForegroundColor Yellow
    Push-Location (Join-Path $Root "backend")
    try {
        go test ./internal/proxy/... -run "TestReservePortNumber|TestValidateDirectFallback" -count=1
        if ($LASTEXITCODE -ne 0) {
            Add-Result -Id "H-P1t" -Name "proxy gate unit tests" -Pass $false -Detail "exit $LASTEXITCODE"
        } else {
            Add-Result -Id "H-P1t" -Name "proxy gate unit tests" -Pass $true -Detail "pass"
        }

        go test . -run "TestWorkbenchRejectsScript|TestWorkbenchRejectsBeforeInjection" -count=1
        if ($LASTEXITCODE -ne 0) {
            Add-Result -Id "H-P5t" -Name "workbench gate unit tests" -Pass $false -Detail "exit $LASTEXITCODE"
        } else {
            Add-Result -Id "H-P5t" -Name "workbench gate unit tests" -Pass $true -Detail "pass"
        }

        go test ./internal/launchcode/... -run "TestAPIAuditMiddleware" -count=1
        if ($LASTEXITCODE -ne 0) {
            Add-Result -Id "H-P4t" -Name "audit middleware unit test" -Pass $false -Detail "exit $LASTEXITCODE"
        } else {
            Add-Result -Id "H-P4t" -Name "audit middleware unit test" -Pass $true -Detail "pass"
        }
    } finally {
        Pop-Location
    }
}

if ($Track -in @("XBS", "all")) {
    $packRoot = Join-Path $Root "platform-packs/$Platform"
    $required = @("pack.yaml", "urls.yaml", "selectors.yaml", "cadence.yaml")
    foreach ($file in $required) {
        $exists = Test-Path (Join-Path $packRoot $file)
        if ($exists) {
            Add-Result -Id "S-$Platform-$file" -Name "platform pack $Platform/$file" -Pass $true -Detail "present"
        } else {
            Add-Result -Id "S-$Platform-$file" -Name "platform pack $Platform/$file" -Pass $false -Detail "missing"
        }
    }

    $planDir = Join-Path $Root "plans"
    foreach ($plan in @("account-login-v1.yaml", "account-nurture-conservative-v1.yaml", "feed-scrape-v1.yaml", "platform-console-readonly-v1.yaml")) {
        $exists = Test-Path (Join-Path $planDir $plan)
        if ($exists) {
            Add-Result -Id "S-plan-$plan" -Name "generic plan $plan" -Pass $true -Detail "present"
        } else {
            Add-Result -Id "S-plan-$plan" -Name "generic plan $plan" -Pass $false -Detail "missing"
        }
    }

    $spec = Join-Path $Root "docs/44-dual-track-99plus-spec.md"
    if (Test-Path $spec) {
        Add-Result -Id "S-spec-44" -Name "docs/44-dual-track-99plus-spec.md" -Pass $true -Detail "present"
    } else {
        Add-Result -Id "S-spec-44" -Name "docs/44-dual-track-99plus-spec.md" -Pass $false -Detail "missing"
    }

    # G0/G1 stealth gates (structure + unit)
    Test-SourceContains -Id "G0-1" -Name "InputPlaneRouter package" -RelativePath "backend/internal/behavior/inputplane/router.go" -Pattern "func ResolvePlane"
    Test-SourceContains -Id "G0-3" -Name "ActionRequest.inputMode" -RelativePath "backend/internal/launchcode/server.go" -Pattern "inputMode"
    Test-SourceContains -Id "G1-1" -Name "CDP-minimal Page.enable" -RelativePath "backend/internal/behavior/cdp_executor.go" -Pattern "EnableMinimalSession"
    Test-SourceContains -Id "G1-2" -Name "CreepJS 99+ threshold constant" -RelativePath "backend/internal/asymmetric/stealth_matrix.go" -Pattern "TargetCreepJSTrust99Plus"
    Test-SourceContains -Id "G1-2b" -Name "stealth-probe HTTP endpoint" -RelativePath "backend/internal/launchcode/server.go" -Pattern "/api/workbench/stealth-probe"
    Test-SourceContains -Id "G1-3" -Name "Camoufox launch arg filter" -RelativePath "backend/internal/browser/camoufox_launch.go" -Pattern "FilterLaunchArgsForCamoufox"

    Push-Location (Join-Path $Root "backend")
    try {
        go test ./internal/behavior/inputplane/... -count=1
        if ($LASTEXITCODE -ne 0) {
            Add-Result -Id "G0-1t" -Name "inputplane unit tests" -Pass $false -Detail "exit $LASTEXITCODE"
        } else {
            Add-Result -Id "G0-1t" -Name "inputplane unit tests" -Pass $true -Detail "pass"
        }
        go test ./internal/browser/... -run "TestFilterLaunchArgsForCamoufox|TestMaterializeRuntimeArgsForCore" -count=1
        if ($LASTEXITCODE -ne 0) {
            Add-Result -Id "G1-3t" -Name "camoufox launch unit tests" -Pass $false -Detail "exit $LASTEXITCODE"
        } else {
            Add-Result -Id "G1-3t" -Name "camoufox launch unit tests" -Pass $true -Detail "pass"
        }
        go test ./internal/detection/... -count=1
        if ($LASTEXITCODE -ne 0) {
            Add-Result -Id "G1-2t" -Name "detection/CreepJS unit tests" -Pass $false -Detail "exit $LASTEXITCODE"
        } else {
            Add-Result -Id "G1-2t" -Name "detection/CreepJS unit tests" -Pass $true -Detail "pass"
        }
    } finally {
        Pop-Location
    }
}

if ($LaunchPort -gt 0) {
    Write-Host ""
    Write-Host "--- live API audit probe (port $LaunchPort) ---" -ForegroundColor Yellow
    try {
        Invoke-RestMethod -Uri "http://127.0.0.1:$LaunchPort/api/health" -TimeoutSec 3 | Out-Null
        Invoke-RestMethod -Uri "http://127.0.0.1:$LaunchPort/api/instances/status?profileId=a4b108a0-afb3-4e93-843e-f5ef22278b9d" -TimeoutSec 5 | Out-Null
        $logs = Invoke-RestMethod -Uri "http://127.0.0.1:$LaunchPort/api/audit/logs?limit=20" -TimeoutSec 5
        $items = @($logs.items)
        $apiHits = @($items | Where-Object { $_.category -eq "api" -and $_.path -like "/api/*" })
        Add-Result -Id "H-P4live" -Name "live audit logs contain API calls" -Pass ($apiHits.Count -gt 0) -Detail ("api entries=" + $apiHits.Count)
    } catch {
        $msg = $_.Exception.Message
        Add-Result -Id "H-P4live" -Name "live audit logs contain API calls" -Pass $false -Detail $msg
    }
}

Write-Host ""
Write-Host "=== Results ===" -ForegroundColor Cyan
$Results | Format-Table -AutoSize
$failed = @($Results | Where-Object { -not $_.Pass })
$passed = @($Results | Where-Object { $_.Pass })
$color = "Green"
if ($failed.Count -gt 0) { $color = "Red" }
Write-Host ("PASS: " + $passed.Count + "  FAIL: " + $failed.Count) -ForegroundColor $color

if ($failed.Count -gt 0) {
    exit 1
}
exit 0
