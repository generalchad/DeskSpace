//! The installed-apps picker (ROADMAP F-53): a modal window listing every app
//! in the shell's Applications folder (`shell:AppsFolder`, which holds Store
//! apps as well as desktop programs), with a check box per row, a search box
//! that filters as you type, and Add / Cancel.
//!
//! The list is a standard report-view list view. Icons are rendered lazily,
//! the first time a row is drawn (`LVN_GETDISPINFOW`), because rendering a few
//! hundred icons up front would delay the window.

use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, FillRect, HDC, HGDIOBJ};
use windows::Win32::UI::Controls::{
    EM_SETCUEBANNER, HIMAGELIST, I_IMAGECALLBACK, ILC_COLOR32, ImageList_Add, ImageList_Create,
    ImageList_Destroy, LIST_VIEW_ITEM_STATE_FLAGS, LVCF_WIDTH, LVCOLUMNW, LVCOLUMNW_MASK,
    LVIF_IMAGE, LVIF_PARAM, LVIF_STATE, LVIF_TEXT, LVIS_STATEIMAGEMASK, LVITEMW,
    LVM_DELETEALLITEMS, LVM_INSERTCOLUMNW, LVM_INSERTITEMW, LVM_SETBKCOLOR, LVM_SETCOLUMNWIDTH,
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETIMAGELIST, LVM_SETITEMSTATE, LVM_SETTEXTBKCOLOR,
    LVM_SETTEXTCOLOR, LVN_GETDISPINFOW, LVN_ITEMCHANGED, LVS_EX_CHECKBOXES, LVS_EX_DOUBLEBUFFER,
    LVS_EX_FULLROWSELECT, LVS_NOCOLUMNHEADER, LVS_REPORT, LVS_SHOWSELALWAYS, LVS_SINGLESEL,
    LVSCW_AUTOSIZE_USEHEADER, LVSIL_SMALL, NMHDR, NMLISTVIEW, NMLVDISPINFOW, WC_LISTVIEWW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, KF_FLAG_DEFAULT,
    SHGetKnownFolderItem,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CREATESTRUCTW, DefWindowProcW, EN_CHANGE, ES_AUTOHSCROLL,
    GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, RegisterClassW, SendMessageW,
    SetWindowLongPtrW, SetWindowTextW, WINDOW_EX_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN,
    WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_NCCREATE,
    WM_NCDESTROY, WM_NOTIFY, WNDCLASSW, WS_EX_CLIENTEDGE, WS_TABSTOP,
};
use windows::core::{PCWSTR, PWSTR, w};

use crate::input::ShellItem;
use crate::win32::dialog::{self, DialogChrome, ID_CANCEL, ID_OK, window_text};
use crate::win32::drag_image::dib;
use crate::win32::icons::shell_item_icon;
use crate::win32::settings_window::colorref;
use crate::win32::shortcuts::shell_item;
use crate::win32::system::wide;

const CLASS_NAME: PCWSTR = w!("DeskSpaceAppPicker");
const ID_SEARCH: usize = 10;
const ID_LIST: usize = 11;
/// The client size, at 96 DPI.
const CLIENT_WIDTH: i32 = 440;
const CLIENT_HEIGHT: i32 = 560;
/// Row icons, at 96 DPI.
const ICON_SIZE: i32 = 24;
/// A list-view state image index lives in bits 12-15 of the item state; with
/// `LVS_EX_CHECKBOXES` image 1 is unchecked and 2 is checked.
/// https://learn.microsoft.com/en-us/windows/win32/controls/extended-list-view-styles
const UNCHECKED: u32 = 1 << 12;
const CHECKED: u32 = 2 << 12;

static REGISTER_CLASS: Once = Once::new();
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

/// One app in the list.
struct AppRow {
    item: ShellItem,
    shell: IShellItem,
    checked: bool,
    /// Its icon's index in the image list, once rendered.
    image: Option<i32>,
}

struct PickerState {
    owner: HWND,
    chrome: DialogChrome,
    apps: Vec<AppRow>,
    /// Indices into `apps` of the rows the search shows, in list order.
    visible: Vec<usize>,
    images: HIMAGELIST,
    search: Option<HWND>,
    list: Option<HWND>,
    add: Option<HWND>,
    /// Set while the list is refilled, so the check-state notifications it
    /// raises are not taken as clicks.
    filling: bool,
    result: Vec<ShellItem>,
}

impl Drop for PickerState {
    fn drop(&mut self) {
        // SAFETY: the list view that used the image list is destroyed before
        // the state is dropped (after the modal loop ends).
        unsafe {
            let _ = ImageList_Destroy(Some(self.images));
        }
    }
}

/// Shows the picker titled `title` over `owner` and returns the checked apps,
/// in list order. Empty when the user cancels. Blocks in a modal loop.
pub(crate) unsafe fn pick_apps(owner: HWND, title: &str, dark: bool) -> Vec<ShellItem> {
    // SAFETY: Win32 window creation on the UI thread. `state` is owned here
    // and dropped after the loop; the window points at it only while alive.
    unsafe {
        if !ensure_class() {
            return Vec::new();
        }
        let chrome = DialogChrome::new(dialog::dpi_of(owner), dark);
        let icon_px = chrome.s(ICON_SIZE);
        let mut apps: Vec<AppRow> = installed_apps()
            .into_iter()
            .map(|(item, shell)| AppRow {
                item,
                shell,
                checked: false,
                image: None,
            })
            .collect();
        apps.sort_by_cached_key(|app| app.item.name.to_lowercase());
        let client = (chrome.s(CLIENT_WIDTH), chrome.s(CLIENT_HEIGHT));
        let mut state = Box::new(PickerState {
            owner,
            visible: (0..apps.len()).collect(),
            apps,
            images: ImageList_Create(icon_px, icon_px, ILC_COLOR32, 64, 64),
            chrome,
            search: None,
            list: None,
            add: None,
            filling: false,
            result: Vec::new(),
        });
        let pointer = &mut *state as *mut PickerState;
        let dpi = state.chrome.dpi;
        let Some(window) =
            dialog::create_frame(CLASS_NAME, title, owner, client, dpi, pointer.cast())
        else {
            tracing::warn!("could not create the app picker window");
            return Vec::new();
        };
        build(window, &mut state, client);
        fill_list(window);
        dialog::show(window, dark);
        if let Some(search) = state.search {
            let _ = SetFocus(Some(search));
        }
        dialog::run_modal(owner, window);
        std::mem::take(&mut state.result)
    }
}

/// Every app in the Applications folder, with its shell item.
/// https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid (FOLDERID_AppsFolder)
fn installed_apps() -> Vec<(ShellItem, IShellItem)> {
    let mut apps = Vec::new();
    // SAFETY: OLE is initialized on this thread; every interface is released
    // when it goes out of scope.
    unsafe {
        let Ok(folder) =
            SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)
        else {
            tracing::warn!("cannot open the Applications folder");
            return apps;
        };
        let Ok(items) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else {
            tracing::warn!("cannot enumerate the Applications folder");
            return apps;
        };
        loop {
            let mut batch = [None];
            let mut fetched = 0u32;
            if items.Next(&mut batch, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            if let Some(shell) = batch[0].take()
                && let Some(item) = shell_item(&shell)
                && !item.name.trim().is_empty()
            {
                apps.push((item, shell));
            }
        }
    }
    tracing::debug!(count = apps.len(), "listed installed apps");
    apps
}

/// Creates the search box, the list, and the buttons.
unsafe fn build(window: HWND, state: &mut PickerState, client: (i32, i32)) {
    let chrome = &state.chrome;
    let s = |value| chrome.s(value);
    let margin = s(12);
    let button = (s(96), s(30));
    let width = client.0 - 2 * margin;
    let buttons_y = client.1 - margin - button.1;
    let list_top = margin + s(26) + s(10);
    let tab = WS_TABSTOP.0;
    // SAFETY: every control is a child of our live window.
    unsafe {
        state.search = dialog::control(
            chrome,
            window,
            w!("Edit"),
            "",
            ES_AUTOHSCROLL as u32 | tab,
            WS_EX_CLIENTEDGE,
            (margin, margin, width, s(26)),
            ID_SEARCH,
        );
        if let Some(search) = state.search {
            let cue = wide("Search apps");
            SendMessageW(
                search,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(cue.as_ptr() as isize)),
            );
        }
        state.list = dialog::control(
            chrome,
            window,
            WC_LISTVIEWW,
            "",
            LVS_REPORT | LVS_NOCOLUMNHEADER | LVS_SHOWSELALWAYS | LVS_SINGLESEL | tab,
            WS_EX_CLIENTEDGE,
            (margin, list_top, width, buttons_y - s(12) - list_top),
            ID_LIST,
        );
        if let Some(list) = state.list {
            configure_list(list, state);
        }
        state.add = dialog::control(
            chrome,
            window,
            w!("Button"),
            &add_caption(0),
            BS_DEFPUSHBUTTON as u32 | tab,
            WINDOW_EX_STYLE(0),
            (
                client.0 - margin - 2 * button.0 - s(8),
                buttons_y,
                button.0,
                button.1,
            ),
            ID_OK,
        );
        if let Some(add) = state.add {
            let _ = EnableWindow(add, false);
        }
        dialog::control(
            chrome,
            window,
            w!("Button"),
            "Cancel",
            BS_PUSHBUTTON as u32 | tab,
            WINDOW_EX_STYLE(0),
            (client.0 - margin - button.0, buttons_y, button.0, button.1),
            ID_CANCEL,
        );
    }
}

/// Check boxes, full-row selection, one column, the image list, and colors
/// that follow the dark/light setting.
unsafe fn configure_list(list: HWND, state: &PickerState) {
    let style = LVS_EX_CHECKBOXES | LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER;
    let colors = state.chrome.colors;
    let background = if colors.dark {
        colors.command_bar
    } else {
        colors.body
    };
    // SAFETY: `list` is our live list view; the column struct outlives the
    // message that copies it.
    unsafe {
        SendMessageW(
            list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            Some(WPARAM(style as usize)),
            Some(LPARAM(style as isize)),
        );
        SendMessageW(
            list,
            LVM_SETIMAGELIST,
            Some(WPARAM(LVSIL_SMALL as usize)),
            Some(LPARAM(state.images.0)),
        );
        for message in [LVM_SETBKCOLOR, LVM_SETTEXTBKCOLOR] {
            SendMessageW(
                list,
                message,
                None,
                Some(LPARAM(colorref(background).0 as isize)),
            );
        }
        SendMessageW(
            list,
            LVM_SETTEXTCOLOR,
            None,
            Some(LPARAM(colorref(colors.text).0 as isize)),
        );
        let column = LVCOLUMNW {
            mask: LVCOLUMNW_MASK(LVCF_WIDTH.0),
            cx: 100,
            ..Default::default()
        };
        SendMessageW(
            list,
            LVM_INSERTCOLUMNW,
            Some(WPARAM(0)),
            Some(LPARAM(&column as *const LVCOLUMNW as isize)),
        );
    }
}

/// Refills the list with the rows the search shows, keeping their checks.
unsafe fn fill_list(window: HWND) {
    // SAFETY: the rows are copied out under a short borrow; the list messages
    // are sent after it, with `filling` set so the check notifications they
    // raise are ignored.
    unsafe {
        let Some((list, rows)) = with_state(window, |state| {
            state.filling = true;
            let rows: Vec<(usize, String, bool)> = state
                .visible
                .iter()
                .map(|&index| {
                    let app = &state.apps[index];
                    (index, app.item.name.clone(), app.checked)
                })
                .collect();
            state.list.map(|list| (list, rows))
        })
        .flatten() else {
            return;
        };
        SendMessageW(list, LVM_DELETEALLITEMS, None, None);
        for (row, (index, name, checked)) in rows.into_iter().enumerate() {
            let mut text = wide(&name);
            let item = LVITEMW {
                mask: LVIF_TEXT | LVIF_IMAGE | LVIF_PARAM,
                iItem: row as i32,
                pszText: PWSTR(text.as_mut_ptr()),
                iImage: I_IMAGECALLBACK,
                lParam: LPARAM(index as isize),
                ..Default::default()
            };
            SendMessageW(
                list,
                LVM_INSERTITEMW,
                None,
                Some(LPARAM(&item as *const LVITEMW as isize)),
            );
            let state_item = LVITEMW {
                stateMask: LVIS_STATEIMAGEMASK,
                state: LIST_VIEW_ITEM_STATE_FLAGS(if checked { CHECKED } else { UNCHECKED }),
                ..Default::default()
            };
            SendMessageW(
                list,
                LVM_SETITEMSTATE,
                Some(WPARAM(row)),
                Some(LPARAM(&state_item as *const LVITEMW as isize)),
            );
        }
        SendMessageW(
            list,
            LVM_SETCOLUMNWIDTH,
            Some(WPARAM(0)),
            Some(LPARAM(LVSCW_AUTOSIZE_USEHEADER as isize)),
        );
        with_state(window, |state| state.filling = false);
    }
}

/// Re-filters the list after the search text changed.
unsafe fn on_search(window: HWND, search: HWND) {
    // SAFETY: `search` is our live edit control; the borrow below makes no
    // Win32 calls.
    unsafe {
        let query = window_text(search);
        with_state(window, |state| {
            let names: Vec<&str> = state
                .apps
                .iter()
                .map(|app| app.item.name.as_str())
                .collect();
            state.visible = matching_apps(&names, &query);
        });
        fill_list(window);
    }
}

/// Supplies a row's icon the first time the list draws it.
unsafe fn on_display_info(window: HWND, info: &mut NMLVDISPINFOW) {
    if info.item.mask.0 & LVIF_IMAGE.0 == 0 {
        return;
    }
    let index = info.item.lParam.0 as usize;
    // SAFETY: the shell item is cloned out under the borrow, rendered after
    // it, and the image list only stores the bitmap's copy.
    unsafe {
        let Some(Some((known, shell, images, size))) = with_state(window, |state| {
            let app = state.apps.get(index)?;
            Some((
                app.image,
                app.shell.clone(),
                state.images,
                state.chrome.s(ICON_SIZE),
            ))
        }) else {
            return;
        };
        let image = match known {
            Some(image) => image,
            None => {
                let image = shell_item_icon(&shell, size as u32)
                    .and_then(|icon| dib(&icon, false))
                    .map_or(-1, |bitmap| {
                        let image = ImageList_Add(images, bitmap, None);
                        let _ = DeleteObject(HGDIOBJ(bitmap.0));
                        image
                    });
                with_state(window, |state| {
                    if let Some(app) = state.apps.get_mut(index) {
                        app.image = Some(image);
                    }
                });
                image
            }
        };
        info.item.iImage = image;
    }
}

/// Records a check box the user toggled and updates the Add button.
unsafe fn on_item_changed(window: HWND, change: &NMLISTVIEW) {
    let toggled = (change.uNewState ^ change.uOldState) & LVIS_STATEIMAGEMASK.0 != 0;
    if change.uChanged.0 & LVIF_STATE.0 == 0 || !toggled {
        return;
    }
    let checked = change.uNewState & LVIS_STATEIMAGEMASK.0 == CHECKED;
    let index = change.lParam.0 as usize;
    // SAFETY: the borrow ends before the button is touched.
    unsafe {
        let Some(Some((add, count))) = with_state(window, |state| {
            if state.filling {
                return None;
            }
            state.apps.get_mut(index)?.checked = checked;
            let count = state.apps.iter().filter(|app| app.checked).count();
            Some((state.add, count))
        }) else {
            return;
        };
        if let Some(add) = add {
            let caption = wide(&add_caption(count));
            let _ = SetWindowTextW(add, PCWSTR(caption.as_ptr()));
            let _ = EnableWindow(add, count > 0);
        }
    }
}

/// Add: hands back the checked apps and closes. Nothing checked does nothing
/// (Enter in the search box arrives here too).
unsafe fn on_add(window: HWND) {
    // SAFETY: the borrow ends before the dialog is closed.
    unsafe {
        let Some(owner) = with_state(window, |state| {
            state.result = state
                .apps
                .iter()
                .filter(|app| app.checked)
                .map(|app| app.item.clone())
                .collect();
            (!state.result.is_empty()).then_some(state.owner)
        })
        .flatten() else {
            return;
        };
        dialog::close(owner, window);
    }
}

unsafe fn on_cancel(window: HWND) {
    // SAFETY: the borrow ends before the dialog is closed.
    unsafe {
        if let Some(owner) = with_state(window, |state| {
            state.result.clear();
            state.owner
        }) {
            dialog::close(owner, window);
        }
    }
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: called by Windows with valid arguments for `message`; state
    // access follows the rules on `with_state`.
    unsafe {
        match message {
            WM_NCCREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                if let Some(create) = create.as_ref() {
                    SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xFFFF;
                let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                match id {
                    ID_OK => on_add(window),
                    ID_CANCEL => on_cancel(window),
                    ID_SEARCH if code == EN_CHANGE => {
                        on_search(window, HWND(lparam.0 as *mut core::ffi::c_void));
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_NOTIFY => {
                let header = lparam.0 as *const NMHDR;
                match header.as_ref().map(|header| header.code) {
                    Some(LVN_GETDISPINFOW) => {
                        on_display_info(window, &mut *(lparam.0 as *mut NMLVDISPINFOW));
                    }
                    Some(LVN_ITEMCHANGED) => {
                        on_item_changed(window, &*(lparam.0 as *const NMLISTVIEW));
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                with_state(window, |state| state.chrome.control_color(message, wparam))
                    .flatten()
                    .unwrap_or_else(|| DefWindowProcW(window, message, wparam, lparam))
            }
            WM_ERASEBKGND => {
                let Some(brush) = with_state(window, |state| state.chrome.body_brush) else {
                    return DefWindowProcW(window, message, wparam, lparam);
                };
                let mut client = RECT::default();
                let _ = GetClientRect(window, &mut client);
                FillRect(HDC(wparam.0 as *mut core::ffi::c_void), &client, brush);
                LRESULT(1)
            }
            WM_CLOSE => {
                on_cancel(window);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                // The state belongs to `pick_apps`, which drops it after the
                // modal loop; only the pointer is cleared here.
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

fn ensure_class() -> bool {
    REGISTER_CLASS.call_once(|| {
        let Some(instance) = dialog::instance() else {
            return;
        };
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `class` is fully initialised and its name is static.
        CLASS_REGISTERED.store(unsafe { RegisterClassW(&class) } != 0, Ordering::SeqCst);
    });
    CLASS_REGISTERED.load(Ordering::SeqCst)
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut PickerState) -> R) -> Option<R> {
    // SAFETY: the pointer is the live state `pick_apps` owns for as long as
    // the window exists, and `f` cannot re-enter to create a second borrow.
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut PickerState;
    unsafe { pointer.as_mut() }.map(f)
}

/// The indices of the apps whose names contain every word of `query`,
/// ignoring case, in their original order. An empty query matches all.
fn matching_apps(names: &[&str], query: &str) -> Vec<usize> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    names
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            let name = name.to_lowercase();
            words.iter().all(|word| name.contains(word.as_str()))
        })
        .map(|(index, _)| index)
        .collect()
}

/// The Add button's caption: the number of checked apps once there are any.
fn add_caption(count: usize) -> String {
    if count == 0 {
        "Add".to_string()
    } else {
        format!("Add ({count})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_search_matches_every_word_ignoring_case() {
        let names = [
            "Visual Studio Code",
            "Calculator",
            "Microsoft Visual Studio",
        ];
        assert_eq!(matching_apps(&names, ""), [0, 1, 2]);
        assert_eq!(matching_apps(&names, "  "), [0, 1, 2]);
        assert_eq!(matching_apps(&names, "visual"), [0, 2]);
        assert_eq!(matching_apps(&names, "studio CODE"), [0]);
        assert!(matching_apps(&names, "paint").is_empty());
    }

    #[test]
    fn the_applications_folder_lists_apps_with_id_lists() {
        use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
        // SAFETY: initializes COM for this test thread only.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let apps = installed_apps();
        assert!(!apps.is_empty(), "every Windows install has some apps");
        assert!(apps.iter().all(|(item, _)| !item.id_list.is_empty()));
        // Store apps have no filesystem path; they must still be listed.
        let virtual_apps = apps.iter().filter(|(item, _)| item.path.is_none()).count();
        assert!(virtual_apps > 0, "Store apps are listed");
    }

    #[test]
    fn the_add_button_counts_the_checked_apps() {
        assert_eq!(add_caption(0), "Add");
        assert_eq!(add_caption(3), "Add (3)");
    }
}
