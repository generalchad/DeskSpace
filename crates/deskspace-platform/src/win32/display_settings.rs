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

/// Explorer display options that affect how items are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShellDisplay {
    /// The view shows a check box on each item (`FWF_CHECKSELECT`).
    pub check_boxes: bool,
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
        if let Some(check_boxes) = view_check_boxes(&view) {
            display.check_boxes = check_boxes;
        }
    }
    display
}

fn view_check_boxes(view: &IShellView) -> Option<bool> {
    let folder = view.cast::<IFolderView2>().ok()?;
    // Explorer sets FWF_AUTOCHECKSELECT for the desktop's "use check boxes"
    // option and FWF_CHECKSELECT elsewhere; either means show them.
    let flag = FWF_CHECKSELECT.0 as u32 | FWF_AUTOCHECKSELECT.0 as u32;
    // SAFETY: reading a flag from a live folder view.
    match unsafe { folder.GetCurrentFolderFlags() } {
        Ok(flags) => Some(flags & flag != 0),
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
        check_boxes: false,
        show_extensions: set(SSF_SHOWEXTENSIONS),
        show_hidden: set(SSF_SHOWALLOBJECTS),
        show_superhidden: set(SSF_SHOWSUPERHIDDEN),
    }
}
