param(
  [string]$BridgeUrl = "",
  [string]$BridgeToken = "",
  [string]$ProfileId = "",
  [string]$PlanFile = "",
  [switch]$SpawnCore
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")
$root = Get-CapabilityProjectRoot
Set-Location $root

if (-not $PlanFile) { throw "PlanFile required" }
$planPath = if ([System.IO.Path]::IsPathRooted($PlanFile)) { $PlanFile } else { Join-Path $root $PlanFile }
if (-not (Test-Path $planPath)) { throw "plan not found: $planPath" }
if (-not $ProfileId) { throw "ProfileId required for primitive_plan" }

$harness = $null
try {
  if ($SpawnCore -or -not $BridgeUrl) {
    $harness = Start-CapabilityCoreHarness -Root $root
    $BridgeUrl = $harness.BridgeUrl
    $BridgeToken = $harness.BridgeToken
  }
  $planJson = Get-Content -LiteralPath $planPath -Raw -Encoding UTF8
  $result = Invoke-CapabilityCoreRpc -BridgeUrl $BridgeUrl -BridgeToken $BridgeToken `
    -Method "BehaviorExecutePrimitivePlan" -Args @($ProfileId, $planJson)
  Write-Output ($result | ConvertTo-Json -Depth 8)
}
finally {
  Stop-CapabilityCoreHarness -Harness $harness
}
