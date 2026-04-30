param(
    [ValidateSet("web", "desktop")]
    [string]$Mode = "web",
    [int]$Port = 5218,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Push-Location $repoRoot

function Assert-Command {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found in PATH."
    }
}

function Resolve-ExecutablePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [string[]]$FallbackPaths = @()
    )

    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($command -and $command.Source) {
        return $command.Source
    }

    foreach ($path in $FallbackPaths) {
        if ($path -and (Test-Path $path)) {
            return $path
        }
    }

    return $null
}

function Initialize-RustToolchainPath {
    $cargo = Resolve-ExecutablePath -Name "cargo" -FallbackPaths @(
        (Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe")
    )

    if (-not $cargo) {
        throw "Desktop mode requires Rust. cargo was not found in PATH or under $env:USERPROFILE\.cargo\bin."
    }

    $cargoBin = Split-Path -Parent $cargo
    $pathEntries = $env:Path -split ';'
    if ($pathEntries -notcontains $cargoBin) {
        $env:Path = "$cargoBin;$env:Path"
        Write-Host "[Debug Entry] Added Rust toolchain to PATH: $cargoBin" -ForegroundColor DarkYellow
    }
}

function Get-ProcessCommandLine {
    param(
        [Parameter(Mandatory = $true)]
        [int]$ProcessId
    )

    $process = Get-CimInstance Win32_Process -Filter "ProcessId = $ProcessId" -ErrorAction SilentlyContinue
    return $process.CommandLine
}

function Ensure-DevPortAvailable {
    param(
        [Parameter(Mandatory = $true)]
        [int]$Port
    )

    $listeningPids = @(
        Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue |
            Select-Object -ExpandProperty OwningProcess -Unique
    )

    if (-not $listeningPids -or $listeningPids.Count -eq 0) {
        return
    }

    foreach ($processId in $listeningPids) {
        $commandLine = Get-ProcessCommandLine -ProcessId $processId
        if ($commandLine -and $commandLine.ToLowerInvariant().Contains($repoRoot.ToLowerInvariant())) {
            Write-Host "[Debug Entry] Stopping stale repo-owned dev process on port ${Port}: PID $processId" -ForegroundColor DarkYellow
            Stop-Process -Id $processId -Force -ErrorAction Stop
            continue
        }

        $details = if ($commandLine) { $commandLine } else { "(command line unavailable)" }
        throw "Port $Port is already in use by PID $processId. Command: $details"
    }

    Start-Sleep -Seconds 1

    $remainingPids = @(
        Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue |
            Select-Object -ExpandProperty OwningProcess -Unique
    )
    if ($remainingPids -and $remainingPids.Count -gt 0) {
        throw "Port $Port is still occupied after stale-process cleanup. PIDs: $($remainingPids -join ', ')"
    }
}

function Invoke-DebugCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Command
    )

    $executable = $Command[0]
    $arguments = @()
    if ($Command.Length -gt 1) {
        $arguments = $Command[1..($Command.Length - 1)]
    }

    & $executable @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Debug command failed with exit code $LASTEXITCODE."
    }
}

try {
    Assert-Command -Name "npm"
    Ensure-DevPortAvailable -Port $Port

    $command = @()
    if ($Mode -eq "web") {
        $command = @("npm", "run", "dev:raw", "--", "--host", "127.0.0.1", "--port", "$Port")
        Write-Host "[Debug Entry] Mode: web (Vite only)." -ForegroundColor Cyan
        Write-Host "[Debug Entry] URL: http://127.0.0.1:$Port/"
    }
    else {
        Initialize-RustToolchainPath
        $command = @("npm", "run", "tauri:dev")
        Write-Host "[Debug Entry] Mode: desktop (Tauri dev)." -ForegroundColor Cyan
    }

    Write-Host "[Debug Entry] Packaging is not involved in this path."
    Write-Host "[Debug Entry] Command: $($command -join ' ')"

    if ($DryRun) {
        Write-Host "[Debug Entry] DryRun complete." -ForegroundColor Green
        return
    }

    Invoke-DebugCommand -Command $command
}
finally {
    Pop-Location
}
