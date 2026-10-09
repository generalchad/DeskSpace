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
    let mut display = global_display();
    if let Some(view) = shell_view::desktop_shell_view() {
        // Only override when the view reports its flags; a failed read (the
        // view is briefly unavailable) must not silently hide check boxes.
        if let Some(mode) = view_check_boxes(&view) {
            display.check_boxes = mode;
        }
        if let Some(size) = view_icon_size(&view) {
            display.icon_size = Some(size);
        }
        if let Some(spacing) = view_spacing(&view) {
            display.icon_spacing = Some(spacing);
        }
    }
    display
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

/// The desktop's icon size, only when the view shows icons. In list or details
/// mode `GetViewModeAndIconSize` reports the small list icon, which says
/// nothing useful about the desktop's icons.
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
