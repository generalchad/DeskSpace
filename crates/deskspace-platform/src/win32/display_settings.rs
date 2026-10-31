//! Explorer's display settings, so spaces can match the desktop's look.
//!
//! The desktop's own view flags (item check boxes), item size, and item spacing
//! come from its `IShellView`; the global folder options (hidden files,
//! filename extensions) come from `SHGetSetSettings`. The app overlays these on
//! its own Appearance settings when "Match Explorer's display" is on.

use deskspace_core::geometry::Size;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Shell::{
    FVM_ICON, FVM_SMALLICON, FWF_AUTOCHECKSELECT, FWF_CHECKSELECT, IFolderView, IFolderView2,
    IShellView, SHELLSTATEA, SHGetSetSettings, SSF_MASK, SSF_SHOWALLOBJECTS, SSF_SHOWCOMPCOLOR,
    SSF_SHOWEXTENSIONS, SSF_SHOWSUPERHIDDEN,
};
use windows::core::Interface;

use crate::win32::shell_view;

/// The desktop icon sizes Ctrl+wheel steps between, in pixels: Explorer's
/// small icon up to its largest "extra large" size.
pub const DESKTOP_ICON_RANGE: (i32, i32) = (16, 256);

/// How many pixels one wheel notch changes the desktop's icon size by.
pub const DESKTOP_ICON_STEP: i32 = 8;

/// How Explorer's desktop view shows item check boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckBoxMode {
    /// No check boxes.
    #[default]
    Hidden,
    /// Auto-check-select: the tick mirrors the selection
    /// (`FWF_AUTOCHECKSELECT`).
    Selection,
    /// Independent "item check boxes": the tick is separate from the selection
    /// (`FWF_CHECKSELECT`).
    Independent,
}

/// Explorer display options that affect how items are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShellDisplay {
    /// How the view shows item check boxes.
    pub check_boxes: CheckBoxMode,
    /// Filenames are shown with their extension (`SSF_SHOWEXTENSIONS`).
    pub show_extensions: bool,
    /// Hidden files are listed (`SSF_SHOWALLOBJECTS`).
    pub show_hidden: bool,
    /// Protected operating-system files are listed (`SSF_SHOWSUPERHIDDEN`).
    pub show_superhidden: bool,
    /// Compressed files are colored and encrypted files are colored green
    /// (`SSF_SHOWCOMPCOLOR`).
    pub show_compressed_color: bool,
    /// The desktop's icon size in pixels
    /// (`IFolderView2::GetViewModeAndIconSize`). `None` when the view is not
    /// showing icons or the read failed.
    pub icon_size: Option<i32>,
    /// The desktop's full cell spacing in pixels
    /// (`IFolderView::GetSpacing`): the distance Explorer places the next item
    /// at, including the icon and its surrounding whitespace.
    pub icon_spacing: Option<Size>,
}

/// Reads the desktop's current display options.
pub fn desktop_display() -> ShellDisplay {
    desktop_display_since(CheckBoxMode::Hidden)
}

/// Like `desktop_display`, but when the desktop view is unavailable or its
/// flags cannot be read it keeps `previous` as the check-box mode instead of
/// reporting `Hidden`. A failed read (the view is briefly unavailable, for
/// example while Explorer restarts) must not silently hide the boxes.
///
/// The icon size and spacing still fall back to their defaults on a failed
/// read: their `None` also means "not in icon mode", so they need a separate
/// design.
pub fn desktop_display_since(previous: CheckBoxMode) -> ShellDisplay {
    let mut display = global_display();
    display.check_boxes = previous;
    if let Some(view) = shell_view::desktop_shell_view() {
        display.check_boxes = check_box_mode(view_check_boxes(&view), previous);
        if let Some(size) = view_icon_size(&view) {
            display.icon_size = Some(size);
        }
        if let Some(spacing) = view_spacing(&view) {
            display.icon_spacing = Some(spacing);
        }
    }
    display
}

/// The check-box mode for a display read: the view's mode when it was readable,
/// else the mode already known.
fn check_box_mode(read: Option<CheckBoxMode>, previous: CheckBoxMode) -> CheckBoxMode {
    read.unwrap_or(previous)
}

fn view_check_boxes(view: &IShellView) -> Option<CheckBoxMode> {
    let folder = view.cast::<IFolderView2>().ok()?;
    // SAFETY: reading a flag from a live folder view.
    match unsafe { folder.GetCurrentFolderFlags() } {
        Ok(flags) => Some(if flags & FWF_CHECKSELECT.0 as u32 != 0 {
            CheckBoxMode::Independent
        } else if flags & FWF_AUTOCHECKSELECT.0 as u32 != 0 {
            CheckBoxMode::Selection
        } else {
            CheckBoxMode::Hidden
        }),
        Err(error) => {
            tracing::debug!(%error, "could not read the desktop view flags");
            None
        }
    }
}

/// The desktop's icon size, only when the view shows icons. Explorer's
/// desktop offers only the three icon views, and Small icons keep their
/// labels below the icon, so the size alone describes its look. In a list
/// or details mode `GetViewModeAndIconSize` reports the small list icon,
/// which says nothing useful about the desktop's icons.
fn view_icon_size(view: &IShellView) -> Option<i32> {
    let folder = view.cast::<IFolderView2>().ok()?;
    let mut mode = Default::default();
    let mut size = 0i32;
    // SAFETY: filling two out-parameters of a live folder view.
    let result = unsafe { folder.GetViewModeAndIconSize(&mut mode, &mut size) };
    match result {
        Ok(()) if size > 0 && (mode.0 == FVM_ICON.0 || mode.0 == FVM_SMALLICON.0) => Some(size),
        Ok(()) => None,
        Err(error) => {
            tracing::debug!(%error, "could not read the desktop icon size");
            None
        }
    }
}

/// The desktop icon size `notches` wheel notches away from `current`, kept
/// within `DESKTOP_ICON_RANGE`.
pub fn next_desktop_icon_size(current: i32, notches: i32) -> i32 {
    (current + notches * DESKTOP_ICON_STEP).clamp(DESKTOP_ICON_RANGE.0, DESKTOP_ICON_RANGE.1)
}

/// Sets the desktop's own icon size, as Ctrl+wheel on the desktop does, and
/// keeps its view mode. Returns whether Explorer accepted it.
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifolderview2-setviewmodeandiconsize
pub fn set_desktop_icon_size(pixels: i32) -> bool {
    let Some(view) = shell_view::desktop_shell_view() else {
        return false;
    };
    let Ok(folder) = view.cast::<IFolderView2>() else {
        return false;
    };
    let mut mode = Default::default();
    let mut current = 0i32;
    // SAFETY: reading two out-parameters, then setting the size, on a live
    // folder view.
    let result = unsafe {
        folder
            .GetViewModeAndIconSize(&mut mode, &mut current)
            .and_then(|()| folder.SetViewModeAndIconSize(mode, pixels))
    };
    match result {
        Ok(()) => true,
        Err(error) => {
            tracing::debug!(%error, pixels, "could not set the desktop icon size");
            false
        }
    }
}

/// The desktop's item spacing (the full width and height of an item cell).
fn view_spacing(view: &IShellView) -> Option<Size> {
    let folder = view.cast::<IFolderView>().ok()?;
    let mut point = POINT::default();
    // SAFETY: filling an out-parameter of a live folder view.
    let result = unsafe { folder.GetSpacing(&mut point) };
    match result {
        Ok(()) if point.x > 0 && point.y > 0 => Some(Size::new(point.x, point.y)),
        Ok(()) => None,
        Err(error) => {
            tracing::debug!(%error, "could not read the desktop item spacing");
            None
        }
    }
}

fn global_display() -> ShellDisplay {
    let mut state = SHELLSTATEA::default();
    let mask = SSF_SHOWALLOBJECTS | SSF_SHOWEXTENSIONS | SSF_SHOWSUPERHIDDEN | SSF_SHOWCOMPCOLOR;
    // SAFETY: `state` is a valid out-structure of the documented size; the
    // call only fills the masked bits.
    unsafe { SHGetSetSettings(Some(&mut state), mask, false) };
    let bits = state._bitfield1;
    let set = |flag: SSF_MASK| bits & flag.0 as i32 != 0;
    ShellDisplay {
        check_boxes: CheckBoxMode::Hidden,
        show_extensions: set(SSF_SHOWEXTENSIONS),
        show_hidden: set(SSF_SHOWALLOBJECTS),
        show_superhidden: set(SSF_SHOWSUPERHIDDEN),
        show_compressed_color: set(SSF_SHOWCOMPCOLOR),
        icon_size: None,
        icon_spacing: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_box_mode_keeps_the_previous_mode_when_the_read_fails() {
        assert_eq!(
            check_box_mode(Some(CheckBoxMode::Independent), CheckBoxMode::Selection),
            CheckBoxMode::Independent
        );
        assert_eq!(
            check_box_mode(None, CheckBoxMode::Selection),
            CheckBoxMode::Selection
        );
        assert_eq!(
            check_box_mode(None, CheckBoxMode::Hidden),
            CheckBoxMode::Hidden
        );
    }

    #[test]
    fn a_notch_steps_the_desktop_icon_size_either_way() {
        assert_eq!(next_desktop_icon_size(48, 1), 48 + DESKTOP_ICON_STEP);
        assert_eq!(next_desktop_icon_size(48, -1), 48 - DESKTOP_ICON_STEP);
        assert_eq!(next_desktop_icon_size(48, 3), 48 + 3 * DESKTOP_ICON_STEP);
    }

    #[test]
    fn the_desktop_icon_size_stops_at_both_ends() {
        assert_eq!(next_desktop_icon_size(20, -1), DESKTOP_ICON_RANGE.0);
        assert_eq!(
            next_desktop_icon_size(DESKTOP_ICON_RANGE.0, -5),
            DESKTOP_ICON_RANGE.0
        );
        assert_eq!(next_desktop_icon_size(252, 1), DESKTOP_ICON_RANGE.1);
        assert_eq!(
            next_desktop_icon_size(DESKTOP_ICON_RANGE.1, 5),
            DESKTOP_ICON_RANGE.1
        );
    }
}
