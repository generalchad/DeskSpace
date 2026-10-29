<#
.SYNOPSIS
    Checks which spaces cast a drawn shadow, where, and how dark (ROADMAP
    D15, TASKS.md O2 and O3).

.DESCRIPTION
    Runs DeskSpace on a scratch config and checks the shadow windows'
    structure, not their pixels:

    - one visible DeskSpaceShadow per space that should cast one, directly
      below its space, and none for the spaces that should not (Clear, a
      desktop-hosted placement, the desktop layer, or show_shadow off);
    - the shadow's rect against ShadowLook::rect at the space's DPI, including
      a space with its own shadow_size;
    - its extended style (layered, click-through, never activated, a tool
      window);
    - WindowFromPoint through the band beside and below the space, which must
      reach the backdrop rather than the shadow.

    With -Pixels it also reads the drawn alpha over a white backdrop, for two
    300 px Solid spaces per run: half the shadow_opacity beside the space and
    three quarters below it (the drop), no pixel darker than the setting, both
    falloffs reaching 0 by the reach, a 16 px corner thinner than a square
    one, opacity 0 leaving the screen untouched, and a Solid space's centre
    equal to its tint. Both runs show the backdrop through
    Start-DeskSpace -BackdropColor, which puts it under the spaces.

    Which spaces cast one follows the code's rule: a chrome space that is not
    Clear, not hosted, and has show_shadow on. Under a composition backdrop
    (Windows 11 with "Transparency effects" on) DWM draws no shadow, so every
    such space gets one. An accent space (Windows 10, or transparency off)
    keeps DWM's shadow where DWM rounds the corners (radii 4 and 8 on Windows
    11), so it gets a drawn one only at radius 0 and above 8; Windows 10 has
    no DWM rounding, so every radius gets one.

    The lock screen covers everything a check reads, so a locked workstation
    is reported NOT COVERED and the script returns at once.

    Run from the repository root: ./scripts/onscreen/shadows.ps1 [-Pixels]
#>
[CmdletBinding()]
param(
    # Also check the shadow's pixels (TASKS.md O3).
    [switch]$Pixels
)

$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'OnScreen.psm1') -Force

$GW_HWNDNEXT = 2
$EX_LAYERED = 0x00080000
$EX_TRANSPARENT = 0x00000020
$EX_NOACTIVATE = 0x08000000
$EX_TOOLWINDOW = 0x00000080
$BACKDROP_CLASS = 'DeskSpaceOnScreenBackdrop'
$SHADOW_CLASS = 'DeskSpaceShadow'
$SPACE_CLASS = 'DeskSpaceSpace'

function Get-AlphaAt {
    # Alpha of one pixel of a Get-ScreenPixels capture over the white backdrop.
    param([psobject]$Pixels, [int]$X, [int]$Y)
    $index = ($Y * $Pixels.Width + $X) * 4
    1.0 - ($Pixels.Bgr[$index + 2] / 255.0)
}

function Get-Bit {
    param([long]$Value, [long]$Flag)
    ($Value -band $Flag) -eq $Flag
}

function Get-Reach {
    # ShadowLook::reach: the look's size in 96-DPI pixels, scaled and rounded.
    param([int]$Size, [int]$Dpi)
    [Math]::Max(1, [int][Math]::Round($Size * ($Dpi / 96.0), [MidpointRounding]::AwayFromZero))
}

function New-ShadowConfig {
    param(
        [string]$Body,
        [bool]$ShowShadow = $true,
        [int]$GlobalShadowSize = 16,
        [string]$Backdrop = 'solid',
        [int]$Radius = 8
    )
    @"
version = 2

[theme]
backdrop = "$Backdrop"
corner_radius = $Radius
show_shadow = $($ShowShadow.ToString().ToLower())
shadow_size = $GlobalShadowSize

[window.animations.startup_exit]
effect = "none"
$Body
"@
}

function New-SpaceBlock {
    param(
        [int]$Id,
        [string]$Title,
        [int]$X,
        [int]$Y,
        [int]$Width,
        [int]$Height,
        [string]$Backdrop,
        [int]$Radius,
        [int]$ShadowSize = 0,
        [string]$Tint = '',
        [int]$Opacity = -1,
        [switch]$Plain
    )
    if ($Plain) {
        return @"

[[desktop.spaces]]
id = $Id
title = "$Title"
rect = { x = $X, y = $Y, width = $Width, height = $Height }
user_sized = true
"@
    }
    $theme = "backdrop = `"$Backdrop`", corner_radius = $Radius"
    if ($Tint) {
        $theme += ", custom_colors = true, tint = { $Tint }"
    }
    if ($Opacity -ge 0) {
        $theme += ", shadow_opacity = $Opacity"
    }
    if ($ShadowSize -gt 0) {
        $theme += ", shadow_size = $ShadowSize"
    }
    @"

[[desktop.spaces]]
id = $Id
title = "$Title"
rect = { x = $X, y = $Y, width = $Width, height = $Height }
user_sized = true
custom = true
look = { theme = { $theme } }
"@
}

function Get-WindowClassAt {
    param([int]$X, [int]$Y)
    $point = New-Object DeskSpaceOnScreen+POINT
    $point.X = $X
    $point.Y = $Y
    [DeskSpaceOnScreen]::ClassOf([DeskSpaceOnScreen]::WindowFromPoint($point))
}

if ([DeskSpaceOnScreen]::IsWorkstationLocked()) {
    Assert-Check 'the workstation is unlocked' $false 'the lock screen is up; unlock and rerun' -NotCovered
    Complete-Checks
    return
}

# Which side of D15 this machine checks: composition when Windows 11 has
# "Transparency effects" on, else the accent path.
$personalize = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
$transparencyValue = (Get-ItemProperty -Path $personalize -Name EnableTransparency -ErrorAction SilentlyContinue).EnableTransparency
$transparency = if ($null -eq $transparencyValue) { $true } else { $transparencyValue -ne 0 }
$win11 = [Environment]::OSVersion.Version.Build -ge 22000
$composition = $transparency -and $win11

function Test-Casts {
    param([string]$Backdrop, [int]$Radius)
    if ($Backdrop -eq 'clear') { return $false }
    if ($composition) { return $true }
    if (-not $win11) { return $true }
    # Accent on Windows 11: DWM rounds 4 and 8 and casts its own shadow there.
    $radius -eq 0 -or ($radius -ne 4 -and $radius -ne 8)
}

$backdrops = 'acrylic', 'blur', 'solid', 'clear'
$radii = 0, 4, 8, 16
$cases = [System.Collections.Generic.List[object]]::new()
$body = ''
$id = 1
for ($row = 0; $row -lt $radii.Count; $row++) {
    for ($col = 0; $col -lt $backdrops.Count; $col++) {
        $backdrop = $backdrops[$col]
        $radius = $radii[$row]
        $x = 20 + $col * 180
        $y = 20 + $row * 130
        $cases.Add([pscustomobject]@{
                Id       = $id
                Title    = "$backdrop r$radius"
                X        = $x
                Y        = $y
                Backdrop = $backdrop
                Radius   = $radius
                Shadow   = 16
                Casts    = (Test-Casts $backdrop $radius)
            })
        $body += New-SpaceBlock $id "$backdrop r$radius" $x $y 160 110 $backdrop $radius
        $id++
    }
}
# Two spaces below the grid: one with its own shadow size, one with the global
# one, so each shadow's reach can be compared with its own theme.
$cases.Add([pscustomobject]@{
        Id = $id; Title = 'size 40'; X = 20; Y = 560
        Backdrop = 'solid'; Radius = 16; Shadow = 40; Casts = (Test-Casts 'solid' 16)
    })
$body += New-SpaceBlock $id 'size 40' 20 560 160 110 'solid' 16 40
$id++
$cases.Add([pscustomobject]@{
        Id = $id; Title = 'size 16'; X = 200; Y = 560
        Backdrop = 'solid'; Radius = 16; Shadow = 16; Casts = (Test-Casts 'solid' 16)
    })
$body += New-SpaceBlock $id 'size 16' 200 560 160 110 'solid' 16
$id++

try {
    Start-DeskSpace -Config (New-ShadowConfig -Body $body) -BackdropColor 'ffffff' | Out-Null
    Start-Sleep -Milliseconds 400

    $windows = @(Get-DeskSpaceWindows)
    $spaces = @($windows | Where-Object { $_.Class -eq $SPACE_CLASS -and $_.Visible -and -not $_.Child })
    $shadows = @($windows | Where-Object { $_.Class -eq $SHADOW_CLASS -and $_.Visible })
    $expectedCasts = @($cases | Where-Object { $_.Casts })

    $half = if ($composition) { 'composition' } else { 'accent' }
    $missing = @($cases | Where-Object {
            $case = $_
            -not ($spaces | Where-Object {
                    $_.Left -eq $case.X -and $_.Top -eq $case.Y -and
                    $_.Width -eq 160 -and $_.Height -eq 110
                })
        })
    Assert-Check 'every case has its window' ($missing.Count -eq 0) (
        "$($spaces.Count) spaces, $half path"
    )

    $wrong = [System.Collections.Generic.List[string]]::new()
    foreach ($case in $cases) {
        $space = $spaces | Where-Object {
            $_.Left -eq $case.X -and $_.Top -eq $case.Y -and $_.Width -eq 160
        } | Select-Object -First 1
        if (-not $space) { $wrong.Add("$($case.Title): no window"); continue }
        $below = [DeskSpaceOnScreen]::GetWindow($space.Handle, $GW_HWNDNEXT)
        $belowIsShadow = [DeskSpaceOnScreen]::ClassOf($below) -eq $SHADOW_CLASS
        $belowVisible = [DeskSpaceOnScreen]::IsWindowVisible($below)
        $hasShadow = $belowIsShadow -and $belowVisible
        if ($hasShadow -ne $case.Casts) {
            $wrong.Add("$($case.Title): expected $(if ($case.Casts) { 'a shadow' } else { 'none' })")
        }
    }
    $castsDetail = if ($wrong.Count -eq 0) {
        "$($expectedCasts.Count) of $($cases.Count) cast one"
    } else { $wrong -join '; ' }
    Assert-Check 'each space casts per the D15 rules' ($wrong.Count -eq 0) $castsDetail

    Assert-Check 'one shadow per casting space' ($shadows.Count -eq $expectedCasts.Count) (
        "$($shadows.Count) shadows, $($expectedCasts.Count) expected"
    )

    # Every shadow sits directly below a space: the window above it is one.
    $misplaced = @($shadows | Where-Object {
            $above = [DeskSpaceOnScreen]::GetWindow($_.Handle, 3)
            [DeskSpaceOnScreen]::ClassOf($above) -ne $SPACE_CLASS
        })
    Assert-Check 'each shadow sits directly below its space' ($misplaced.Count -eq 0) (
        "$($misplaced.Count) misplaced"
    )

    $badRects = [System.Collections.Generic.List[string]]::new()
    $badStyles = [System.Collections.Generic.List[string]]::new()
    $badClicks = [System.Collections.Generic.List[string]]::new()
    foreach ($case in $expectedCasts) {
        $space = $spaces | Where-Object {
            $_.Left -eq $case.X -and $_.Top -eq $case.Y -and $_.Width -eq 160
        } | Select-Object -First 1
        if (-not $space) { continue }
        $shadowHandle = [DeskSpaceOnScreen]::GetWindow($space.Handle, $GW_HWNDNEXT)
        $shadow = $shadows | Where-Object { $_.Handle -eq $shadowHandle } | Select-Object -First 1
        if (-not $shadow) { continue }

        $reach = Get-Reach $case.Shadow $space.Dpi
        $drop = [int][Math]::Floor($reach / 4)
        $expected = @(
            ($space.Left - $reach),
            ($space.Top - $reach + $drop),
            ($space.Width + 2 * $reach),
            ($space.Height + 2 * $reach)
        )
        $actual = @($shadow.Left, $shadow.Top, $shadow.Width, $shadow.Height)
        if (($expected -join ',') -ne ($actual -join ',')) {
            $badRects.Add("$($case.Title): ($($actual -join ',')) != ($($expected -join ','))")
        }

        $ex = $shadow.ExStyle
        if (-not ((Get-Bit $ex $EX_LAYERED) -and (Get-Bit $ex $EX_TRANSPARENT) -and
                (Get-Bit $ex $EX_NOACTIVATE) -and (Get-Bit $ex $EX_TOOLWINDOW))) {
            $badStyles.Add("$($case.Title): ex-style 0x$('{0:X}' -f $ex)")
        }

        $midY = [int](($space.Top + $space.Bottom) / 2)
        $midX = [int](($space.Left + $space.Right) / 2)
        foreach ($point in @(
                @(($space.Left - 1), $midY),
                @($midX, ($space.Bottom + 1))
            )) {
            $class = Get-WindowClassAt $point[0] $point[1]
            if ($class -ne $BACKDROP_CLASS) {
                $badClicks.Add("$($case.Title) at ($($point[0]),$($point[1])): $class")
            }
        }
    }
    $rectsDetail = if ($badRects.Count -eq 0) { 'reach, drop, and size match' } else { $badRects -join '; ' }
    Assert-Check 'each shadow has its look rect at the space DPI' ($badRects.Count -eq 0) $rectsDetail
    $stylesDetail = if ($badStyles.Count -eq 0) { 'layered, transparent, no-activate, tool window' } else { $badStyles -join '; ' }
    Assert-Check 'each shadow is layered and click-through' ($badStyles.Count -eq 0) $stylesDetail
    $clicksDetail = if ($badClicks.Count -eq 0) { 'both bands reach it' } else { $badClicks -join '; ' }
    Assert-Check 'the shadow band lets clicks reach the backdrop' ($badClicks.Count -eq 0) $clicksDetail

    $size40 = $spaces | Where-Object { $_.Left -eq 20 -and $_.Top -eq 560 } | Select-Object -First 1
    $size16 = $spaces | Where-Object { $_.Left -eq 200 -and $_.Top -eq 560 } | Select-Object -First 1
    $reach40 = if ($size40) { $shadows | Where-Object {
            $_.Handle -eq [DeskSpaceOnScreen]::GetWindow($size40.Handle, $GW_HWNDNEXT)
        } | Select-Object -First 1 } else { $null }
    $reach16 = if ($size16) { $shadows | Where-Object {
            $_.Handle -eq [DeskSpaceOnScreen]::GetWindow($size16.Handle, $GW_HWNDNEXT)
        } | Select-Object -First 1 } else { $null }
    $ok = $false
    $detail = 'a space is missing'
    if ($reach40 -and $reach16 -and $size40 -and $size16) {
        $expected40 = Get-Reach 40 $size40.Dpi
        $expected16 = Get-Reach 16 $size16.Dpi
        $got40 = ($reach40.Width - $size40.Width) / 2
        $got16 = ($reach16.Width - $size16.Width) / 2
        $ok = $got40 -eq $expected40 -and $got16 -eq $expected16 -and $expected40 -gt $expected16
        $detail = "reach $got40 (own 40) and $got16 (global 16), expected $expected40 and $expected16"
    }
    Assert-Check 'a customized shadow_size uses its own theme' $ok $detail
} finally {
    Stop-DeskSpace
}

# Desktop-hosted placement: hosted windows get no materials, so no shadows.
try {
    $one = New-SpaceBlock 1 'hosted' 100 100 320 200 'solid' 8
    Start-DeskSpace -Config (New-ShadowConfig -Body $one) -AppArgs @('--placement', 'desktop-background') | Out-Null
    Start-Sleep -Milliseconds 400
    $windows = @(Get-DeskSpaceWindows)
    $spaces = @($windows | Where-Object { $_.Class -eq $SPACE_CLASS -and $_.Visible })
    $shadows = @($windows | Where-Object { $_.Class -eq $SHADOW_CLASS -and $_.Visible })
    $hosted = @($spaces | Where-Object { $_.Child })
    if ($hosted.Count -eq 0) {
        Assert-Check 'desktop placement casts none' $false 'no desktop host; spaces stayed top-level' -NotCovered
    } else {
        Assert-Check 'desktop placement casts none' ($shadows.Count -eq 0) (
            "$($shadows.Count) shadows, $($hosted.Count) hosted spaces"
        )
    }
} finally {
    Stop-DeskSpace
}

# The chrome-less desktop layer casts none either.
try {
    $one = New-SpaceBlock 1 'layer test' 100 100 320 200 'solid' 8
    Start-DeskSpace -Config (New-ShadowConfig -Body $one) -AppArgs @('--takeover-layer') | Out-Null
    Start-Sleep -Milliseconds 400
    $windows = @(Get-DeskSpaceWindows)
    $shadows = @($windows | Where-Object { $_.Class -eq $SHADOW_CLASS -and $_.Visible })
    $virtualWidth = [DeskSpaceOnScreen]::GetSystemMetrics(78)
    $virtualHeight = [DeskSpaceOnScreen]::GetSystemMetrics(79)
    $layer = $windows | Where-Object {
        $_.Width -eq $virtualWidth -and $_.Height -eq $virtualHeight -and $_.Visible
    } | Select-Object -First 1
    $layerDetail = if ($layer) {
        "$($layer.Width)x$($layer.Height) layer, $($shadows.Count) shadows"
    } else {
        "no $($virtualWidth)x$($virtualHeight) layer, $($shadows.Count) shadows"
    }
    $others = if ($layer) { @($spaces | Where-Object { $_.Handle -ne $layer.Handle }) } else { @() }
    $expected = if (Test-Casts 'solid' 8) { $others.Count } else { 0 }
    $layerBelow = if ($layer) {
        [DeskSpaceOnScreen]::ClassOf([DeskSpaceOnScreen]::GetWindow($layer.Handle, $GW_HWNDNEXT))
    } else { '' }
    Assert-Check 'the desktop layer casts none' (
        $null -ne $layer -and $layerBelow -ne $SHADOW_CLASS -and $shadows.Count -eq $expected
    ) $layerDetail
} finally {
    Stop-DeskSpace
}

# show_shadow off: no shadows at all.
try {
    $one = New-SpaceBlock 1 'no shadow' 100 100 320 200 'solid' 8 -Plain
    Start-DeskSpace -Config (New-ShadowConfig -Body $one -ShowShadow $false) | Out-Null
    Start-Sleep -Milliseconds 400
    $shadows = @(Get-DeskSpaceWindows | Where-Object {
            $_.Class -eq $SHADOW_CLASS -and $_.Visible
        })
    Assert-Check 'show_shadow off casts none' ($shadows.Count -eq 0) "$($shadows.Count) shadows"
} finally {
    Stop-DeskSpace
}

# O3: the shadow's pixels, over a white backdrop.
if ($Pixels) {
    $tint = 'r = 0, g = 0, b = 255, a = 255'
    # Two 300 px spaces per run, far enough apart that no shadow touches the
    # other's. A 300 px side keeps the corners (whose span is at most 2 *
    # reach + radius, 136 px at 125 %) clear of the middle of each edge, so
    # the side and bottom readings are a straight edge's.
    $xPositions = @(80, 620)
    $y = 120
    $sideLength = 300
    $sets = @(
        @(@{ Size = 16; Opacity = 40; Radius = 8 }, @{ Size = 16; Opacity = 100; Radius = 8 }),
        @(@{ Size = 48; Opacity = 40; Radius = 8 }, @{ Size = 48; Opacity = 100; Radius = 8 }),
        @(@{ Size = 16; Opacity = 0; Radius = 8 }, @{ Size = 16; Opacity = 100; Radius = 0 }),
        @(@{ Size = 16; Opacity = 100; Radius = 16 })
    )

    $darkWrong = [System.Collections.Generic.List[string]]::new()
    $fadeWrong = [System.Collections.Generic.List[string]]::new()
    $noneWrong = [System.Collections.Generic.List[string]]::new()
    $insideWrong = [System.Collections.Generic.List[string]]::new()
    $corner = @{}

    foreach ($set in $sets) {
        $body = ''
        $id = 1
        $cases = [System.Collections.Generic.List[object]]::new()
        for ($index = 0; $index -lt $set.Count; $index++) {
            $case = $set[$index]
            $case.X = $xPositions[$index]
            $case.Y = $y
            $case.Title = "size $($case.Size), opacity $($case.Opacity), radius $($case.Radius)"
            $body += New-SpaceBlock $id $case.Title $case.X $y $sideLength $sideLength `
                'solid' $case.Radius $case.Size -Tint $tint -Opacity $case.Opacity
            $cases.Add($case)
            $id++
        }

        try {
            Start-DeskSpace -Config (New-ShadowConfig -Body $body) -BackdropColor 'ffffff' | Out-Null
            Start-Sleep -Milliseconds 300
            $windows = @(Get-DeskSpaceWindows)
            $spaces = @($windows | Where-Object {
                    $_.Class -eq $SPACE_CLASS -and $_.Visible -and -not $_.Child
                })
            $shadows = @($windows | Where-Object { $_.Class -eq $SHADOW_CLASS -and $_.Visible })

            foreach ($case in $cases) {
                $space = $spaces | Where-Object {
                    $_.Left -eq $case.X -and $_.Top -eq $case.Y -and $_.Width -eq $sideLength
                } | Select-Object -First 1
                if (-not $space) { $darkWrong.Add("${case.Title}: no space"); continue }
                $reach = Get-Reach $case.Size $space.Dpi
                $drop = [int][Math]::Floor($reach / 4)
                $midX = [int](($space.Left + $space.Right) / 2)
                $midY = [int](($space.Top + $space.Bottom) / 2)

                if ($case.Opacity -eq 0) {
                    # The window may exist so a later change can show it, but
                    # it must be hidden and paint nothing.
                    $below = [DeskSpaceOnScreen]::GetWindow($space.Handle, $GW_HWNDNEXT)
                    if ([DeskSpaceOnScreen]::ClassOf($below) -eq $SHADOW_CLASS -and
                        [DeskSpaceOnScreen]::IsWindowVisible($below)) {
                        $noneWrong.Add("${case.Title}: a shadow shows")
                    }
                    $points = @(
                        @(($space.Left - 1), $midY),
                        @($midX, ($space.Top - 1)),
                        @($midX, ($space.Bottom + 1))
                    )
                    foreach ($point in $points) {
                        $one = Get-ScreenPixels -Rect ([pscustomobject]@{
                                Left = $point[0]; Top = $point[1]; Width = 1; Height = 1
                            })
                        $pixel = Get-Pixel -Pixels $one -X 0 -Y 0
                        if ($pixel.R -lt 250 -or $pixel.G -lt 250 -or $pixel.B -lt 250) {
                            $noneWrong.Add(
                                "${case.Title}: ($($point[0]),$($point[1])) is rgb($($pixel.R), $($pixel.G), $($pixel.B))"
                            )
                        }
                    }
                    continue
                }

                $shadowHandle = [DeskSpaceOnScreen]::GetWindow($space.Handle, $GW_HWNDNEXT)
                $shadow = $shadows | Where-Object { $_.Handle -eq $shadowHandle } | Select-Object -First 1
                if (-not $shadow) { $darkWrong.Add("${case.Title}: no shadow"); continue }

                $capture = Get-ScreenPixels -Rect ([pscustomobject]@{
                        Left = $shadow.Left; Top = $shadow.Top; Width = $shadow.Width; Height = $shadow.Height
                    })
                $localLeft = $space.Left - $shadow.Left
                $localTop = $space.Top - $shadow.Top
                $localRight = $localLeft + $space.Width
                $localBottom = $localTop + $space.Height
                $localMidX = [int](($localLeft + $localRight) / 2)
                $localMidY = [int](($localTop + $localBottom) / 2)
                $opacity = $case.Opacity / 100.0

                # A straight edge shows half the blur beside the space; the
                # drop lay a little of the box below it, so the bottom band
                # peaks around three quarters of the setting.
                $sideAlpha = Get-AlphaAt $capture ($localLeft - 1) $localMidY
                if ([Math]::Abs($sideAlpha - 0.5 * $opacity) -gt 0.09) {
                    $darkWrong.Add(
                        "${case.Title}: beside the space $([Math]::Round($sideAlpha, 3)), " +
                        "expected $([Math]::Round(0.5 * $opacity, 3))"
                    )
                }
                $span = 2 * $reach + [int][Math]::Ceiling($case.Radius)
                $bottomAlpha = 0.0
                for ($x = $localLeft + $span; $x -lt $localRight - $span; $x++) {
                    $bottomAlpha = [Math]::Max($bottomAlpha, (Get-AlphaAt $capture $x ($localBottom + 1)))
                }
                if ([Math]::Abs($bottomAlpha - 0.75 * $opacity) -gt 0.10) {
                    $darkWrong.Add(
                        "${case.Title}: below the space $([Math]::Round($bottomAlpha, 3)), " +
                        "expected $([Math]::Round(0.75 * $opacity, 3))"
                    )
                }

                # Nothing is darker than the setting: the backdrop is white
                # everywhere the shadow has faded, so a darker pixel means
                # another window or a shadow that did not fade.
                $peak = 0.0
                $peakX = -1
                $peakY = -1
                for ($py = 0; $py -lt $capture.Height; $py += 4) {
                    for ($px = 0; $px -lt $capture.Width; $px += 4) {
                        if ($px -ge $localLeft -and $px -lt $localRight -and
                            $py -ge $localTop -and $py -lt $localBottom) { continue }
                        $alpha = Get-AlphaAt $capture $px $py
                        if ($alpha -gt $peak) {
                            $peak = $alpha
                            $peakX = $px
                            $peakY = $py
                        }
                    }
                }
                if ($peak -gt $opacity + 0.02) {
                    $darkWrong.Add(
                        "${case.Title}: $([Math]::Round($peak, 3)) at ($peakX,$peakY) outside the space"
                    )
                }

                # The falloff only goes one way, and it is gone by the edge.
                $previous = 2.0
                for ($x = $localLeft - 1; $x -ge 0; $x--) {
                    $alpha = Get-AlphaAt $capture $x $localMidY
                    if ($alpha -gt $previous + 0.005) {
                        $fadeWrong.Add("${case.Title}: the left line rises at $x")
                        break
                    }
                    $previous = $alpha
                }
                if ($previous -gt 0.02) {
                    $fadeWrong.Add("${case.Title}: the left edge is $([Math]::Round($previous, 3))")
                }
                $previous = 2.0
                for ($nextY = $localBottom; $nextY -lt $capture.Height; $nextY++) {
                    $alpha = Get-AlphaAt $capture $localMidX $nextY
                    if ($alpha -gt $previous + 0.005) {
                        $fadeWrong.Add("${case.Title}: the bottom line rises at $nextY")
                        break
                    }
                    $previous = $alpha
                }
                if ($previous -gt 0.02) {
                    $fadeWrong.Add("${case.Title}: the bottom edge is $([Math]::Round($previous, 3))")
                }

                # Inside a Solid space nothing is darkened: the tint is the
                # tint, not the shadow showing through a cut-out that misses.
                $center = Get-ScreenPixels -Rect ([pscustomobject]@{
                        Left = $midX; Top = $midY; Width = 1; Height = 1
                    })
                $pixel = Get-Pixel -Pixels $center -X 0 -Y 0
                if ($pixel.R -gt 2 -or $pixel.G -gt 2 -or $pixel.B -lt 250) {
                    $insideWrong.Add("${case.Title}: center is rgb($($pixel.R), $($pixel.G), $($pixel.B))")
                }

                # Where a 16 px corner's arc crosses the diagonal: a square
                # corner's shadow is still there, a rounded one's is not.
                if ($case.Opacity -eq 100) {
                    $moved = [int][Math]::Round(16 * ($space.Dpi / 96.0) * (1 - 1 / [Math]::Sqrt(2)))
                    $cornerX = $localLeft - $moved
                    $cornerY = $localBottom + $drop + $moved
                    if ($cornerX -ge 0 -and $cornerY -lt $capture.Height) {
                        $corner[$case.Radius] = Get-AlphaAt $capture $cornerX $cornerY
                    }
                }
            }
        } finally {
            Stop-DeskSpace
        }
    }

    $darkDetail = if ($darkWrong.Count -eq 0) {
        'half the opacity beside, three quarters below, nothing darker'
    } else { $darkWrong -join '; ' }
    Assert-Check 'each shadow is as dark as its opacity allows' ($darkWrong.Count -eq 0) $darkDetail
    $fadeDetail = if ($fadeWrong.Count -eq 0) {
        'both lines only fall and end at 0'
    } else { $fadeWrong -join '; ' }
    Assert-Check 'alpha never rises outward and ends at 0' ($fadeWrong.Count -eq 0) $fadeDetail
    $noneDetail = if ($noneWrong.Count -eq 0) {
        'the shadow window is hidden and the backdrop is untouched'
    } else { $noneWrong -join '; ' }
    Assert-Check 'opacity 0 casts no shadow and darkens nothing' ($noneWrong.Count -eq 0) $noneDetail
    $insideDetail = if ($insideWrong.Count -eq 0) { 'the Solid tint is untouched' } else { $insideWrong -join '; ' }
    Assert-Check 'the cut-out leaves a Solid space undarkened' ($insideWrong.Count -eq 0) $insideDetail

    $cornerOk = $false
    $cornerDetail = 'the 0 px and 16 px corners were not both measured'
    if ($corner.ContainsKey(0) -and $corner.ContainsKey(16)) {
        $square = $corner[0]
        $round = $corner[16]
        $cornerOk = $square -ge 0.015 -and $round -le $square / 2
        $cornerDetail = "square $([Math]::Round($square, 3)), round $([Math]::Round($round, 3))"
    }
    Assert-Check 'a 16 px corner thins the shadow where a square one does not' $cornerOk $cornerDetail
}
Complete-Checks
