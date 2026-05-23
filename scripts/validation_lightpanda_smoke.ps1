param(
  [string]$Url = "https://example.com/",
  [int]$Repetitions = 2,
  [int]$TimeoutSeconds = 20,
  [string]$OutputDir = "data/reports/validation-smoke",
  [string]$ProfileId = ""
)

$ErrorActionPreference = "Stop"

$projectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $projectRoot
try {
  $argsList = @(
    "run", "--bin", "validation_lightpanda_smoke", "--",
    "--url", $Url,
    "--repetitions", $Repetitions.ToString(),
    "--timeout-seconds", $TimeoutSeconds.ToString(),
    "--output-dir", $OutputDir
  )
  if (-not [string]::IsNullOrWhiteSpace($ProfileId)) {
    $argsList += @("--profile-id", $ProfileId)
  }

  cargo @argsList
}
finally {
  Pop-Location
}
