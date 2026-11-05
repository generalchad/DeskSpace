//! Dark mode for DeskSpace's own UI: popup menus, window captions, and the
//! controls and scroll bars of the settings window, the debug window, and the
//! dialogs.
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
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Controls::{COMBOBOXINFO, GetComboBoxInfo, SetWindowTheme};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GWL_STYLE, GetClassNameW, GetWindowLongPtrW, WS_HSCROLL, WS_VSCROLL,
};
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
    // Process state: the uxtheme exports, looked up once. They live for the
    // process and every themed menu asks for them; first use fills it, and it
    // is never cleared.
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

impl ControlTheme {
    /// The dark class for a control of window class `class`, which has a
    /// scroll bar of its own when `scrolls`; `None` leaves it as it is.
    ///
    /// Only `DarkMode_Explorer` draws a dark scroll bar, so any window that
    /// scrolls takes it: a settings page, the debug window's pane, and a
    /// multi-line text box, where `DarkMode_CFD` left the bar white.
    /// Notepad++'s dark mode makes the same choice for its edits.
    /// https://github.com/notepad-plus-plus/notepad-plus-plus/blob/master/PowerEditor/src/NppDarkMode.cpp
    fn for_class(class: &str, scrolls: bool) -> Option<Self> {
        match class {
            "Edit" if scrolls => Some(Self::Explorer),
            "ComboBox" | "Edit" => Some(Self::FileDialog),
            "Button" | "ListBox" | "SysListView32" | "msctls_trackbar32" => Some(Self::Explorer),
            _ if scrolls => Some(Self::Explorer),
            _ => None,
        }
    }
}

/// Gives every control under `window` the dark or light visual style for its
/// class, its scroll bars included. A drop-down's list is a window of its own,
/// outside the children, so it is themed through its combo box; otherwise a
/// long list, such as the installed fonts, scrolled on a white bar.
pub(crate) fn theme_children(window: HWND, dark: bool) {
    unsafe extern "system" fn visit(control: HWND, dark: LPARAM) -> BOOL {
        let dark = dark.0 != 0;
        let mut class = [0u16; 32];
        // SAFETY: `control` is a live descendant being enumerated; the
        // buffer is sized for the call, and the style read has no
        // preconditions.
        let (class, style) = unsafe {
            let length = GetClassNameW(control, &mut class) as usize;
            (
                String::from_utf16_lossy(&class[..length]),
                GetWindowLongPtrW(control, GWL_STYLE) as u32,
            )
        };
        let scrolls = style & (WS_VSCROLL.0 | WS_HSCROLL.0) != 0;
        if let Some(theme) = ControlTheme::for_class(&class, scrolls) {
            theme_control(control, theme, dark);
        }
        if class == "ComboBox" {
            let mut info = COMBOBOXINFO {
                cbSize: std::mem::size_of::<COMBOBOXINFO>() as u32,
                ..Default::default()
            };
            // SAFETY: `control` is a live combo box and `info` is sized.
            if unsafe { GetComboBoxInfo(control, &mut info) }.is_ok() && !info.hwndList.is_invalid()
            {
                theme_control(info.hwndList, ControlTheme::Explorer, dark);
            }
        }
        BOOL::from(true)
    }
    // SAFETY: the callback only themes the enumerated controls.
    unsafe {
        let _ = EnumChildWindows(Some(window), Some(visit), LPARAM(dark as isize));
    }
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
    /// The selection bar and marked dot in the navigation panes, and the
    /// color picker's selection ring.
    pub accent: Color,
    /// Hairline between the navigation panes.
    pub divider: Color,
    /// The fill of a page's card, behind a group of fields.
    pub card: Color,
    /// A card's edge and the hairline between its rows.
    pub card_stroke: Color,
}

impl UiColors {
    /// The colors with Windows' accent, in the shade Windows' own settings
    /// draw it, as the spaces' check boxes follow the accent too. Falls back
    /// to `new`'s, which are the default accent's shades.
    pub(crate) fn for_system(dark: bool) -> Self {
        let colors = Self::new(dark);
        match crate::win32::system::accent_shade(dark) {
            Some(accent) => Self { accent, ..colors },
            None => colors,
        }
    }

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
                card: Color::rgb(0x32, 0x32, 0x32),
                card_stroke: Color::rgb(0x3D, 0x3D, 0x3D),
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
                card: Color::rgb(0xF9, 0xF9, 0xF9),
                card_stroke: Color::rgb(0xE5, 0xE5, 0xE5),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anything_that_scrolls_takes_the_class_with_a_dark_scroll_bar() {
        use ControlTheme::{Explorer, FileDialog};
        assert_eq!(ControlTheme::for_class("Edit", false), Some(FileDialog));
        assert_eq!(ControlTheme::for_class("Edit", true), Some(Explorer));
        assert_eq!(ControlTheme::for_class("ComboBox", true), Some(FileDialog));
        assert_eq!(ControlTheme::for_class("ListBox", true), Some(Explorer));
        assert_eq!(
            ControlTheme::for_class("DeskSpaceSettingsPage", true),
            Some(Explorer),
            "a page that scrolls"
        );
        assert_eq!(
            ControlTheme::for_class("DeskSpaceSettingsPage", false),
            None
        );
        assert_eq!(ControlTheme::for_class("Static", false), None);
    }

    #[test]
    fn text_contrasts_with_the_body_in_both_modes() {
        for dark in [true, false] {
            let colors = UiColors::new(dark);
            let contrast = (colors.text.luminance() - colors.body.luminance()).abs();
            assert!(contrast > 0.6, "dark={dark}");
            let on_card = (colors.text.luminance() - colors.card.luminance()).abs();
            assert!(on_card > 0.6, "text on a card, dark={dark}");
            assert_eq!(colors.body.luminance() < 0.5, dark);
        }
    }

    #[test]
    fn a_card_stands_off_the_body_and_its_stroke_stands_off_the_card() {
        for dark in [true, false] {
            let colors = UiColors::new(dark);
            assert_ne!(colors.card, colors.body, "dark={dark}");
            assert_ne!(colors.card_stroke, colors.card, "dark={dark}");
        }
    }
}
