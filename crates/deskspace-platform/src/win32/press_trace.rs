//! Log lines that say where a press on a surface, or the desktop menu it
//! opens, really came from. A press can reach a space or the desktop layer
//! from over another program's window in two ways: through a mouse capture
//! the surface still holds, or because the layer, which spans the desktop,
//! rose above a taskbar. Both look the same from inside the window procedure.

use deskspace_core::geometry::Point;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::MapWindowPoints;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::GetCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    GW_HWNDPREV, GetClassNameW, GetWindow, GetWindowThreadProcessId, WindowFromPoint,
};

use crate::win32::debug_info::{TaskbarRow, taskbars};

/// The taskbar whose rectangle holds `point`, if any.
fn taskbar_under(taskbars: &[TaskbarRow], point: Point) -> Option<&TaskbarRow> {
    taskbars.iter().find(|taskbar| taskbar.rect.contains(point))
}

/// The window a click at `point` (screen coordinates) would reach, ignoring
/// any capture.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-windowfrompoint
fn window_at(point: Point) -> Option<HWND> {
    // SAFETY: a plain hit test; the handle is only read from.
    let window = unsafe {
        WindowFromPoint(POINT {
            x: point.x,
            y: point.y,
        })
    };
    (!window.is_invalid()).then_some(window)
}

fn class_of(window: HWND) -> String {
    let mut class = [0u16; 256];
    // SAFETY: reads a window's class into a local buffer sized for the call.
    let length = unsafe { GetClassNameW(window, &mut class) }.max(0) as usize;
    String::from_utf16_lossy(&class[..length])
}

/// Whether `window` belongs to another process than this one.
fn is_foreign(window: HWND) -> bool {
    let mut owner = 0;
    // SAFETY: plain queries; `owner` is a local.
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut owner));
        owner != GetCurrentProcessId()
    }
}

/// Logs where the right-click that opens the desktop menu at `at` (screen
/// coordinates) landed: the window a click there reaches, the taskbar that
/// holds the point and whether it is above `layer`, and the window just
/// above the layer. A taskbar below the layer, or a foreign window under the
/// point, names which way the click came.
pub(crate) fn log_desktop_menu(at: Point, layer: Option<HWND>) {
    if !tracing::enabled!(tracing::Level::DEBUG) {
        return;
    }
    let under = window_at(at);
    let rows = taskbars(layer);
    let taskbar = taskbar_under(&rows, at);
    // SAFETY: a plain z-order query on the live layer window.
    let above_layer = layer.and_then(|layer| unsafe { GetWindow(layer, GW_HWNDPREV) }.ok());
    tracing::debug!(
        x = at.x,
        y = at.y,
        under = under.map(class_of).as_deref().unwrap_or("none"),
        under_is_foreign = under.is_some_and(is_foreign),
        taskbar = taskbar.map(|taskbar| taskbar.class.as_str()),
        taskbar_above_layer = ?taskbar.and_then(|taskbar| taskbar.above_layer),
        above_layer = above_layer.map(class_of).as_deref().unwrap_or("none"),
        "opening the desktop menu"
    );
}

/// Logs a press on `window` at `client` (its client coordinates) when a
/// click there would reach another program's window, so the press came
/// through the mouse capture. `held_capture` says whether `window` held the
/// capture before this press, which only a press it never saw released
/// leaves behind. Called before the press takes the capture itself.
pub(crate) fn log_press_from_elsewhere(window: HWND, client: POINT) {
    if !tracing::enabled!(tracing::Level::DEBUG) {
        return;
    }
    let mut point = [client];
    // SAFETY: `window` is live; `point` is a valid one-element buffer.
    unsafe { MapWindowPoints(Some(window), None, &mut point) };
    let screen = Point::new(point[0].x, point[0].y);
    let Some(under) = window_at(screen).filter(|under| is_foreign(*under)) else {
        return;
    };
    // SAFETY: a plain query of this thread's capture.
    let held_capture = unsafe { GetCapture() } == window;
    tracing::debug!(
        x = screen.x,
        y = screen.y,
        under = %class_of(under),
        held_capture,
        "a press reached a surface from over another program's window"
    );
}

#[cfg(test)]
mod tests {
    use deskspace_core::geometry::Rect;

    use super::*;

    fn taskbar(class: &str, rect: Rect) -> TaskbarRow {
        TaskbarRow {
            class: class.into(),
            rect,
            topmost: true,
            above_layer: Some(true),
        }
    }

    #[test]
    fn the_taskbar_under_a_point_is_found_on_any_monitor() {
        let rows = [
            taskbar("Shell_TrayWnd", Rect::new(0, 1032, 1920, 48)),
            taskbar("Shell_SecondaryTrayWnd", Rect::new(1920, 1032, 1720, 48)),
        ];
        let main = taskbar_under(&rows, Point::new(960, 1050)).map(|row| row.class.as_str());
        let second = taskbar_under(&rows, Point::new(2500, 1079)).map(|row| row.class.as_str());
        assert_eq!(main, Some("Shell_TrayWnd"));
        assert_eq!(second, Some("Shell_SecondaryTrayWnd"));
    }

    #[test]
    fn a_point_off_every_taskbar_finds_none() {
        let rows = [taskbar("Shell_TrayWnd", Rect::new(0, 1032, 1920, 48))];
        assert!(taskbar_under(&rows, Point::new(960, 500)).is_none());
        // The bottom edge is outside, as `Rect::contains` has it.
        assert!(taskbar_under(&rows, Point::new(960, 1080)).is_none());
        assert!(taskbar_under(&[], Point::new(0, 0)).is_none());
    }

    #[test]
    fn this_processs_own_window_is_not_foreign() {
        // SAFETY: a message-only window of the system "Static" class, made
        // and destroyed on this thread.
        let window = unsafe {
            windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
                windows::core::w!("Static"),
                windows::core::w!(""),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(windows::Win32::UI::WindowsAndMessaging::HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .expect("a message-only window");
        let foreign = is_foreign(window);
        let class = class_of(window);
        // SAFETY: the window was created above on this thread.
        let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyWindow(window) };
        assert!(!foreign);
        assert_eq!(class, "Static");
    }
}
