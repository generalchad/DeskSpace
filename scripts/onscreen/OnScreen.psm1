<#
.SYNOPSIS
    Runs DeskSpace against a scratch configuration and reads the screen.

.DESCRIPTION
    The shared module for the on-screen checks in `docs/TASKS.md` section O.
    Every check imports it, starts DeskSpace with a config of its own, puts a
    plain backdrop behind the `--on-top` spaces, and reads pixels or window
    facts back. Nothing here changes a Windows setting, and every process and
    window it makes is removed by Stop-DeskSpace and Stop-Backdrop.

    The P/Invoke class lives here, once, and `scripts/zorder.ps1` imports it
    too, so there is one copy of the Win32 declarations.

    See `docs/TASKS.md` section O for the checks that use it.
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;

public static class DeskSpaceOnScreen {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct SIZE { public int CX, CY; }
  [StructLayout(LayoutKind.Sequential)] public struct PAINTSTRUCT {
    public IntPtr hdc;
    public int fErase;
    public RECT rcPaint;
    public int fRestore;
    public int fIncUpdate;
    [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)] public byte[] rgbReserved;
  }
  [StructLayout(LayoutKind.Sequential)] public struct BITMAPINFOHEADER {
    public uint biSize;
    public int biWidth;
    public int biHeight;
    public ushort biPlanes;
    public ushort biBitCount;
    public uint biCompression;
    public uint biSizeImage;
    public int biXPelsPerMeter;
    public int biYPelsPerMeter;
    public uint biClrUsed;
    public uint biClrImportant;
  }
  [StructLayout(LayoutKind.Sequential)] public struct BITMAPINFO {
    public BITMAPINFOHEADER bmiHeader;
    public uint bmiColors0;
  }
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] public struct WNDCLASSW {
    public uint style;
    public IntPtr lpfnWndProc;
    public int cbClsExtra;
    public int cbWndExtra;
    public IntPtr hInstance;
    public IntPtr hIcon;
    public IntPtr hCursor;
    public IntPtr hbrBackground;
    [MarshalAs(UnmanagedType.LPWStr)] public string lpszMenuName;
    [MarshalAs(UnmanagedType.LPWStr)] public string lpszClassName;
  }
  public delegate IntPtr WndProc(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

  [DllImport("user32.dll")] public static extern IntPtr GetTopWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint f);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
  [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr hdc);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern ushort RegisterClassW(ref WNDCLASSW c);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool UnregisterClassW(string name, IntPtr instance);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr CreateWindowExW(
    uint exStyle, string className, string windowName, uint style,
    int x, int y, int width, int height,
    IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);
  [DllImport("user32.dll")] public static extern bool DestroyWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
  [DllImport("user32.dll")] public static extern IntPtr DefWindowProcW(IntPtr h, uint msg, IntPtr wParam, IntPtr lParam);
  [DllImport("user32.dll")] public static extern IntPtr BeginPaint(IntPtr h, out PAINTSTRUCT ps);
  [DllImport("user32.dll")] public static extern bool EndPaint(IntPtr h, ref PAINTSTRUCT ps);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern int FillRect(IntPtr hdc, ref RECT r, IntPtr brush);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateSolidBrush(uint color);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleDC(IntPtr hdc);
  [DllImport("gdi32.dll")] public static extern bool DeleteDC(IntPtr hdc);
  [DllImport("gdi32.dll")] public static extern IntPtr SelectObject(IntPtr hdc, IntPtr obj);
  [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr obj);
  [DllImport("gdi32.dll")] public static extern bool BitBlt(
    IntPtr dest, int x, int y, int width, int height,
    IntPtr source, int sourceX, int sourceY, uint rop);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateDIBSection(
    IntPtr hdc, ref BITMAPINFO info, uint usage, out IntPtr bits, IntPtr section, uint offset);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleBitmap(IntPtr hdc, int width, int height);
  [DllImport("gdi32.dll", SetLastError = true)] public static extern int GetDIBits(
    IntPtr hdc, IntPtr bitmap, uint start, uint lines, IntPtr bits, ref BITMAPINFO info, uint usage);
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr GetModuleHandleW(string name);

  public static string ClassOf(IntPtr h) {
    var text = new StringBuilder(256);
    GetClassNameW(h, text, 256);
    return text.ToString();
  }
}
"@

# https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindow
$script:GW_HWNDNEXT = 2
$script:GWL_EXSTYLE = -20
$script:WS_EX_TOPMOST = 0x8
$script:WS_POPUP = [uint32]2147483648
$script:WS_VISIBLE = [uint32]268435456
$script:WS_EX_NOACTIVATE = [uint32]134217728
$script:WS_EX_TOOLWINDOW = [uint32]128
$script:SW_SHOWNOACTIVATE = 4
$script:WM_PAINT = 0x000F
$script:WM_ERASEBKGND = 0x0014
$script:SRCCOPY_CAPTUREBLT = 0x40CC0020 # SRCCOPY | CAPTUREBLT
$script:SPACE_CLASS = 'DeskSpaceSpace'
$script:BACKDROP_CLASS = 'DeskSpaceOnScreenBackdrop'
$script:BACKDROP_TITLE = 'DeskSpace on-screen backdrop'

# The running instance, its scratch folder, and the backdrop's state.
$script:Instance = $null
$script:Backdrop = $null
$script:BackdropBrush = [IntPtr]::Zero
$script:BackdropClassRegistered = $false
$script:Checks = [System.Collections.Generic.List[object]]::new()

# Resident with the module: a collected delegate would crash the window procedure.
$script:BackdropWndProc = [DeskSpaceOnScreen+WndProc] {
    param([IntPtr]$hWnd, [uint32]$msg, [IntPtr]$wParam, [IntPtr]$lParam)
    if ($msg -eq $script:WM_PAINT) {
        $paint = New-Object DeskSpaceOnScreen+PAINTSTRUCT
        $hdc = [DeskSpaceOnScreen]::BeginPaint($hWnd, [ref]$paint)
        $rect = New-Object DeskSpaceOnScreen+RECT
        [void][DeskSpaceOnScreen]::GetClientRect($hWnd, [ref]$rect)
        [void][DeskSpaceOnScreen]::FillRect($hdc, [ref]$rect, $script:BackdropBrush)
        [void][DeskSpaceOnScreen]::EndPaint($hWnd, [ref]$paint)
        return [IntPtr]::Zero
    }
    if ($msg -eq $script:WM_ERASEBKGND) {
        return [IntPtr]1
    }
    [DeskSpaceOnScreen]::DefWindowProcW($hWnd, $msg, $wParam, $lParam)
}

# A check whose branch this machine does not select is reported, never passed.
function Assert-Check {
    <#
    .SYNOPSIS
        Records one PASS, FAIL, or NOT COVERED row for Complete-Checks.
    #>
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][bool]$Ok,
        [string]$Detail = '',
        [switch]$NotCovered
    )
    $status = if ($NotCovered) { 'NOT COVERED' } elseif ($Ok) { 'PASS' } else { 'FAIL' }
    $script:Checks.Add([pscustomobject]@{ Name = $Name; Status = $status; Detail = $Detail })
}

function Complete-Checks {
    <#
    .SYNOPSIS
        Prints the checks' table and exits non-zero when anything failed.
    #>
    $nameWidth = 4
    foreach ($check in $script:Checks) {
        $nameWidth = [Math]::Max($nameWidth, $check.Name.Length)
    }
    $failed = $false
    foreach ($check in $script:Checks) {
        if ($check.Status -eq 'FAIL') { $failed = $true }
        '{0}  {1,-11}  {2}' -f $check.Name.PadRight($nameWidth), $check.Status, $check.Detail
    }
    if ($failed) { exit 1 }
}

function ConvertTo-ColorRef {
    # "#RRGGBB", "RRGGBB", or a number, to a COLORREF (0x00BBGGRR).
    param([Parameter(Mandatory)]$Color)
    if ($Color -is [int] -or $Color -is [long]) {
        $value = [int]$Color
        return [uint32]((($value -band 0xFF) -shl 16) -bor ($value -band 0xFF00) -bor (($value -shr 16) -band 0xFF))
    }
    $text = ([string]$Color).TrimStart('#')
    if ($text -notmatch '^[0-9a-fA-F]{6}$') {
        throw "a backdrop color must be RRGGBB or #RRGGBB: $Color"
    }
    $r = [Convert]::ToInt32($text.Substring(0, 2), 16)
    $g = [Convert]::ToInt32($text.Substring(2, 2), 16)
    $b = [Convert]::ToInt32($text.Substring(4, 2), 16)
    [uint32]($r -bor ($g -shl 8) -bor ($b -shl 16))
}

function Get-DeskSpaceWindows {
    <#
    .SYNOPSIS
        Every top-level window of the running DeskSpace, top of the z-order first.
    #>
    if (-not $script:Instance) {
        throw 'DeskSpace is not running; call Start-DeskSpace first.'
    }
    $pidOfInterest = [uint32]$script:Instance.Process.Id
    $windows = [System.Collections.Generic.List[object]]::new()
    $window = [DeskSpaceOnScreen]::GetTopWindow([IntPtr]::Zero)
    $index = 0
    while ($window -ne [IntPtr]::Zero) {
        $owner = [uint32]0
        [void][DeskSpaceOnScreen]::GetWindowThreadProcessId($window, [ref]$owner)
        if ($owner -eq $pidOfInterest) {
            $rect = New-Object DeskSpaceOnScreen+RECT
            [void][DeskSpaceOnScreen]::GetWindowRect($window, [ref]$rect)
            $exStyle = [DeskSpaceOnScreen]::GetWindowLongPtrW($window, $script:GWL_EXSTYLE).ToInt64()
            $windows.Add([pscustomobject]@{
                Class   = [DeskSpaceOnScreen]::ClassOf($window)
                Handle  = $window
                Visible = [DeskSpaceOnScreen]::IsWindowVisible($window)
                Left    = $rect.L
                Top     = $rect.T
                Right   = $rect.R
                Bottom  = $rect.B
                Width   = $rect.R - $rect.L
                Height  = $rect.B - $rect.T
                ExStyle = $exStyle
                Topmost = ($exStyle -band $script:WS_EX_TOPMOST) -ne 0
                Dpi     = [DeskSpaceOnScreen]::GetDpiForWindow($window)
                Z       = $index
            })
        }
        $window = [DeskSpaceOnScreen]::GetWindow($window, $script:GW_HWNDNEXT)
        $index++
    }
    $windows
}

function Show-Backdrop {
    <#
    .SYNOPSIS
        A plain, non-topmost window of one color over the primary monitor.

    .DESCRIPTION
        The `--on-top` spaces draw above it, so a check can read the space's
        own pixels against a known color. One backdrop is live at a time;
        Stop-Backdrop removes it.
    #>
    param([Parameter(Mandatory)][string]$Color)

    if ($script:Backdrop) { Stop-Backdrop }

    # Best effort: the coordinates and pixels agree only when this process
    # shares the spaces' per-monitor DPI awareness. An already-aware host
    # refuses the call, which is fine.
    [void][DeskSpaceOnScreen]::SetProcessDpiAwarenessContext([IntPtr](-4))

    $instance = [DeskSpaceOnScreen]::GetModuleHandleW($null)
    if (-not $script:BackdropClassRegistered) {
        $class = New-Object DeskSpaceOnScreen+WNDCLASSW
        $class.style = 0
        $class.lpfnWndProc = [System.Runtime.InteropServices.Marshal]::GetFunctionPointerForDelegate($script:BackdropWndProc)
        $class.hInstance = $instance
        $class.hbrBackground = [IntPtr]::Zero
        $class.lpszClassName = $script:BACKDROP_CLASS
        $atom = [DeskSpaceOnScreen]::RegisterClassW([ref]$class)
        if ($atom -eq 0) {
            throw "could not register $($script:BACKDROP_CLASS)"
        }
        $script:BackdropClassRegistered = $true
    }

    $script:BackdropBrush = [DeskSpaceOnScreen]::CreateSolidBrush((ConvertTo-ColorRef $Color))
    $width = [DeskSpaceOnScreen]::GetSystemMetrics(0)
    $height = [DeskSpaceOnScreen]::GetSystemMetrics(1)
    if ($width -le 0 -or $height -le 0) {
        throw 'could not read the primary monitor size'
    }

    $style = [uint32]($script:WS_POPUP -bor $script:WS_VISIBLE)
    $exStyle = [uint32]($script:WS_EX_NOACTIVATE -bor $script:WS_EX_TOOLWINDOW)
    $window = [DeskSpaceOnScreen]::CreateWindowExW(
        $exStyle, $script:BACKDROP_CLASS, $script:BACKDROP_TITLE, $style,
        0, 0, $width, $height,
        [IntPtr]::Zero, [IntPtr]::Zero, $instance, [IntPtr]::Zero)
    if ($window -eq [IntPtr]::Zero) {
        throw 'could not create the backdrop window'
    }
    [void][DeskSpaceOnScreen]::ShowWindow($window, $script:SW_SHOWNOACTIVATE)
    $script:Backdrop = $window
}

function Stop-Backdrop {
    <#
    .SYNOPSIS
        Removes the backdrop window Show-Backdrop made, if any.
    #>
    if ($script:Backdrop) {
        [void][DeskSpaceOnScreen]::DestroyWindow($script:Backdrop)
        $script:Backdrop = $null
    }
    if ($script:BackdropBrush -ne [IntPtr]::Zero) {
        [void][DeskSpaceOnScreen]::DeleteObject($script:BackdropBrush)
        $script:BackdropBrush = [IntPtr]::Zero
    }
}

function Start-DeskSpace {
    <#
    .SYNOPSIS
        Builds DeskSpace, starts it on a scratch config, and waits for its spaces.

    .DESCRIPTION
        The config is TOML text (or the path to a file), written to a fresh
        folder under %TEMP%. The app runs with `--config`, `--on-top`, and
        `--no-icons`, so the spaces draw above other windows and Explorer's
        desktop is untouched. Returns once every space window is visible and
        its rect has held still for 500 ms. Stop-DeskSpace ends it and removes
        the folder.
    #>
    param(
        [Parameter(Mandatory)][string]$Config,
        [int]$TimeoutSeconds = 30
    )

    if ($script:Instance) { Stop-DeskSpace }

    $repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    & (Join-Path $repo 'scripts/dev.ps1') -NoRun | Out-Null
    $exe = Join-Path $repo 'target/debug/deskspace.exe'
    if (-not (Test-Path $exe)) {
        throw "the build did not write $exe"
    }

    $folder = Join-Path $env:TEMP ('deskspace-onscreen-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force -Path $folder | Out-Null
    $configPath = Join-Path $folder 'deskspace.toml'
    if (Test-Path $Config -PathType Leaf) {
        $Config = Get-Content -Raw $Config
    }
    Set-Content -Path $configPath -Value $Config -Encoding utf8 -NoNewline

    $process = Start-Process -FilePath $exe -PassThru -ArgumentList @(
        '--config', $configPath, '--on-top', '--no-icons'
    )
    $script:Instance = [pscustomobject]@{
        Process    = $process
        Folder     = $folder
        ConfigPath = $configPath
    }

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $lastRects = $null
    $steadySince = $null
    while ((Get-Date) -lt $deadline) {
        if ($process.HasExited) {
            Stop-DeskSpace
            throw ('DeskSpace exited with code ' + $process.ExitCode + ' before its spaces showed')
        }
        $spaces = @(Get-DeskSpaceWindows | Where-Object {
                $_.Class -eq $script:SPACE_CLASS -and $_.Visible
            })
        if ($spaces.Count -gt 0) {
            $rects = ($spaces | ForEach-Object { "$($_.Handle):$($_.Left),$($_.Top),$($_.Width),$($_.Height)" }) -join ';'
            if ($rects -ne $lastRects) {
                $lastRects = $rects
                $steadySince = Get-Date
            } elseif (((Get-Date) - $steadySince).TotalMilliseconds -ge 500) {
                return $script:Instance
            }
        }
        Start-Sleep -Milliseconds 100
    }

    Stop-DeskSpace
    throw "DeskSpace's spaces did not settle within $TimeoutSeconds s"
}

function Stop-DeskSpace {
    <#
    .SYNOPSIS
        Ends the instance Start-DeskSpace made and deletes its scratch folder.
    #>
    if (-not $script:Instance) { return }
    try {
        if (-not $script:Instance.Process.HasExited) {
            Stop-Process -Id $script:Instance.Process.Id -Force
            [void]$script:Instance.Process.WaitForExit(5000)
        }
    } finally {
        Remove-Item -Recurse -Force $script:Instance.Folder -ErrorAction SilentlyContinue
        $script:Instance = $null
    }
}

function Get-ScreenPixels {
    <#
    .SYNOPSIS
        The screen's composed pixels in a rectangle, as BGR bytes.

    .DESCRIPTION
        Reads the desktop's own DC with BitBlt and CAPTUREBLT, so layered and
        composition windows are included when Windows lets them be. The result
        is a top-down 32-bit buffer: B, G, R, A per pixel. Use Get-Pixel for
        one pixel.
    #>
    param([Parameter(Mandatory)][psobject]$Rect)

    $left = [int]$Rect.Left
    $top = [int]$Rect.Top
    $width = [int]$Rect.Width
    $height = [int]$Rect.Height
    if ($width -le 0 -or $height -le 0) {
        throw "the capture rectangle is empty: ${width}x${height}"
    }

    $screen = [DeskSpaceOnScreen]::GetDC([IntPtr]::Zero)
    if ($screen -eq [IntPtr]::Zero) { throw 'could not read the screen' }
    $memory = [DeskSpaceOnScreen]::CreateCompatibleDC($screen)
    $bitmap = [DeskSpaceOnScreen]::CreateCompatibleBitmap($screen, $width, $height)
    if ($bitmap -eq [IntPtr]::Zero) {
        [void][DeskSpaceOnScreen]::DeleteDC($memory)
        [void][DeskSpaceOnScreen]::ReleaseDC([IntPtr]::Zero, $screen)
        throw 'could not make the capture bitmap'
    }

    try {
        $previous = [DeskSpaceOnScreen]::SelectObject($memory, $bitmap)
        $copied = [DeskSpaceOnScreen]::BitBlt(
            $memory, 0, 0, $width, $height,
            $screen, $left, $top, $script:SRCCOPY_CAPTUREBLT)
        if (-not $copied) {
            throw 'BitBlt could not read the screen'
        }

        # A struct nested in another is copied on property access, so build the
        # header on its own and assign it once.
        $header = New-Object DeskSpaceOnScreen+BITMAPINFOHEADER
        $header.biSize = [System.Runtime.InteropServices.Marshal]::SizeOf([type][DeskSpaceOnScreen+BITMAPINFOHEADER])
        $header.biWidth = $width
        # Negative: top-down rows, as Get-Pixel reads them.
        $header.biHeight = -$height
        $header.biPlanes = 1
        $header.biBitCount = 32
        $header.biCompression = 0
        $info = New-Object DeskSpaceOnScreen+BITMAPINFO
        $info.bmiHeader = $header
        $bytes = New-Object byte[] ($width * $height * 4)
        # GetDIBits refuses a bitmap that is still selected into a DC.
        [void][DeskSpaceOnScreen]::SelectObject($memory, $previous)
        $buffer = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($bytes.Length)
        try {
            $lines = [DeskSpaceOnScreen]::GetDIBits($screen, $bitmap, 0, $height, $buffer, [ref]$info, 0)
            if ($lines -ne $height) {
                $error = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
                throw "GetDIBits read $lines of $height rows (Win32 error $error)"
            }
            [System.Runtime.InteropServices.Marshal]::Copy($buffer, $bytes, 0, $bytes.Length)
        } finally {
            [System.Runtime.InteropServices.Marshal]::FreeHGlobal($buffer)
        }
    } finally {
        [void][DeskSpaceOnScreen]::DeleteObject($bitmap)
        [void][DeskSpaceOnScreen]::DeleteDC($memory)
        [void][DeskSpaceOnScreen]::ReleaseDC([IntPtr]::Zero, $screen)
    }

    [pscustomobject]@{ Width = $width; Height = $height; Top = $top; Left = $left; Bgr = $bytes }
}

function Get-Pixel {
    <#
    .SYNOPSIS
        One pixel of a Get-ScreenPixels result, as R, G, and B.
    #>
    param(
        [Parameter(Mandatory)][psobject]$Pixels,
        [Parameter(Mandatory)][int]$X,
        [Parameter(Mandatory)][int]$Y
    )
    if ($X -lt 0 -or $Y -lt 0 -or $X -ge $Pixels.Width -or $Y -ge $Pixels.Height) {
        throw "pixel ($X, $Y) is outside $($Pixels.Width)x$($Pixels.Height)"
    }
    $index = ($Y * $Pixels.Width + $X) * 4
    [pscustomobject]@{
        B = $Pixels.Bgr[$index]
        G = $Pixels.Bgr[$index + 1]
        R = $Pixels.Bgr[$index + 2]
        A = $Pixels.Bgr[$index + 3]
    }
}

Export-ModuleMember -Function Assert-Check, Complete-Checks, Get-DeskSpaceWindows, Get-Pixel,
    Get-ScreenPixels, Show-Backdrop, Start-DeskSpace, Stop-Backdrop, Stop-DeskSpace
