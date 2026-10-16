//! Window-class registration shared by every DeskSpace window.
//!
//! A module describes its class with a `WindowClass` and calls `register`
//! before each `CreateWindowExW`. Registering an existing class is not an
//! error, so no "registered once" static is needed beside each class.
//!
//! <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerclassw>

use windows::Win32::Foundation::{ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    IDC_ARROW, LoadCursorW, RegisterClassW, WNDCLASS_STYLES, WNDCLASSW, WNDPROC,
};
use windows::core::PCWSTR;

use crate::error::PlatformError;

/// What a window class needs. Classes get no background brush: every
/// DeskSpace window erases or paints its whole client itself.
#[derive(Clone, Copy)]
pub(crate) struct WindowClass {
    /// A static class name, from `w!`.
    name: PCWSTR,
    procedure: WNDPROC,
    style: WNDCLASS_STYLES,
    /// Whether the class sets the arrow cursor; without one the window must
    /// answer `WM_SETCURSOR` itself.
    arrow_cursor: bool,
}

impl WindowClass {
    /// A class with no style and no class cursor.
    pub(crate) const fn new(name: PCWSTR, procedure: WNDPROC) -> Self {
        Self {
            name,
            procedure,
            style: WNDCLASS_STYLES(0),
            arrow_cursor: false,
        }
    }

    pub(crate) const fn style(self, style: WNDCLASS_STYLES) -> Self {
        Self { style, ..self }
    }

    pub(crate) const fn arrow_cursor(self) -> Self {
        Self {
            arrow_cursor: true,
            ..self
        }
    }
}

/// This executable's module handle, the `hInstance` every class and window
/// uses.
pub(crate) fn instance() -> Result<HINSTANCE, PlatformError> {
    // SAFETY: querying our own module handle has no preconditions.
    let module = unsafe { GetModuleHandleW(None) }?;
    Ok(HINSTANCE(module.0))
}

/// Registers `class` for this executable, or does nothing if it already is.
pub(crate) fn register(class: WindowClass) -> Result<(), PlatformError> {
    let cursor = if class.arrow_cursor {
        // SAFETY: loading a shared stock cursor.
        unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default()
    } else {
        Default::default()
    };
    let description = WNDCLASSW {
        style: class.style,
        lpfnWndProc: class.procedure,
        hInstance: instance()?,
        hCursor: cursor,
        lpszClassName: class.name,
        ..Default::default()
    };
    // SAFETY: `description` is fully initialised and its name is static.
    if unsafe { RegisterClassW(&description) } != 0 {
        return Ok(());
    }
    // SAFETY: reads this thread's last-error value, set by the failed call.
    let error = unsafe { GetLastError() };
    if error == ERROR_CLASS_ALREADY_EXISTS {
        return Ok(());
    }
    tracing::error!(?error, "window class registration failed");
    Err(PlatformError::ClassRegistration)
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;
    use windows::core::w;

    use super::*;

    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: forwards Windows' own arguments unchanged.
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    #[test]
    fn registering_a_class_twice_succeeds() {
        let class = WindowClass::new(w!("DeskSpaceTestClass"), Some(procedure)).arrow_cursor();
        assert!(register(class).is_ok());
        assert!(register(class).is_ok(), "an existing class is not an error");
    }
}
