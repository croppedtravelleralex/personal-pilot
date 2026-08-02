param(
  [string]$OutputDir = "data/reports/provider-acceptance"
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$absoluteOutputDir = Join-Path $projectRoot $OutputDir
New-Item -ItemType Directory -Force -Path $absoluteOutputDir | Out-Null

$domains = @(
  [ordered]@{
    domain = "captcha"
    managerContract = @("select_solver_provider", "detect_challenge", "request_solution", "fill_solution", "preserve_failure_reason")
    cdpContract = @("detect_sitekey", "inject_token", "verify_page_result")
    productionStatus = "contract_only"
    blockers = @("missing real credentials", "runtime manager not wired into task flow", "real provider smoke not run")
  },
  [ordered]@{
    domain = "sms"
    managerContract = @("select_sms_provider", "request_number", "fill_phone", "wait_code", "fill_code", "finish_or_cancel")
    cdpContract = @("detect_phone_field", "detect_code_field", "fill_phone_and_code")
    productionStatus = "contract_only"
    blockers = @("missing real credentials", "number purchase/status/cancel/finish flow not wired", "real provider smoke not run")
  },
  [ordered]@{
    domain = "email"
    managerContract = @("create_or_reuse_inbox", "persist_session_identity", "wait_code", "fill_email_and_code", "preserve_registration_result")
    cdpContract = @("detect_email_field", "detect_code_field", "fill_email_and_code")
    productionStatus = "contract_only"
    blockers = @("registration-flow smoke not run", "CDP fill not wired into automation runtime")
  }
)

$report = [ordered]@{
  schemaVersion = "provider_manager_wiring_gate_v1"
  generatedAt = (Get-Date).ToString("o")
  status = "contract_ready_real_provider_blocked"
  projectRoot = $projectRoot
  domains = $domains
  failureReason = "manager/CDP contracts are documented, but production closure requires credentials, task-flow wiring, CDP execution, and real provider smoke"
  notes = @(
    "This gate does not call paid or external providers.",
    "It preserves the minimum manager/CDP contract needed before real provider acceptance."
  )
}

$reportPath = Join-Path $absoluteOutputDir ("provider-manager-wiring-gate-{0}.json" -f ([DateTimeOffset]::Now.ToUnixTimeMilliseconds()))
$report | ConvertTo-Json -Depth 8 | Set-Content -Path $reportPath -Encoding UTF8
Write-Host "Provider manager wiring gate report: $reportPath"
Write-Host "Status: $($report.status)"
