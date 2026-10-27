<#
.SYNOPSIS
    Checks which spaces cast a drawn shadow and where (ROADMAP D15, TASKS.md O2).

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

    Which spaces cast one follows the code's rule: a chrome space that is not
    Clear, not hosted, and has show_shadow on. Under a composition backdrop
    (Windows 11 with "Transparency effects" on) DWM draws no shadow, so every
    such space gets one. An accent space (Windows 10, or transparency off)
    keeps DWM's shadow where DWM rounds the corners (radii 4 and 8 on Windows
    11), so it gets a drawn one only at radius 0 and above 8; Windows 10 has
    no DWM rounding, so every radius gets one.

    Run from the repository root: ./scripts/onscreen/shadows.ps1
#>
[CmdletBinding()]
param()

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
    Start-DeskSpace -Config (New-ShadowConfig -Body $body) | Out-Null
    Show-Backdrop -Color 'ffffff'
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
    Stop-Backdrop
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

Complete-Checks
