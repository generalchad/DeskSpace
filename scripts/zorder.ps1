<#
.SYNOPSIS
    Shows which windows sit where in the z-order and which one takes the mouse.

.DESCRIPTION
    Lists, top first, every visible window of DeskSpace, the taskbars, and the
    desktop shell (Progman, WorkerW), with its z-order index, process,
    topmost flag, and rectangle. Then, for the center of each space window
    and each taskbar, it names the top-level window that a click there would
    reach (WindowFromPoint), and finally the foreground window.

    Run it while a problem is on screen, for example when a space or a
    taskbar stops taking clicks, or a drop shows the "no" cursor. A hit test
    that names a window other than the one at that spot points at what is
    covering it. A center covered by an ordinary app window reports that app,
    which is expected.

.EXAMPLE
    ./scripts/zorder.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'onscreen/OnScreen.psm1') -Force

# https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindow
$GW_HWNDNEXT = 2
# https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getancestor
$GA_ROOT = 2
$GWL_EXSTYLE = -20
$WS_EX_TOPMOST = 0x8

$shellClasses = '^(Shell_TrayWnd|Shell_SecondaryTrayWnd|Progman|WorkerW)$'
$hitTested = '^(DeskSpaceSpace|Shell_TrayWnd|Shell_SecondaryTrayWnd)$'
$deskSpace = @(Get-Process deskspace -ErrorAction SilentlyContinue | ForEach-Object Id)
if ($deskSpace.Count -eq 0) {
    Write-Warning 'DeskSpace is not running; only the shell windows are listed.'
}

function Get-Rect([IntPtr]$Window) {
    $rect = New-Object DeskSpaceOnScreen+RECT
    [void][DeskSpaceOnScreen]::GetWindowRect($Window, [ref]$rect)
    $rect
}

function Format-Window([IntPtr]$Window) {
    $rect = Get-Rect $Window
    $processId = 0
    [void][DeskSpaceOnScreen]::GetWindowThreadProcessId($Window, [ref]$processId)
    $process = (Get-Process -Id $processId -ErrorAction SilentlyContinue).ProcessName
    $exStyle = [DeskSpaceOnScreen]::GetWindowLongPtrW($Window, $GWL_EXSTYLE).ToInt64()
    $topmost = [int](($exStyle -band $WS_EX_TOPMOST) -ne 0)
    '{0,-10} {1,-24} {2,-12} topmost={3} rect=({4},{5})-({6},{7})' -f `
        ('0x{0:X}' -f $Window.ToInt64()), [DeskSpaceOnScreen]::ClassOf($Window), $process,
        $topmost, $rect.L, $rect.T, $rect.R, $rect.B
}

'--- z-order, top first ---'
$targets = @()
$window = [DeskSpaceOnScreen]::GetTopWindow([IntPtr]::Zero)
$index = 0
while ($window -ne [IntPtr]::Zero) {
    $class = [DeskSpaceOnScreen]::ClassOf($window)
    $owner = 0
    [void][DeskSpaceOnScreen]::GetWindowThreadProcessId($window, [ref]$owner)
    $listed = $class -match $shellClasses -or $deskSpace -contains $owner
    if ($listed -and [DeskSpaceOnScreen]::IsWindowVisible($window)) {
        '#{0,-4} {1}' -f $index, (Format-Window $window)
        if ($class -match $hitTested) {
            $targets += , @($class, $window)
        }
    }
    $window = [DeskSpaceOnScreen]::GetWindow($window, $GW_HWNDNEXT)
    $index++
}

'--- what a click at each center reaches ---'
foreach ($target in $targets) {
    $rect = Get-Rect $target[1]
    $point = New-Object DeskSpaceOnScreen+POINT
    $point.X = [int](($rect.L + $rect.R) / 2)
    $point.Y = [int](($rect.T + $rect.B) / 2)
    $hit = [DeskSpaceOnScreen]::GetAncestor([DeskSpaceOnScreen]::WindowFromPoint($point), $GA_ROOT)
    '{0} 0x{1:X} at ({2},{3}) -> {4}' -f $target[0], $target[1].ToInt64(), $point.X, $point.Y,
        (Format-Window $hit)
}

'--- foreground ---'
Format-Window ([DeskSpaceOnScreen]::GetForegroundWindow())
