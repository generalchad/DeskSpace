//! The application icon the build script embeds, loaded at the sizes the
//! tray and each top-level window need.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, GCLP_HICON, GCLP_HICONSM, HICON, ICON_BIG, ICON_SMALL,
    ICONINFO, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON, SendMessageW,
    SetClassLongPtrW, WM_SETICON,
};
use windows::core::{PCWSTR, Result};

use crate::render::Icon;
use crate::render::d2d::{debug_badge, decode_icon};
use crate::win32::dpi::window_dpi;
use crate::win32::drag_image;

/// `MAKEINTRESOURCE(1)`: the Win32 resource APIs take an integer id in place
/// of a name, encoded as the raw value `1`. It is deliberately the value 1 and
/// not an aligned dangling pointer.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-makeintresourcew
const ICON_RESOURCE: PCWSTR = PCWSTR(std::ptr::without_provenance::<u16>(1));

/// Loads icon resource 1 (embedded by the build script) at `size` pixels
/// square, picking the closest image in the `.ico`. It is not `LR_SHARED`, so
/// the returned handle is the caller's to destroy.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-loadimagew
pub(crate) fn load(size: i32) -> Result<HICON> {
    // SAFETY: the module handle is this process, and resource 1 is the icon the
    // build script embedded; the returned handle is a fresh, unshared icon.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let handle = LoadImageW(
            Some(instance.into()),
            ICON_RESOURCE,
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        )?;
        Ok(HICON(handle.0))
    }
}

/// Which variant of the application icon a window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Badge {
    /// The icon as embedded.
    None,
    /// The icon with a bug on an amber disc in its bottom-right corner, so
    /// the debug window stands apart on the taskbar and in Alt+Tab.
    Debug,
}

/// A window's own big and small icons, destroyed when this is dropped (with
/// the window's state, in `WM_NCDESTROY`).
///
/// The taskbar and Alt+Tab draw `ICON_BIG`, so it must be loaded at the big
/// size for the window's DPI: a small icon there is what left the settings
/// and debug windows without a usable taskbar icon.
/// https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-seticon
///
/// The icons are also set on the window's class. The taskbar asks the window
/// for its icon with a timeout, and when the UI thread is still busy with
/// startup the request goes unanswered and the button kept a generic icon
/// (seen 2026-10-19 with `--debug`). Its fallback, the class icon, needs no
/// answer from the window. Only settings and debug windows use this, and each
/// class has one window at a time, so the class icons are the window's.
#[derive(Default)]
pub(crate) struct WindowIcons {
    /// The window whose class holds the icons, cleared before they are
    /// destroyed.
    window: Option<HWND>,
    big: Option<HICON>,
    small: Option<HICON>,
}

impl WindowIcons {
    /// Loads both sizes for `window`'s DPI with `badge` and sets them on it
    /// and its class.
    pub(crate) fn set(window: HWND, badge: Badge) -> Self {
        let dpi = window_dpi(window);
        // SAFETY: plain metric queries for a known DPI.
        let (big_size, small_size) = unsafe {
            (
                GetSystemMetricsForDpi(SM_CXICON, dpi),
                GetSystemMetricsForDpi(SM_CXSMICON, dpi),
            )
        };
        let icons = Self {
            window: Some(window),
            big: load_badged(big_size, badge),
            small: load_badged(small_size, badge),
        };
        let slots = [
            (ICON_BIG, GCLP_HICON, icons.big),
            (ICON_SMALL, GCLP_HICONSM, icons.small),
        ];
        for (kind, class_slot, icon) in slots {
            let Some(icon) = icon else { continue };
            // SAFETY: `window` is live and `icon` stays alive until this value
            // is dropped with the window's state, which first clears the
            // class slot.
            unsafe {
                SendMessageW(
                    window,
                    WM_SETICON,
                    Some(WPARAM(kind as usize)),
                    Some(LPARAM(icon.0 as isize)),
                );
                SetClassLongPtrW(window, class_slot, icon.0 as isize);
            }
        }
        icons
    }
}

impl Drop for WindowIcons {
    fn drop(&mut self) {
        if let Some(window) = self.window {
            for slot in [GCLP_HICON, GCLP_HICONSM] {
                // SAFETY: called from the window's WM_NCDESTROY, while the
                // handle is still valid; the class must not keep icons that
                // are destroyed below.
                unsafe {
                    SetClassLongPtrW(window, slot, 0);
                }
            }
        }
        for icon in [self.big, self.small].into_iter().flatten() {
            // SAFETY: both icons were created by `load_badged` for this value
            // alone, and the window that showed them is being destroyed.
            unsafe {
                let _ = DestroyIcon(icon);
            }
        }
    }
}

/// The application icon at `size` with `badge`, or the plain icon when the
/// badge cannot be drawn.
fn load_badged(size: i32, badge: Badge) -> Option<HICON> {
    let plain = match load(size) {
        Ok(icon) => icon,
        Err(error) => {
            tracing::warn!(%error, size, "could not load the application icon");
            return None;
        }
    };
    match badge {
        Badge::None => Some(plain),
        Badge::Debug => {
            let badged = decode_icon(plain)
                .and_then(|icon| debug_badge(&icon))
                .and_then(|icon| to_hicon(&icon));
            if badged.is_none() {
                tracing::debug!(size, "could not badge the icon; using the plain one");
                return Some(plain);
            }
            // SAFETY: `plain` was loaded above, unshared, and is not used again.
            unsafe {
                let _ = DestroyIcon(plain);
            }
            badged
        }
    }
}

/// An `HICON` from `icon`'s pixels. A 32 bpp icon's color bitmap holds
/// straight alpha, as an `.ico` file does; the mask is empty because the
/// alpha channel decides transparency.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createiconindirect
fn to_hicon(icon: &Icon) -> Option<HICON> {
    let color = drag_image::dib(icon, false)?;
    // Each mask row is padded to a WORD.
    let mask = vec![0u8; (icon.width() as usize).div_ceil(16) * 2 * icon.height() as usize];
    // SAFETY: the mask buffer holds exactly the bitmap it describes, and
    // CreateIconIndirect copies both bitmaps, so they are deleted here.
    unsafe {
        let mask = CreateBitmap(
            icon.width() as i32,
            icon.height() as i32,
            1,
            1,
            Some(mask.as_ptr().cast()),
        );
        let info = ICONINFO {
            fIcon: true.into(),
            hbmMask: mask,
            hbmColor: color,
            ..Default::default()
        };
        let created = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        created
    }
}
