<#
.SYNOPSIS
    Proves the on-screen harness can read a space's pixels.

.DESCRIPTION
    Starts DeskSpace on a scratch config with one Solid, opaque red space, puts
    a white backdrop behind it, and reads the composed screen: red at the
    space's center, white well away from it. If the space's own pixels cannot
    be read (composition content missing from a BitBlt capture), the check
    fails on purpose; `TASKS.md` O1 says to stop and ask before reaching for
    Windows.Graphics.Capture.

    Run from the repository root: ./scripts/onscreen/smoke.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'OnScreen.psm1') -Force

# One Solid space, kept at its configured size (an empty space that is not
# user-sized would fit itself), with no animation.
$config = @'
version = 2

[theme]
backdrop = "solid"
custom_colors = true
tint = { r = 255, g = 0, b = 0, a = 255 }
tint_opacity = 100

[window.animations.startup_exit]
effect = "none"

[[desktop.spaces]]
id = 1
title = "Smoke"
rect = { x = 100, y = 100, width = 400, height = 300 }
user_sized = true
'@

try {
    Start-DeskSpace -Config $config -BackdropColor 'ffffff' | Out-Null
    Start-Sleep -Milliseconds 400

    $spaces = @(Get-DeskSpaceWindows | Where-Object {
            $_.Class -eq 'DeskSpaceSpace' -and $_.Visible
        })
    Assert-Check 'a space window is visible' ($spaces.Count -ge 1) "$($spaces.Count) seen"

    if ($spaces.Count -eq 0) {
        Complete-Checks
        return
    }

    $space = $spaces[0]
    Assert-Check 'the space keeps its configured rect' (
        $space.Left -eq 100 -and $space.Top -eq 100 -and
        $space.Width -eq 400 -and $space.Height -eq 300
    ) "rect ($($space.Left),$($space.Top)) $($space.Width)x$($space.Height)"

    $centerX = [int](($space.Left + $space.Right) / 2)
    $centerY = [int](($space.Top + $space.Bottom) / 2)
    $inside = Get-ScreenPixels -Rect ([pscustomobject]@{
            Left = $centerX; Top = $centerY; Width = 1; Height = 1
        })
    $red = Get-Pixel -Pixels $inside -X 0 -Y 0
    Assert-Check 'the space paints its tint' (
        $red.R -ge 200 -and $red.G -le 60 -and $red.B -le 60
    ) "center is rgb($($red.R), $($red.G), $($red.B))"

    $away = Get-ScreenPixels -Rect ([pscustomobject]@{
            Left = 20; Top = 20; Width = 1; Height = 1
        })
    $white = Get-Pixel -Pixels $away -X 0 -Y 0
    Assert-Check 'the backdrop shows away from the space' (
        $white.R -ge 235 -and $white.G -ge 235 -and $white.B -ge 235
    ) "corner is rgb($($white.R), $($white.G), $($white.B))"
} finally {
    Stop-DeskSpace
}

Complete-Checks
