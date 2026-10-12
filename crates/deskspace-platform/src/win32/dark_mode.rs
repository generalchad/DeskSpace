//! Dark mode for DeskSpace's own UI: popup menus, the settings window's
//! caption, and its controls.
//!
//! Win32 has no documented dark mode for menus or classic controls. Menus
//! follow the process "app mode" set through two unnamed uxtheme exports
//! (ordinals 135 `SetPreferredAppMode` and 136 `FlushMenuThemes`, Windows 10
//! 1903 and later), as Notepad++, Windows Terminal, and the win32-darkmode
//! sample do. Controls use the dark visual-style classes Explorer itself uses
//! (`DarkMode_Explorer`, `DarkMode_CFD`). If the exports are missing, menus
//! simply stay light.
//! https://github.com/ysc3839/win32-darkmode

use std::sync::OnceLock;

use deskspace_core::theme::Color;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::core::{BOOL, PCSTR, PCWSTR, w};

/// `PreferredAppMode` values for `SetPreferredAppMode`.
const APP_MODE_FORCE_DARK: i32 = 2;
const APP_MODE_FORCE_LIGHT: i32 = 3;

type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
type FlushMenuThemes = unsafe extern "system" fn();

struct Exports {
    set_preferred_app_mode: SetPreferredAppMode,
    flush_menu_themes: FlushMenuThemes,
}

fn exports() -> Option<&'static Exports> {
    static EXPORTS: OnceLock<Option<Exports>> = OnceLock::new();
    EXPORTS
        .get_or_init(|| {
            // SAFETY: uxtheme is a system DLL that stays loaded; ordinals are
            // passed as MAKEINTRESOURCEA values, and the exports at these
            // ordinals have the signatures above on every build that has
            // them.
            unsafe {
                let uxtheme = LoadLibraryW(w!("uxtheme.dll")).ok()?;
                let ordinal = |n: usize| PCSTR(n as *const u8);
                let set_mode = GetProcAddress(uxtheme, ordinal(135))?;
                let flush = GetProcAddress(uxtheme, ordinal(136))?;
                Some(Exports {
                    set_preferred_app_mode: std::mem::transmute::<
                        unsafe extern "system" fn() -> isize,
                        SetPreferredAppMode,
                    >(set_mode),
                    flush_menu_themes: std::mem::transmute::<
                        unsafe extern "system" fn() -> isize,
                        FlushMenuThemes,
                    >(flush),
                })
            }
        })
        .as_ref()
}

/// Switches popup menus (DeskSpace's and the shell's item menus) between
/// dark and light.
pub(crate) fn set_menus_dark(dark: bool) {
    let Some(exports) = exports() else {
        tracing::debug!("uxtheme app mode is unavailable; menus stay light");
        return;
    };
    let mode = if dark {
        APP_MODE_FORCE_DARK
    } else {
        APP_MODE_FORCE_LIGHT
    };
    // SAFETY: both functions take/return plain values.
    unsafe {
        (exports.set_preferred_app_mode)(mode);
        (exports.flush_menu_themes)();
    }
}

/// Dark or light caption for a top-level window.
/// https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute
pub(crate) fn set_caption_dark(hwnd: HWND, dark: bool) {
    let value = BOOL::from(dark);
    // SAFETY: `hwnd` is our live top-level window; `value` outlives the call.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &value as *const BOOL as *const core::ffi::c_void,
            std::mem::size_of::<BOOL>() as u32,
        );
    }
}

/// The visual-style class a control uses in dark mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlTheme {
    /// Push buttons, checkboxes, and scrollbars.
    Explorer,
    /// Combo boxes and edits, as in the common file dialog.
    FileDialog,
}

/// Applies `theme`'s dark class to `control`, or restores the default
/// (light) visual style.
pub(crate) fn theme_control(control: HWND, theme: ControlTheme, dark: bool) {
    let class = match (dark, theme) {
        (false, _) => PCWSTR::null(),
        (true, ControlTheme::Explorer) => w!("DarkMode_Explorer"),
        (true, ControlTheme::FileDialog) => w!("DarkMode_CFD"),
    };
    // SAFETY: `control` is a live control of ours; null restores the default.
    unsafe {
        let _ = SetWindowTheme(control, class, PCWSTR::null());
    }
}

/// Colors of the settings window, after the Windows 11 dialog palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UiColors {
    pub dark: bool,
    /// Page background.
    pub body: Color,
    /// The command bar behind OK / Cancel / Apply.
    pub command_bar: Color,
    pub text: Color,
    /// Unselected navigation rows.
    pub secondary_text: Color,
    /// Labels of disabled fields.
    pub disabled_text: Color,
    /// The selection bar and marked dot in the navigation panes.
    pub accent: Color,
    /// Hairline between the navigation panes.
    pub divider: Color,
}

impl UiColors {
    pub(crate) fn new(dark: bool) -> Self {
        if dark {
            Self {
                dark,
                body: Color::rgb(0x2B, 0x2B, 0x2B),
                command_bar: Color::rgb(0x20, 0x20, 0x20),
                text: Color::rgb(0xFF, 0xFF, 0xFF),
                secondary_text: Color::rgb(0xC5, 0xC5, 0xC5),
                disabled_text: Color::rgb(0x7A, 0x7A, 0x7A),
                accent: Color::rgb(0x4C, 0xC2, 0xFF),
                divider: Color::rgb(0x3D, 0x3D, 0x3D),
            }
        } else {
            Self {
                dark,
                body: Color::rgb(0xFF, 0xFF, 0xFF),
                command_bar: Color::rgb(0xF3, 0xF3, 0xF3),
                text: Color::rgb(0x1A, 0x1A, 0x1A),
                secondary_text: Color::rgb(0x5D, 0x5D, 0x5D),
                disabled_text: Color::rgb(0xA3, 0xA3, 0xA3),
                accent: Color::rgb(0x00, 0x5F, 0xB8),
                divider: Color::rgb(0xE5, 0xE5, 0xE5),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_contrasts_with_the_body_in_both_modes() {
        for dark in [true, false] {
            let colors = UiColors::new(dark);
            let contrast = (colors.text.luminance() - colors.body.luminance()).abs();
            assert!(contrast > 0.6, "dark={dark}");
            assert_eq!(colors.body.luminance() < 0.5, dark);
        }
    }
}
