<#
.SYNOPSIS
    Builds and runs DeskSpace for quick manual testing.

.DESCRIPTION
    Stops any DeskSpace started from this repository's target folder (a
    running instance locks the executable and makes the build fail), builds,
    and runs the app in the foreground. Press Ctrl+C or use the tray menu's
    Exit to stop it; both save the configuration.

    By default the app uses a scratch configuration under
    %TEMP%\deskspace-dev, so your real configuration is never touched. The
    scratch file persists between runs; use -Fresh to start over.

.EXAMPLE
    ./scripts/dev.ps1
    Debug build, scratch config, spaces seeded from the desktop folder.

.EXAMPLE
    ./scripts/dev.ps1 -Fresh -OnTop -Settings
    Start from an empty scratch config, spaces above other windows, settings open.

.EXAMPLE
    ./scripts/dev.ps1 -Check -Release
    Run fmt, clippy, and tests first, then a release build.

.EXAMPLE
    ./scripts/dev.ps1 -RealConfig -- --placement desktop-background
    Use the real configuration and pass extra arguments to DeskSpace.
#>
[CmdletBinding()]
param(
    # Build with optimizations.
    [switch]$Release,
    # Delete the scratch configuration before running.
    [switch]$Fresh,
    # Use the real configuration (%APPDATA%) instead of the scratch one.
    [switch]$RealConfig,
    # Keep spaces above other windows so they are visible over your editor.
    [switch]$OnTop,
    # Open the settings window on start.
    [switch]$Settings,
    # Do not seed a space from the desktop folder.
    [switch]$NoIcons,
    # Run cargo fmt, clippy (warnings as errors), and tests before building.
    [switch]$Check,
    # Build only; do not run.
    [switch]$NoRun,
    # Log verbosity: 0 = info, 1 = debug, 2 = trace.
    [ValidateRange(0, 2)]
    [int]$Verbosity = 0,
    # Extra arguments passed to DeskSpace unchanged.
    [Parameter(ValueFromRemainingArguments)]
    [string[]]$AppArgs
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$profileDir = if ($Release) { 'release' } else { 'debug' }
$exe = Join-Path $root "target\$profileDir\deskspace.exe"
$scratchDir = Join-Path $env:TEMP 'deskspace-dev'
$scratchConfig = Join-Path $scratchDir 'deskspace.toml'

function Invoke-Cargo {
    param([string[]]$Arguments)
    Write-Host "> cargo $($Arguments -join ' ')" -ForegroundColor DarkGray
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($Arguments[0]) failed with exit code $LASTEXITCODE"
    }
}

Push-Location $root
try {
    $targetDir = Join-Path $root 'target'
    $running = Get-Process -Name deskspace -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($targetDir, [StringComparison]::OrdinalIgnoreCase) }
    foreach ($process in $running) {
        Write-Host "Stopping DeskSpace (pid $($process.Id)) so the build can replace it" -ForegroundColor Yellow
        Stop-Process -Id $process.Id -Force
        $process.WaitForExit(5000) | Out-Null
    }

    if ($Check) {
        Invoke-Cargo @('fmt', '--all')
        Invoke-Cargo @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
        Invoke-Cargo @('test', '--workspace')
    }

    $build = @('build', '-p', 'deskspace-app')
    if ($Release) { $build += '--release' }
    Invoke-Cargo $build

    if ($NoRun) {
        Write-Host "Built $exe" -ForegroundColor Green
        return
    }

    $arguments = @()
    if (-not $RealConfig) {
        if ($Fresh -and (Test-Path $scratchDir)) {
            Remove-Item -Recurse -Force $scratchDir
        }
        New-Item -ItemType Directory -Force -Path $scratchDir | Out-Null
        $arguments += @('--config', $scratchConfig)
    }
    if ($OnTop) { $arguments += '--on-top' }
    if ($Settings) { $arguments += '--settings' }
    if ($NoIcons) { $arguments += '--no-icons' }
    if ($Verbosity -gt 0) { $arguments += '-' + ('v' * $Verbosity) }
    if ($AppArgs) { $arguments += $AppArgs | Where-Object { $_ -ne '--' } }

    $configLabel = if ($RealConfig) { 'real config' } else { $scratchConfig }
    Write-Host "Running DeskSpace ($profileDir, $configLabel). Ctrl+C or tray > Exit to stop." -ForegroundColor Green
    & $exe @arguments
    if ($LASTEXITCODE -ne 0) {
        Write-Host "DeskSpace exited with code $LASTEXITCODE" -ForegroundColor Red
    }
}
finally {
    Pop-Location
}
