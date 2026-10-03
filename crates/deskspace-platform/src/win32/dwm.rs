//! Windows 11 window chrome for spaces: blurred material behind the client
//! area, rounded corners, dark mode, and border color.
//!
//! The material is applied with the accent policy
//! (`SetWindowCompositionAttribute`), not `DWMWA_SYSTEMBACKDROP_TYPE`. The
//! documented system backdrops only render while a window is active and fall
//! back to a flat color otherwise; spaces are never activated, so they showed
//! no transparency at all. The accent policy is undocumented but has been
//! stable since Windows 10 1803 and blurs regardless of activation.
//!
//! Every attribute is best-effort: on builds that lack one, the space keeps
//! its translucent tint without blur or rounding.

use std::sync::OnceLock;

use deskspace_core::theme::Backdrop;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_NONE, DWMWA_BORDER_COLOR,
    DWMWA_COLOR_DEFAULT, DWMWA_COLOR_NONE, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
    DWMWCP_ROUNDSMALL, DWMWINDOWATTRIBUTE, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::UI::Controls::MARGINS;
use windows::core::{BOOL, s, w};

use crate::render::SpaceStyle;

/// `WINDOWCOMPOSITIONATTRIB::WCA_ACCENT_POLICY` and the `ACCENT_STATE` values
/// used with it. Not in the Windows SDK headers.
const WCA_ACCENT_POLICY: u32 = 19;
const ACCENT_DISABLED: u32 = 0;
const ACCENT_ENABLE_BLURBEHIND: u32 = 3;
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
/// Acrylic needs a non-zero gradient alpha; at zero some builds stop drawing
/// the material. The visible tint is painted by the space itself, so this is
/// the faintest possible tint (0xAABBGGRR).
const ACRYLIC_GRADIENT: u32 = 0x0100_0000;

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: u32,
    data: *mut core::ffi::c_void,
    size: usize,
}

type SetWindowCompositionAttribute =
    unsafe extern "system" fn(HWND, *mut WindowCompositionAttributeData) -> BOOL;

fn set_window_composition_attribute() -> Option<SetWindowCompositionAttribute> {
    static FUNCTION: OnceLock<Option<SetWindowCompositionAttribute>> = OnceLock::new();
    *FUNCTION.get_or_init(|| {
        // SAFETY: user32 is always loaded in a GUI process. The export, when
        // present, has the signature above.
        unsafe {
            let user32 = GetModuleHandleW(w!("user32.dll")).ok()?;
            let address = GetProcAddress(user32, s!("SetWindowCompositionAttribute"))?;
            Some(std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                SetWindowCompositionAttribute,
            >(address))
        }
    })
}

pub(crate) fn apply_style(hwnd: HWND, style: SpaceStyle) {
    // Negative margins extend the frame over the whole client area ("sheet of
    // glass"), so DWM composites the client's per-pixel alpha over the
    // material instead of treating the client as opaque.
    // https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmextendframeintoclientarea
    let glass = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    // SAFETY: `hwnd` is a live top-level window owned by this thread.
    if let Err(error) = unsafe { DwmExtendFrameIntoClientArea(hwnd, &glass) } {
        tracing::debug!(%error, "DwmExtendFrameIntoClientArea failed");
    }

    set(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, BOOL::from(style.dark));
    set(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        corner_preference(style),
    );
    // Make sure no system backdrop competes with the accent material.
    set::<DWM_SYSTEMBACKDROP_TYPE>(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_NONE);
    // DWMWA_COLOR_DEFAULT lets Windows pick the border (including the accent
    // color when "Show accent color on title bars" is on).
    // https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute
    let border = if style.border && style.dwm_draws_frame() {
        DWMWA_COLOR_DEFAULT
    } else {
        DWMWA_COLOR_NONE
    };
    set(hwnd, DWMWA_BORDER_COLOR, border);
    set_accent(hwnd, accent_state(style.backdrop));
}

fn accent_state(backdrop: Backdrop) -> u32 {
    match backdrop {
        Backdrop::Acrylic => ACCENT_ENABLE_ACRYLICBLURBEHIND,
        Backdrop::Blur => ACCENT_ENABLE_BLURBEHIND,
        Backdrop::Solid | Backdrop::Clear => ACCENT_DISABLED,
    }
}

fn set_accent(hwnd: HWND, state: u32) {
    let Some(function) = set_window_composition_attribute() else {
        tracing::debug!("SetWindowCompositionAttribute is unavailable; no blur");
        return;
    };
    let mut policy = AccentPolicy {
        accent_state: state,
        accent_flags: 0,
        gradient_color: if state == ACCENT_ENABLE_ACRYLICBLURBEHIND {
            ACRYLIC_GRADIENT
        } else {
            0
        },
        animation_id: 0,
    };
    let mut data = WindowCompositionAttributeData {
        attribute: WCA_ACCENT_POLICY,
        data: &mut policy as *mut AccentPolicy as *mut core::ffi::c_void,
        size: std::mem::size_of::<AccentPolicy>(),
    };
    // SAFETY: `data` points at a correctly sized AccentPolicy that outlives
    // the call; `hwnd` is our live top-level window.
    if !unsafe { function(hwnd, &mut data) }.as_bool() {
        tracing::debug!(state, "accent policy not applied");
    }
}

/// DWM's own corners where it has the radius; otherwise square, and the
/// space rounds itself (`space_window::update_region` and the painter).
fn corner_preference(style: SpaceStyle) -> DWM_WINDOW_CORNER_PREFERENCE {
    if !style.dwm_draws_frame() {
        return DWMWCP_DONOTROUND;
    }
    match style.corner_radius {
        4 => DWMWCP_ROUNDSMALL,
        8 => DWMWCP_ROUND,
        _ => DWMWCP_DONOTROUND,
    }
}

fn set<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: T) {
    // SAFETY: `value` is a plain value of the size DWM expects for
    // `attribute` and lives for the duration of the call.
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute,
            &value as *const T as *const core::ffi::c_void,
            std::mem::size_of::<T>() as u32,
        )
    };
    if let Err(error) = result {
        tracing::debug!(attribute = attribute.0, %error, "DWM attribute not applied");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_theme_choices_to_accent_and_corner_values() {
        assert_eq!(
            accent_state(Backdrop::Acrylic),
            ACCENT_ENABLE_ACRYLICBLURBEHIND
        );
        assert_eq!(accent_state(Backdrop::Blur), ACCENT_ENABLE_BLURBEHIND);
        assert_eq!(accent_state(Backdrop::Solid), ACCENT_DISABLED);
        assert_eq!(accent_state(Backdrop::Clear), ACCENT_DISABLED);
        let style = |backdrop, corner_radius| SpaceStyle {
            backdrop,
            corner_radius,
            dark: true,
            border: true,
        };
        assert_eq!(corner_preference(style(Backdrop::Acrylic, 8)), DWMWCP_ROUND);
        assert_eq!(
            corner_preference(style(Backdrop::Blur, 4)),
            DWMWCP_ROUNDSMALL
        );
        assert_eq!(
            corner_preference(style(Backdrop::Acrylic, 0)),
            DWMWCP_DONOTROUND
        );
        assert_eq!(
            corner_preference(style(Backdrop::Acrylic, 12)),
            DWMWCP_DONOTROUND
        );
        assert_eq!(
            corner_preference(style(Backdrop::Clear, 8)),
            DWMWCP_DONOTROUND
        );
    }

    #[test]
    fn accent_policy_matches_the_native_layout() {
        assert_eq!(std::mem::size_of::<AccentPolicy>(), 16);
    }
}
