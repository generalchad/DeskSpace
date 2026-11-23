<#
.SYNOPSIS
    Measures DeskSpace's idle CPU usage and thread wake-ups per second
    (TASKS.md O5).

.DESCRIPTION
    Starts DeskSpace via Start-DeskSpace on a scratch configuration with five
    spaces and one lens on a quiet temporary folder, waits for initial listings
    to settle, then samples process CPU time (`TotalProcessorTime`) and
    per-thread context switches (`\Thread(deskspace*)\Context Switches/sec`).

    Fails and names the busiest thread if idle CPU meets or exceeds
    `-MaxCpuPercent` or average wake-ups per second meet or exceed
    `-MaxWakeupsPerSec`. Run only when no build is in progress (`Start-DeskSpace`
    runs `dev.ps1`, which stops a running DeskSpace).

    Run from the repository root: ./scripts/onscreen/idle.ps1
#>
[CmdletBinding()]
param(
    # First-guess thresholds (TASKS.md O5); tune if needed.
    [double]$MaxCpuPercent = 0.1,
    [double]$MaxWakeupsPerSec = 5.0,
    [int]$SettleSeconds = 15,
    [int]$SampleSeconds = 60,
    [int]$SampleIntervalSeconds = 1
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'OnScreen.psm1') -Force

if ($SampleSeconds -le 0) { throw 'SampleSeconds must be positive.' }
if ($SampleIntervalSeconds -le 0) { throw 'SampleIntervalSeconds must be positive.' }

$lensFolder = Join-Path $env:TEMP ('deskspace-idle-lens-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $lensFolder | Out-Null
Set-Content -Path (Join-Path $lensFolder 'alpha.txt') -Value 'alpha' -Encoding utf8 -NoNewline
Set-Content -Path (Join-Path $lensFolder 'beta.txt') -Value 'beta' -Encoding utf8 -NoNewline
$lensTomlPath = $lensFolder.Replace('\', '/')

$config = @"
version = 2

[window.animations.startup_exit]
effect = "none"

[[desktop.spaces]]
id = 1
title = "Idle 1"
rect = { x = 60, y = 60, width = 220, height = 160 }
user_sized = true

[[desktop.spaces]]
id = 2
title = "Idle 2"
rect = { x = 300, y = 60, width = 220, height = 160 }
user_sized = true

[[desktop.spaces]]
id = 3
title = "Idle 3"
rect = { x = 540, y = 60, width = 220, height = 160 }
user_sized = true

[[desktop.spaces]]
id = 4
title = "Idle 4"
rect = { x = 60, y = 240, width = 220, height = 160 }
user_sized = true

[[desktop.spaces]]
id = 5
title = "Idle 5"
rect = { x = 300, y = 240, width = 220, height = 160 }
user_sized = true

[[desktop.spaces]]
id = 6
title = "Quiet lens"
rect = { x = 540, y = 240, width = 260, height = 180 }
user_sized = true
source = { folder = { path = "$lensTomlPath" } }
"@

$cpuPercent = [double]::NaN
$cpuDeltaMs = [double]::NaN
$elapsedSeconds = [double]::NaN
$avgWakeupsPerSec = [double]::NaN
$busiestLabel = 'unknown'
$busiestRate = 0.0
$threadSummary = @()

try {
    $instance = Start-DeskSpace -Config $config

    $spaces = @(Get-DeskSpaceWindows | Where-Object {
            $_.Class -eq 'DeskSpaceSpace' -and $_.Visible
        })
    Assert-Check 'five spaces and one lens are visible' ($spaces.Count -eq 6) "$($spaces.Count) of 6 visible"
    if ($spaces.Count -lt 6) {
        Complete-Checks
        return
    }

    Write-Output "Waiting ${SettleSeconds}s for listings to settle..."
    Start-Sleep -Seconds $SettleSeconds

    $proc = $instance.Process
    $proc.Refresh()
    if ($proc.HasExited) {
        throw "DeskSpace exited with code $($proc.ExitCode) before sampling started."
    }

    $cpuStart = $proc.TotalProcessorTime
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $rawSamples = @(Get-Counter -Counter @(
        '\Thread(deskspace*)\Context Switches/sec',
        '\Thread(deskspace*)\ID Thread'
    ) -SampleInterval $SampleIntervalSeconds -MaxSamples ($SampleSeconds + 1) -ErrorAction SilentlyContinue)
    $stopwatch.Stop()

    $proc.Refresh()
    if ($proc.HasExited) {
        throw "DeskSpace exited with code $($proc.ExitCode) during sampling."
    }
    $cpuEnd = $proc.TotalProcessorTime

    $elapsedSeconds = $stopwatch.Elapsed.TotalSeconds
    $cpuDeltaMs = ($cpuEnd - $cpuStart).TotalMilliseconds
    $cpuPercent = (($cpuEnd - $cpuStart).TotalSeconds / $elapsedSeconds) * 100.0

    # Sample 0 is the baseline for rate counters; skip it so every kept sample
    # covers a full interval.
    $rateSamples = @($rawSamples | Select-Object -Skip 1)
    if ($rateSamples.Count -eq 0) {
        throw 'No Thread(deskspace*) rate counter samples were collected.'
    }

    $tickTotals = [System.Collections.Generic.List[double]]::new()
    $byThreadKey = @{}

    foreach ($sample in $rateSamples) {
        $idsByInstance = @{}
        $switchesByInstance = @{}
        foreach ($counter in $sample.CounterSamples) {
            $regexMatch = [regex]::Match(
                $counter.Path,
                '\\thread\((deskspace/\d+)\)\\(context switches/sec|id thread)$',
                [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
            )
            if (-not $regexMatch.Success) { continue }
            $inst = $regexMatch.Groups[1].Value.ToLowerInvariant()
            $metric = $regexMatch.Groups[2].Value.ToLowerInvariant()
            if ($metric -eq 'id thread') {
                $idsByInstance[$inst] = [uint32]$counter.CookedValue
            } else {
                $switchesByInstance[$inst] = [double]$counter.CookedValue
            }
        }

        $tickSum = 0.0
        foreach ($inst in $switchesByInstance.Keys) {
            $value = [double]$switchesByInstance[$inst]
            $tickSum += $value
            $tid = if ($idsByInstance.ContainsKey($inst) -and $idsByInstance[$inst] -gt 0) {
                $idsByInstance[$inst]
            } else {
                [uint32]0
            }
            $key = if ($tid -gt 0) { "tid:$tid" } else { "inst:$inst" }
            if (-not $byThreadKey.ContainsKey($key)) {
                $description = ''
                if ($tid -gt 0) {
                    $description = [DeskSpaceOnScreen]::ThreadDescription($tid)
                }
                if ([string]::IsNullOrWhiteSpace($description)) {
                    $description = if ($inst -eq 'deskspace/0') { 'main (unnamed)' } else { 'unnamed' }
                }
                $byThreadKey[$key] = [pscustomobject]@{
                    Key         = $key
                    ThreadId    = $tid
                    Instance    = $inst
                    Description = $description
                    Sum         = 0.0
                }
            }
            $byThreadKey[$key].Sum += $value
        }
        $tickTotals.Add($tickSum)
    }

    $avgWakeupsPerSec = ($tickTotals | Measure-Object -Average).Average
    $threadSummary = @(
        $byThreadKey.Values |
            ForEach-Object {
                [pscustomobject]@{
                    Description = $_.Description
                    Instance    = $_.Instance
                    ThreadId    = $_.ThreadId
                    WakeupsSec  = $_.Sum / $rateSamples.Count
                }
            } |
            Sort-Object -Property WakeupsSec -Descending
    )

    if ($threadSummary.Count -gt 0) {
        $top = $threadSummary[0]
        $busiestLabel = "$($top.Description) ($($top.Instance), tid $($top.ThreadId))"
        $busiestRate = $top.WakeupsSec
    }

    Write-Output ('Idle sample ({0:F2}s, {1} samples, pid {2}):' -f $elapsedSeconds, $rateSamples.Count, $proc.Id)
    Write-Output ('  CPU:            {0:F4}% ({1:F1} ms TotalProcessorTime)' -f $cpuPercent, $cpuDeltaMs)
    Write-Output ('  Wake-ups/sec:   {0:F2}/s total context switches' -f $avgWakeupsPerSec)
    Write-Output ('  Busiest thread: {0} at {1:F2}/s' -f $busiestLabel, $busiestRate)
    foreach ($entry in $threadSummary) {
        Write-Output ('    - {0,-28} ({1}, tid {2}): {3:F2}/s' -f $entry.Description, $entry.Instance, $entry.ThreadId, $entry.WakeupsSec)
    }
} finally {
    Stop-DeskSpace
    Remove-Item -Recurse -Force $lensFolder -ErrorAction SilentlyContinue
}

Assert-Check 'idle CPU under threshold' ($cpuPercent -lt $MaxCpuPercent) (
    ('{0:F4}% (limit < {1}%; busiest thread: {2} at {3:F2}/s)' -f $cpuPercent, $MaxCpuPercent, $busiestLabel, $busiestRate)
)
Assert-Check 'idle wake-ups under threshold' ($avgWakeupsPerSec -lt $MaxWakeupsPerSec) (
    ('{0:F2}/s (limit < {1}/s; busiest thread: {2} at {3:F2}/s)' -f $avgWakeupsPerSec, $MaxWakeupsPerSec, $busiestLabel, $busiestRate)
)

Complete-Checks
