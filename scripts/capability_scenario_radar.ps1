param(
  [string]$ReportPath = "",
  [string]$OutputDir = "data/reports/capability-scenarios"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "capability_scenario_lib.ps1")
$root = Get-CapabilityProjectRoot

if (-not $ReportPath) {
  $dir = Join-Path $root $OutputDir
  $latest = Get-ChildItem -LiteralPath $dir -Filter "suite-*.json" -File -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $latest) { Write-Error "no suite report found in $dir" }
  $ReportPath = $latest.FullName
}

$report = Get-Content -LiteralPath $ReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
$axes = Get-CapabilityRadarDimensions
$radar = @{}
foreach ($a in $axes) {
  $val = 0.0
  if ($report.radar) {
    $prop = $report.radar.PSObject.Properties[$a.id]
    if ($prop) { $val = [double]$prop.Value }
  }
  $gaps = @()
  if ($val -lt 100) {
    $gaps += "gap in $($a.label): $val% vs target 100%"
  }
  $radar[$a.id] = @{
    label = $a.label
    automationPct = $val
    designTarget = 100
    gap = $gaps -join "; "
  }
}

$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$outPath = Join-Path (Join-Path $root $OutputDir) "radar-$stamp.json"
$payload = @{
  schema = "capability_scenario_radar_v1"
  sourceReport = $ReportPath
  generatedAt = (Get-Date).ToString("o")
  observabilityScore = $report.observabilityScore
  automationCoveragePct = $report.summary.automationCoveragePct
  axes = $radar
  gaps = @($radar.Values | Where-Object { $_.automationPct -lt 100 } | ForEach-Object { "$($_.label): $($_.automationPct)% -> target 100%" })
}
$payload | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 $outPath
Write-Host "Radar data: $outPath"
Write-Host "Gaps:"
foreach ($g in $payload.gaps) { Write-Host "  - $g" }
