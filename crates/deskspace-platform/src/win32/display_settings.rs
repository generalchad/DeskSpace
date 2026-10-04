//! Explorer's display settings, so spaces can match the desktop's look.
//!
//! The desktop's own view flags (item check boxes) come from its `IShellView`;
//! the global folder options (hidden files, filename extensions) come from
//! `SHGetSetSettings`. The app overlays these on its own Appearance settings
//! when "Match Explorer's display" is on.

use windows::Win32::UI::Shell::{
    FWF_AUTOCHECKSELECT, FWF_CHECKSELECT, IFolderView2, IShellView, SHELLSTATEA, SHGetSetSettings,
    SSF_MASK, SSF_SHOWALLOBJECTS, SSF_SHOWEXTENSIONS, SSF_SHOWSUPERHIDDEN,
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

fn global_display() -> ShellDisplay {
    let mut state = SHELLSTATEA::default();
    let mask = SSF_SHOWALLOBJECTS | SSF_SHOWEXTENSIONS | SSF_SHOWSUPERHIDDEN;
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
    }
}
