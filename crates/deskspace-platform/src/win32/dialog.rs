//! Shared plumbing for DeskSpace's own modal dialogs (the installed-apps
//! picker and the custom shortcut dialog): the themed frame, fonts, colors,
//! control creation, and the modal message pump.
//!
//! A dialog is an ordinary top-level window built from standard controls, so
//! it follows the dark/light setting the way the settings window does
//! (`dark_mode`). Each dialog owns its state; the window points at it through
//! `GWLP_USERDATA` only while it exists.

use std::path::PathBuf;

use deskspace_core::geometry::Point;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, CreateSolidBrush, DeleteObject, HBRUSH, HDC, HFONT, HGDIOBJ, SetBkColor,
    SetTextColor,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::Shell::{
    Common::COMDLG_FILTERSPEC, FILEOPENDIALOGOPTIONS, FOLDERID_Desktop, FOS_ALLOWMULTISELECT,
    FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PICKFOLDERS, FileOpenDialog,
    FileSaveDialog, IFileOpenDialog, IFileSaveDialog, IShellItem, KF_FLAG_DEFAULT,
    SHGetKnownFolderItem, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, EnumChildWindows, GetClassNameW,
    GetCursorPos, GetMessageW, GetWindowTextLengthW, GetWindowTextW, HMENU, IsDialogMessageW,
    IsWindow, MSG, PostQuitMessage, SW_SHOW, SendMessageW, SetForegroundWindow, ShowWindow,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX,
    WM_SETFONT, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME,
    WS_POPUP, WS_SYSMENU, WS_VISIBLE,
};
use windows::core::{BOOL, PCWSTR};

use crate::win32::dark_mode::{self, ControlTheme, UiColors};
use crate::win32::settings_window::colorref;
use crate::win32::shortcuts::display_name;
use crate::win32::system::{message_logfont, wide, work_area_at};

/// The dialog-manager ids `IsDialogMessageW` sends for Enter and Escape, so
/// the dialogs give their OK and Cancel buttons these ids.
/// https://learn.microsoft.com/en-us/windows/win32/dlgbox/dialog-box-programming-considerations
pub(crate) const ID_OK: usize = 1;
pub(crate) const ID_CANCEL: usize = 2;

/// The font, colors, and brushes one dialog paints with, at its DPI.
pub(crate) struct DialogChrome {
    pub dpi: u32,
    pub font: HFONT,
    pub colors: UiColors,
    /// The dialog body.
    pub body_brush: HBRUSH,
    /// Editable fields in dark mode (the settings window's field color).
    pub field_brush: HBRUSH,
}

impl DialogChrome {
    pub(crate) fn new(dpi: u32, dark: bool) -> Self {
        let colors = UiColors::for_system(dark);
        // SAFETY: plain GDI object creation; the objects are owned by the
        // chrome and deleted in `Drop`.
        unsafe {
            Self {
                dpi,
                font: CreateFontIndirectW(&message_logfont(dpi)),
                colors,
                body_brush: CreateSolidBrush(colorref(colors.body)),
                field_brush: CreateSolidBrush(colorref(colors.command_bar)),
            }
        }
    }

    /// Converts 96-DPI layout units to pixels at the dialog's DPI.
    pub(crate) fn s(&self, value: i32) -> i32 {
        (value as i64 * self.dpi as i64 / 96) as i32
    }

    /// Answers a `WM_CTLCOLOR*` message: labels and buttons sit on the body,
    /// and in dark mode edits and lists use the darker field color. Light
    /// edits keep the system colors (`None`, so the caller defers to
    /// `DefWindowProcW`).
    pub(crate) fn control_color(&self, message: u32, wparam: WPARAM) -> Option<LRESULT> {
        let editable = matches!(message, WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX);
        if editable && !self.colors.dark {
            return None;
        }
        let (background, brush) = if editable {
            (self.colors.command_bar, self.field_brush)
        } else {
            (self.colors.body, self.body_brush)
        };
        let dc = HDC(wparam.0 as *mut core::ffi::c_void);
        // SAFETY: `dc` is the control's DC for the duration of the message.
        unsafe {
            SetTextColor(dc, colorref(self.colors.text));
            SetBkColor(dc, colorref(background));
        }
        Some(LRESULT(brush.0 as isize))
    }
}

impl Drop for DialogChrome {
    fn drop(&mut self) {
        // SAFETY: the objects are owned here, and the dialog's controls are
        // destroyed before its state (and so the chrome) is dropped.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.font.0));
            let _ = DeleteObject(HGDIOBJ(self.body_brush.0));
            let _ = DeleteObject(HGDIOBJ(self.field_brush.0));
        }
    }
}

/// The module instance, for window creation.
pub(crate) fn instance() -> Option<HINSTANCE> {
    // SAFETY: querying our own module handle.
    unsafe {
        GetModuleHandleW(None)
            .ok()
            .map(|module| HINSTANCE(module.0))
    }
}

/// The DPI of `window`'s monitor (96 when it cannot be read).
pub(crate) fn dpi_of(window: HWND) -> u32 {
    // SAFETY: `window` is a live window.
    let dpi = unsafe { GetDpiForWindow(window) };
    if dpi == 0 { 96 } else { dpi }
}

/// Creates a dialog frame of class `class` owned by `owner`, with a client
/// area of `client` pixels, centered on the monitor under the pointer. `state`
/// is handed to `WM_NCCREATE` in `CREATESTRUCTW::lpCreateParams`. The window
/// is created hidden; `show` makes it visible once its controls exist.
pub(crate) unsafe fn create_frame(
    class: PCWSTR,
    title: &str,
    owner: HWND,
    client: (i32, i32),
    dpi: u32,
    state: *const core::ffi::c_void,
) -> Option<HWND> {
    let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
    let ex_style = WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME;
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: client.0,
        bottom: client.1,
    };
    let title = wide(title);
    // SAFETY: plain window creation on the UI thread; `title` outlives it.
    unsafe {
        let _ = AdjustWindowRectExForDpi(&mut frame, style, false, ex_style, dpi);
        let (width, height) = (frame.right - frame.left, frame.bottom - frame.top);
        let mut cursor = Default::default();
        let _ = GetCursorPos(&mut cursor);
        let work = work_area_at(Point::new(cursor.x, cursor.y));
        CreateWindowExW(
            ex_style,
            class,
            PCWSTR(title.as_ptr()),
            style,
            work.x + (work.width - width) / 2,
            work.y + (work.height - height) / 2,
            width,
            height,
            Some(owner),
            None,
            instance(),
            Some(state),
        )
        .ok()
    }
}

/// Themes a finished dialog for `dark` and shows it in front.
pub(crate) unsafe fn show(dialog: HWND, dark: bool) {
    dark_mode::set_caption_dark(dialog, dark);
    // SAFETY: `dialog` is our live window.
    unsafe {
        theme_children(dialog, dark);
        let _ = ShowWindow(dialog, SW_SHOW);
        let _ = SetForegroundWindow(dialog);
    }
}

/// The parts of a child control that vary between callers.
pub(crate) struct ControlSpec<'a> {
    pub class: PCWSTR,
    pub text: &'a str,
    pub style: u32,
    pub ex_style: WINDOW_EX_STYLE,
    pub bounds: (i32, i32, i32, i32),
    pub id: usize,
}

/// Creates a child control of `parent` with `font`.
pub(crate) unsafe fn control(font: HFONT, parent: HWND, spec: ControlSpec<'_>) -> Option<HWND> {
    let text = wide(spec.text);
    // SAFETY: creating a child of our own window; `text` outlives the call.
    unsafe {
        let hwnd = CreateWindowExW(
            spec.ex_style,
            spec.class,
            PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(spec.style),
            spec.bounds.0,
            spec.bounds.1,
            spec.bounds.2,
            spec.bounds.3,
            Some(parent),
            Some(HMENU(spec.id as *mut core::ffi::c_void)),
            instance(),
            None,
        )
        .ok()?;
        SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        Some(hwnd)
    }
}

/// Gives every child control the dark or light visual style for its class.
unsafe fn theme_children(window: HWND, dark: bool) {
    unsafe extern "system" fn visit(control: HWND, dark: LPARAM) -> BOOL {
        let mut class = [0u16; 32];
        // SAFETY: `control` is a live descendant being enumerated; the buffer
        // is sized for the call.
        let length = unsafe { GetClassNameW(control, &mut class) } as usize;
        let theme = match String::from_utf16_lossy(&class[..length]).as_str() {
            "Button" | "SysListView32" => Some(ControlTheme::Explorer),
            "Edit" => Some(ControlTheme::FileDialog),
            _ => None,
        };
        if let Some(theme) = theme {
            dark_mode::theme_control(control, theme, dark.0 != 0);
        }
        BOOL::from(true)
    }
    // SAFETY: the callback only themes the enumerated controls.
    unsafe {
        let _ = EnumChildWindows(Some(window), Some(visit), LPARAM(dark as isize));
    }
}

/// Runs a modal loop for `dialog` until it is destroyed, with `owner`
/// disabled meanwhile. `IsDialogMessageW` gives the dialog Tab focus movement
/// and turns Enter and Escape into `ID_OK` / `ID_CANCEL`. Every message is
/// pumped, not just the dialog's: a thread that stops retrieving its other
/// windows' messages is marked "not responding" (as in `color_picker::pick`).
pub(crate) unsafe fn run_modal(owner: HWND, dialog: HWND) {
    // SAFETY: both windows belong to this thread; the loop ends when the
    // dialog is gone, and the owner is re-enabled whichever way it ends.
    unsafe {
        let _ = EnableWindow(owner, false);
        let mut message = MSG::default();
        while IsWindow(Some(dialog)).as_bool() {
            let status = GetMessageW(&mut message, None, 0, 0);
            if status.0 == 0 {
                // Another window asked to quit; hand it back to the outer loop.
                PostQuitMessage(message.wParam.0 as i32);
                break;
            }
            if status.0 == -1 {
                break;
            }
            if IsDialogMessageW(dialog, &message).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = EnableWindow(owner, true);
        if IsWindow(Some(dialog)).as_bool() {
            let _ = DestroyWindow(dialog);
        }
    }
}

/// A control's text.
pub(crate) unsafe fn window_text(control: HWND) -> String {
    // SAFETY: the buffer is sized from GetWindowTextLengthW plus the NUL.
    unsafe {
        let length = GetWindowTextLengthW(control).max(0) as usize;
        let mut buffer = vec![0u16; length + 1];
        let copied = GetWindowTextW(control, &mut buffer).max(0) as usize;
        String::from_utf16_lossy(&buffer[..copied])
    }
}

/// Opens the standard file Open dialog over `owner` and returns the files the
/// user picked, or an empty list when they cancelled. Multi-select is on and
/// the dialog starts on the Desktop folder; only existing filesystem items are
/// offered, so every returned path names a real file (F-55).
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileopendialog
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/ne-shobjidl_core-_fileopendialogoptions
pub(crate) unsafe fn pick_files(owner: HWND, title: &str) -> Vec<PathBuf> {
    // SAFETY: as `open_dialog` requires.
    unsafe { open_dialog(owner, title, FOS_FILEMUSTEXIST | FOS_ALLOWMULTISELECT) }
}

/// Opens the Open dialog in "Select folder" mode over `owner` and returns the
/// folder the user picked, or `None` when they cancelled. Only filesystem
/// folders are offered, so a library or "This PC" cannot be picked.
pub(crate) unsafe fn pick_folder(owner: HWND, title: &str) -> Option<PathBuf> {
    // SAFETY: as `open_dialog` requires.
    unsafe { open_dialog(owner, title, FOS_PICKFOLDERS) }
        .into_iter()
        .next()
}

/// The Open dialog over `owner`, titled `title`, with `options` added to its
/// defaults and `FOS_FORCEFILESYSTEM`, starting on the Desktop. Returns the
/// picked paths, empty when cancelled. `owner` must be a live window of this
/// thread, with OLE initialized.
unsafe fn open_dialog(owner: HWND, title: &str, options: FILEOPENDIALOGOPTIONS) -> Vec<PathBuf> {
    // SAFETY: OLE is initialized on this thread; the dialog is modal over
    // `owner` and released when it goes out of scope.
    unsafe {
        let created: windows::core::Result<IFileOpenDialog> =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER);
        let Ok(picker) = created else {
            return Vec::new();
        };
        let Ok(defaults) = picker.GetOptions() else {
            return Vec::new();
        };
        if picker
            .SetOptions(defaults | FOS_FORCEFILESYSTEM | options)
            .is_err()
        {
            return Vec::new();
        }
        let title = wide(title);
        let _ = picker.SetTitle(PCWSTR(title.as_ptr()));
        if let Ok(desktop) =
            SHGetKnownFolderItem::<IShellItem>(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None)
        {
            let _ = picker.SetFolder(&desktop);
        }
        // The owner is a background window, so the dialog would otherwise open
        // behind the active app.
        let _ = SetForegroundWindow(owner);
        // Cancel comes back as an error (HRESULT_FROM_WIN32(ERROR_CANCELLED)).
        if picker.Show(Some(owner)).is_err() {
            return Vec::new();
        }
        let Ok(results) = picker.GetResults() else {
            return Vec::new();
        };
        let Ok(count) = results.GetCount() else {
            return Vec::new();
        };
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let Ok(item) = results.GetItemAt(index) else {
                continue;
            };
            if let Some(path) = display_name(&item, SIGDN_FILESYSPATH) {
                paths.push(PathBuf::from(path));
            }
        }
        paths
    }
}

/// Opens the standard Save dialog over `owner` and returns the path the user
/// chose, or `None` when they cancelled. `filter` lists (name, pattern)
/// pairs such as `("Log files", "*.log")`; the first is selected, and
/// `default_name`'s extension is added to a name typed without one. Windows
/// asks before an existing file is replaced. The debug console's "Save log…"
/// uses it, and C10b's theme export reuses it.
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifilesavedialog
pub(crate) unsafe fn pick_save_path(
    owner: HWND,
    title: &str,
    default_name: &str,
    filter: &[(&str, &str)],
) -> Option<PathBuf> {
    let names: Vec<(Vec<u16>, Vec<u16>)> = filter
        .iter()
        .map(|(name, pattern)| (wide(name), wide(pattern)))
        .collect();
    let specs: Vec<COMDLG_FILTERSPEC> = names
        .iter()
        .map(|(name, pattern)| COMDLG_FILTERSPEC {
            pszName: PCWSTR(name.as_ptr()),
            pszSpec: PCWSTR(pattern.as_ptr()),
        })
        .collect();
    let title = wide(title);
    let default_name_wide = wide(default_name);
    let extension = std::path::Path::new(default_name)
        .extension()
        .map(|extension| wide(&extension.to_string_lossy()));
    // SAFETY: OLE is initialized on this thread; the dialog is modal over
    // `owner`, the strings outlive it, and it is released when it goes out
    // of scope.
    unsafe {
        let picker: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = picker.GetOptions().ok()?;
        picker
            .SetOptions(options | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT)
            .ok()?;
        let _ = picker.SetTitle(PCWSTR(title.as_ptr()));
        let _ = picker.SetFileName(PCWSTR(default_name_wide.as_ptr()));
        if !specs.is_empty() {
            let _ = picker.SetFileTypes(&specs);
        }
        if let Some(extension) = &extension {
            let _ = picker.SetDefaultExtension(PCWSTR(extension.as_ptr()));
        }
        // Cancel comes back as an error (HRESULT_FROM_WIN32(ERROR_CANCELLED)).
        picker.Show(Some(owner)).ok()?;
        let item = picker.GetResult().ok()?;
        display_name(&item, SIGDN_FILESYSPATH).map(PathBuf::from)
    }
}

/// Ends a modal dialog: re-enables `owner` first, so Windows activates it
/// rather than another app when the dialog goes, then destroys the dialog.
pub(crate) unsafe fn close(owner: HWND, dialog: HWND) {
    // SAFETY: both windows belong to this thread.
    unsafe {
        let _ = EnableWindow(owner, true);
        let _ = DestroyWindow(dialog);
    }
}
