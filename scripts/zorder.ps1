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

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class DeskSpaceZOrder {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] public static extern IntPtr GetTopWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint f);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  public static string ClassOf(IntPtr h) { var s = new StringBuilder(256); GetClassNameW(h, s, 256); return s.ToString(); }
}
"@

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
    $rect = New-Object DeskSpaceZOrder+RECT
    [void][DeskSpaceZOrder]::GetWindowRect($Window, [ref]$rect)
    $rect
}

function Format-Window([IntPtr]$Window) {
    $rect = Get-Rect $Window
    $processId = 0
    [void][DeskSpaceZOrder]::GetWindowThreadProcessId($Window, [ref]$processId)
    $process = (Get-Process -Id $processId -ErrorAction SilentlyContinue).ProcessName
    $exStyle = [DeskSpaceZOrder]::GetWindowLongPtrW($Window, $GWL_EXSTYLE).ToInt64()
    $topmost = [int](($exStyle -band $WS_EX_TOPMOST) -ne 0)
    '{0,-10} {1,-24} {2,-12} topmost={3} rect=({4},{5})-({6},{7})' -f `
        ('0x{0:X}' -f $Window.ToInt64()), [DeskSpaceZOrder]::ClassOf($Window), $process,
        $topmost, $rect.L, $rect.T, $rect.R, $rect.B
}

'--- z-order, top first ---'
$targets = @()
$window = [DeskSpaceZOrder]::GetTopWindow([IntPtr]::Zero)
$index = 0
while ($window -ne [IntPtr]::Zero) {
    $class = [DeskSpaceZOrder]::ClassOf($window)
    $owner = 0
    [void][DeskSpaceZOrder]::GetWindowThreadProcessId($window, [ref]$owner)
    $listed = $class -match $shellClasses -or $deskSpace -contains $owner
    if ($listed -and [DeskSpaceZOrder]::IsWindowVisible($window)) {
        '#{0,-4} {1}' -f $index, (Format-Window $window)
        if ($class -match $hitTested) {
            $targets += , @($class, $window)
        }
    }
    $window = [DeskSpaceZOrder]::GetWindow($window, $GW_HWNDNEXT)
    $index++
}

'--- what a click at each center reaches ---'
foreach ($target in $targets) {
    $rect = Get-Rect $target[1]
    $point = New-Object DeskSpaceZOrder+POINT
    $point.X = [int](($rect.L + $rect.R) / 2)
    $point.Y = [int](($rect.T + $rect.B) / 2)
    $hit = [DeskSpaceZOrder]::GetAncestor([DeskSpaceZOrder]::WindowFromPoint($point), $GA_ROOT)
    '{0} 0x{1:X} at ({2},{3}) -> {4}' -f $target[0], $target[1].ToInt64(), $point.X, $point.Y,
        (Format-Window $hit)
}

'--- foreground ---'
Format-Window ([DeskSpaceZOrder]::GetForegroundWindow())
