//! The icon picker (ROADMAP D16): the icon sets' glyphs as a grid, with a set
//! dropdown, a search box, and Automatic / No icon / OK. Modal, like the
//! installed-apps picker (`app_picker.rs`), whose structure it copies.

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, FillRect, HDC, HGDIOBJ};
use windows::Win32::UI::Controls::{
    EM_SETCUEBANNER, HIMAGELIST, ILC_COLOR32, ImageList_Add, ImageList_Create, ImageList_Destroy,
    LIST_VIEW_ITEM_STATE_FLAGS, LVIF_IMAGE, LVIF_STATE, LVIF_TEXT, LVIS_SELECTED, LVITEMW,
    LVM_ENSUREVISIBLE, LVM_GETNEXTITEM, LVM_REDRAWITEMS, LVM_SETBKCOLOR,
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETIMAGELIST, LVM_SETITEMCOUNT, LVM_SETITEMSTATE,
    LVM_SETTEXTBKCOLOR, LVM_SETTEXTCOLOR, LVN_GETDISPINFOW, LVN_ITEMCHANGED, LVS_EX_DOUBLEBUFFER,
    LVS_ICON, LVS_OWNERDATA, LVS_SHOWSELALWAYS, LVS_SINGLESEL, LVSIL_NORMAL, NM_DBLCLK, NMHDR,
    NMLISTVIEW, NMLVDISPINFOW, WC_LISTVIEWW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE,
    CBS_DROPDOWNLIST, DefWindowProcW, EN_CHANGE, ES_AUTOHSCROLL, GetClassNameW, GetClientRect,
    SendMessageW, SetWindowTextW, WINDOW_EX_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN,
    WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_NCCREATE,
    WM_NCDESTROY, WM_NOTIFY, WS_EX_CLIENTEDGE, WS_TABSTOP,
};
use windows::core::{PCWSTR, w};

use deskspace_core::icon_set::IconSet;
use deskspace_core::{IconRef, icon_match};

use crate::error::PlatformError;
use crate::render::Icon;
use crate::ui::{IconChoice, IconPickerRequest, PickerSet};
use crate::win32::dialog::{self, ControlSpec, DialogChrome, ID_CANCEL, ID_OK};
use crate::win32::drag_image::dib;
use crate::win32::glyphs::GlyphCache;
use crate::win32::settings_window::colorref;
use crate::win32::system::wide;
use crate::win32::window_class::{self, WindowClass};
use crate::win32::window_state;

const CLASS_NAME: PCWSTR = w!("DeskSpaceIconPicker");
const ID_SEARCH: usize = 10;
const ID_LIST: usize = 11;
const ID_SETS: usize = 12;
const ID_NOTE: usize = 13;
const ID_AUTOMATIC: usize = 14;
const ID_NONE: usize = 15;
/// The client size, at 96 DPI.
const CLIENT_WIDTH: i32 = 620;
const CLIENT_HEIGHT: i32 = 520;
/// Cell icons, at 96 DPI.
const ICON_SIZE: i32 = 32;

/// One icon's cell in the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cell {
    /// The icon's name in its set, as saved.
    name: String,
    /// The label the grid shows: the name with hyphens as spaces.
    label: String,
    /// Its image's index in the image list, once drawn.
    image: Option<i32>,
}

/// One set the picker offers, borrowed from the request for the call.
struct PickerSpec<'a> {
    prefix: &'a str,
    label: &'a str,
    note: Option<&'a str>,
    set: Option<&'a IconSet>,
    words: &'a [(&'a str, &'a str)],
}

impl PickerSpec<'_> {
    /// The cells `query` finds in this set.
    fn cells(&self, query: &str) -> Vec<Cell> {
        let Some(set) = self.set else {
            return Vec::new();
        };
        icon_match::search(query, set, self.words)
            .into_iter()
            .map(|name| Cell {
                label: cell_label(name),
                name: name.to_owned(),
                image: None,
            })
            .collect()
    }
}

struct PickerState<'a> {
    owner: HWND,
    chrome: DialogChrome,
    sets: Vec<PickerSpec<'a>>,
    /// The set the grid shows, as an index into `sets`.
    shown: usize,
    /// The cells the search finds in the shown set, in grid order.
    cells: Vec<Cell>,
    images: HIMAGELIST,
    glyphs: GlyphCache,
    search: Option<HWND>,
    list: Option<HWND>,
    note: Option<HWND>,
    ok: Option<HWND>,
    /// The icon the picker opened on, so switching sets can select it again.
    current: Option<IconRef>,
    result: Option<IconChoice>,
}

impl Drop for PickerState<'_> {
    fn drop(&mut self) {
        // SAFETY: the list view that used the image list is destroyed before
        // the state is dropped (after the modal loop ends).
        unsafe {
            let _ = ImageList_Destroy(Some(self.images));
        }
    }
}

/// Shows the icon picker over `owner` and returns the user's choice, or `None`
/// when they cancel. Blocks in a modal loop.
pub(crate) unsafe fn pick_icon(
    owner: HWND,
    request: &IconPickerRequest<'_>,
    dark: bool,
) -> Option<IconChoice> {
    // SAFETY: Win32 window creation on the UI thread. `state` is owned here
    // and dropped after the loop; the window points at it only while alive.
    unsafe {
        if ensure_class().is_err() || request.sets.is_empty() {
            return None;
        }
        let chrome = DialogChrome::new(dialog::dpi_of(owner), dark);
        let icon_px = chrome.s(ICON_SIZE);
        let sets: Vec<PickerSpec<'_>> = request.sets.iter().map(PickerSpec::of).collect();
        let shown = opening_set(&sets, request.current);
        let cells = sets[shown].cells("");
        let client = (chrome.s(CLIENT_WIDTH), chrome.s(CLIENT_HEIGHT));
        let mut state = Box::new(PickerState {
            owner,
            sets,
            shown,
            cells,
            images: ImageList_Create(icon_px, icon_px, ILC_COLOR32, 64, 64),
            glyphs: GlyphCache::new(),
            chrome,
            search: None,
            list: None,
            note: None,
            ok: None,
            current: request.current.cloned(),
            result: None,
        });
        let pointer = &mut *state as *mut PickerState<'_>;
        let dpi = state.chrome.dpi;
        let Some(window) = dialog::create_frame(
            CLASS_NAME,
            request.title,
            owner,
            client,
            dpi,
            pointer.cast(),
        ) else {
            tracing::warn!("could not create the icon picker window");
            return None;
        };
        build(window, &mut state, client, request.sets.len() >= 2);
        fill_list(window);
        select_cell(window, request.current);
        dialog::show(window, dark);
        if let Some(search) = state.search {
            let _ = SetFocus(Some(search));
        }
        dialog::run_modal(owner, window);
        state.result.take()
    }
}

impl<'a> PickerSpec<'a> {
    fn of(set: &PickerSet<'a>) -> Self {
        Self {
            prefix: set.prefix,
            label: set.label,
            note: set.note,
            set: set.set,
            words: set.words,
        }
    }
}

/// The set the picker opens on: the current icon's, else the first.
fn opening_set(sets: &[PickerSpec<'_>], current: Option<&IconRef>) -> usize {
    current
        .and_then(|icon| sets.iter().position(|set| set.prefix == icon.set))
        .unwrap_or(0)
}

/// The cell index the current icon has in `cells`, if the grid shows it.
fn opening_cell(cells: &[Cell], current: Option<&IconRef>, prefix: &str) -> Option<usize> {
    let icon = current?;
    if icon.set != prefix {
        return None;
    }
    cells.iter().position(|cell| cell.name == icon.name)
}

/// A cell's label: its name with hyphens as spaces.
fn cell_label(name: &str) -> String {
    name.replace('-', " ")
}

/// Creates the set dropdown, the search box, the note line, the grid, and the
/// buttons.
unsafe fn build(window: HWND, state: &mut PickerState<'_>, client: (i32, i32), sets_shown: bool) {
    let chrome = &state.chrome;
    let s = |value| chrome.s(value);
    let margin = s(12);
    let button = (s(110), s(30));
    let width = client.0 - 2 * margin;
    let buttons_y = client.1 - margin - button.1;
    let tab = WS_TABSTOP.0;
    // The set dropdown, when there is one, sits above the search box.
    let sets_top = margin;
    let search_top = if sets_shown {
        sets_top + s(26) + s(8)
    } else {
        margin
    };
    let note_top = search_top + s(26) + s(8);
    let list_top = note_top + s(20) + s(8);
    // SAFETY: every control is a child of our live window.
    unsafe {
        if sets_shown {
            let combo = dialog::control(
                chrome.font,
                window,
                ControlSpec {
                    class: w!("ComboBox"),
                    text: "",
                    style: CBS_DROPDOWNLIST as u32 | tab,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds: (margin, sets_top, width, s(240)),
                    id: ID_SETS,
                },
            );
            if let Some(combo) = combo {
                for set in &state.sets {
                    let text = wide(set.label);
                    SendMessageW(
                        combo,
                        CB_ADDSTRING,
                        None,
                        Some(LPARAM(text.as_ptr() as isize)),
                    );
                }
                SendMessageW(combo, CB_SETCURSEL, Some(WPARAM(state.shown)), None);
            }
        }
        state.search = dialog::control(
            chrome.font,
            window,
            ControlSpec {
                class: w!("Edit"),
                text: "",
                style: ES_AUTOHSCROLL as u32 | tab,
                ex_style: WS_EX_CLIENTEDGE,
                bounds: (margin, search_top, width, s(26)),
                id: ID_SEARCH,
            },
        );
        if let Some(search) = state.search {
            let cue = wide("Search icons");
            SendMessageW(
                search,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(cue.as_ptr() as isize)),
            );
        }
        state.note = dialog::control(
            chrome.font,
            window,
            ControlSpec {
                class: w!("Static"),
                text: "",
                style: 0,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: (margin, note_top, width, s(20)),
                id: ID_NOTE,
            },
        );
        state.list = dialog::control(
            chrome.font,
            window,
            ControlSpec {
                class: WC_LISTVIEWW,
                text: "",
                style: LVS_ICON | LVS_OWNERDATA | LVS_SHOWSELALWAYS | LVS_SINGLESEL | tab,
                ex_style: WS_EX_CLIENTEDGE,
                bounds: (margin, list_top, width, buttons_y - s(12) - list_top),
                id: ID_LIST,
            },
        );
        if let Some(list) = state.list {
            configure_list(list, state);
        }
        let button_at = |index: i32| {
            (
                client.0 - margin - button.0 * (index + 1) - s(8) * index,
                buttons_y,
                button.0,
                button.1,
            )
        };
        dialog::control(
            chrome.font,
            window,
            ControlSpec {
                class: w!("Button"),
                text: "Automatic",
                style: BS_PUSHBUTTON as u32 | tab,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: button_at(3),
                id: ID_AUTOMATIC,
            },
        );
        dialog::control(
            chrome.font,
            window,
            ControlSpec {
                class: w!("Button"),
                text: "No icon",
                style: BS_PUSHBUTTON as u32 | tab,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: button_at(2),
                id: ID_NONE,
            },
        );
        dialog::control(
            chrome.font,
            window,
            ControlSpec {
                class: w!("Button"),
                text: "Cancel",
                style: BS_PUSHBUTTON as u32 | tab,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: button_at(1),
                id: ID_CANCEL,
            },
        );
        state.ok = dialog::control(
            chrome.font,
            window,
            ControlSpec {
                class: w!("Button"),
                text: "OK",
                style: BS_DEFPUSHBUTTON as u32 | tab,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: button_at(0),
                id: ID_OK,
            },
        );
        if let Some(ok) = state.ok {
            let _ = EnableWindow(ok, false);
        }
    }
}

/// The grid: one image list, an icon view, and colors that follow dark mode.
unsafe fn configure_list(list: HWND, state: &PickerState<'_>) {
    let style = LVS_EX_DOUBLEBUFFER;
    let colors = state.chrome.colors;
    let background = if colors.dark {
        colors.command_bar
    } else {
        colors.body
    };
    // SAFETY: `list` is our live list view.
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
            Some(WPARAM(LVSIL_NORMAL as usize)),
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
    }
}

/// Refills the grid from the shown set and the search text, and updates the
/// note line.
unsafe fn fill_list(window: HWND) {
    // SAFETY: the cells are built under a short borrow; the list and note
    // messages are sent after it.
    unsafe {
        let Some((list, note_text, count)) = with_state(window, |state| {
            let query = state
                .search
                .map(|search| dialog::window_text(search))
                .unwrap_or_default();
            state.cells = state.sets[state.shown].cells(&query);
            let count = state.cells.len();
            let set = &state.sets[state.shown];
            let note_text = match (set.set, count) {
                (None, _) => set
                    .note
                    .unwrap_or("These icons are not available.")
                    .to_owned(),
                (Some(_), 0) => "No icons match the search.".to_owned(),
                (Some(_), _) => set
                    .note
                    .map_or_else(|| format!("{count} icons"), str::to_owned),
            };
            state.list.map(|list| (list, note_text, count))
        })
        .flatten() else {
            return;
        };
        SendMessageW(list, LVM_SETITEMCOUNT, Some(WPARAM(count)), None);
        if count > 0 {
            SendMessageW(
                list,
                LVM_REDRAWITEMS,
                Some(WPARAM(0)),
                Some(LPARAM(count as isize - 1)),
            );
        }
        if let Some(note) = with_state(window, |state| state.note).flatten() {
            let text = wide(&note_text);
            let _ = SetWindowTextW(note, PCWSTR(text.as_ptr()));
        }
        set_ok_enabled(window);
    }
}

/// Selects the current icon's cell, if the grid shows it.
unsafe fn select_cell(window: HWND, current: Option<&IconRef>) {
    // SAFETY: the borrow ends before the list messages.
    unsafe {
        let Some((list, row)) = with_state(window, |state| {
            let prefix = state.sets[state.shown].prefix;
            let row = opening_cell(&state.cells, current, prefix)?;
            state.list.map(|list| (list, row))
        })
        .flatten() else {
            return;
        };
        let item = LVITEMW {
            stateMask: LVIS_SELECTED,
            state: LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0),
            ..Default::default()
        };
        SendMessageW(
            list,
            LVM_SETITEMSTATE,
            Some(WPARAM(row)),
            Some(LPARAM(&item as *const LVITEMW as isize)),
        );
        SendMessageW(list, LVM_ENSUREVISIBLE, Some(WPARAM(row)), Some(LPARAM(0)));
        set_ok_enabled(window);
    }
}

/// The selected cell's index, if the grid has a selection.
unsafe fn selected_cell(list: HWND) -> Option<usize> {
    // SAFETY: `list` is our live list view.
    let selected = unsafe {
        SendMessageW(
            list,
            LVM_GETNEXTITEM,
            Some(WPARAM(usize::MAX)),
            Some(LPARAM(LVIS_SELECTED.0 as isize)),
        )
    };
    (selected.0 >= 0).then_some(selected.0 as usize)
}

/// Enables OK while a cell is selected.
unsafe fn set_ok_enabled(window: HWND) {
    // SAFETY: the borrow ends before the button is touched.
    unsafe {
        let Some((list, ok)) = with_state(window, |state| (state.list, state.ok)) else {
            return;
        };
        let (Some(list), Some(ok)) = (list, ok) else {
            return;
        };
        let _ = EnableWindow(ok, selected_cell(list).is_some());
    }
}

/// Supplies a cell's label and icon the first time the grid draws it.
unsafe fn on_display_info(window: HWND, info: &mut NMLVDISPINFOW) {
    let index = usize::try_from(info.item.iItem).ok().unwrap_or(usize::MAX);
    let wants_text = info.item.mask.0 & LVIF_TEXT.0 != 0;
    let wants_image = info.item.mask.0 & LVIF_IMAGE.0 != 0;
    // SAFETY: the cell is cloned out under the borrow; the image is rendered
    // after it, and the label is copied into the control's own buffer.
    unsafe {
        let Some(Some((cell, prefix, color, images, size))) = with_state(window, |state| {
            let cell = state.cells.get(index)?.clone();
            Some((
                cell,
                state.sets[state.shown].prefix.to_owned(),
                state.chrome.colors.text,
                state.images,
                state.chrome.s(ICON_SIZE),
            ))
        }) else {
            return;
        };
        if wants_text && !info.item.pszText.is_null() {
            let label: Vec<u16> = cell.label.encode_utf16().collect();
            let capacity = usize::try_from(info.item.cchTextMax).unwrap_or(0).max(1);
            let copy = label.len().min(capacity - 1);
            std::ptr::copy_nonoverlapping(label.as_ptr(), info.item.pszText.0, copy);
            info.item.pszText.0.add(copy).write(0);
        }
        if wants_image {
            let image = match cell.image {
                Some(image) => image,
                None => {
                    let key = format!("{prefix}:{}", cell.name);
                    let name = cell.name.clone();
                    let icon: Option<Icon> = with_state(window, |state| {
                        let set = state.sets[state.shown].set;
                        state
                            .glyphs
                            .icon(&key, size, color, || set.and_then(|set| set.glyph(&name)))
                    })
                    .flatten();
                    let image = icon
                        .and_then(|icon| dib(&icon, false))
                        .map_or(-1, |bitmap| {
                            let image = ImageList_Add(images, bitmap, None);
                            let _ = DeleteObject(HGDIOBJ(bitmap.0));
                            image
                        });
                    with_state(window, |state| {
                        if let Some(cell) = state.cells.get_mut(index) {
                            cell.image = Some(image);
                        }
                    });
                    image
                }
            };
            info.item.iImage = image;
        }
    }
}

/// Switches the grid to the chosen set and selects the current icon there, if
/// it has one.
unsafe fn on_set_changed(window: HWND, combo: HWND, current: Option<&IconRef>) {
    // SAFETY: `combo` is our live combo box; the borrow below makes no Win32
    // calls.
    unsafe {
        let selected = SendMessageW(combo, CB_GETCURSEL, None, None).0;
        with_state(window, |state| {
            if selected >= 0 && (selected as usize) < state.sets.len() {
                state.shown = selected as usize;
                state.cells.clear();
            }
        });
        fill_list(window);
        select_cell(window, current);
    }
}

/// OK: hands back the selected icon and closes. A double-click on a cell
/// arrives here too.
unsafe fn on_ok(window: HWND) {
    // SAFETY: the borrow ends before the dialog is closed.
    unsafe {
        let Some((owner, choice)) = with_state(window, |state| {
            let list = state.list?;
            let row = selected_cell(list)?;
            let cell = state.cells.get(row)?;
            let text = format!("{}:{}", state.sets[state.shown].prefix, cell.name);
            let choice = IconChoice::Icon(IconRef::parse(&text)?);
            state.result = Some(choice.clone());
            Some((state.owner, choice))
        })
        .flatten() else {
            return;
        };
        let _ = choice;
        dialog::close(owner, window);
    }
}

/// Hands back `choice` and closes.
unsafe fn finish(window: HWND, choice: IconChoice) {
    // SAFETY: the borrow ends before the dialog is closed.
    unsafe {
        if let Some(owner) = with_state(window, |state| {
            state.result = Some(choice);
            state.owner
        }) {
            dialog::close(owner, window);
        }
    }
}

unsafe fn on_cancel(window: HWND) {
    // SAFETY: the borrow ends before the dialog is closed.
    unsafe {
        if let Some(owner) = with_state(window, |state| {
            state.result = None;
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
                window_state::attach_create_param::<PickerState<'_>>(window, lparam);
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xFFFF;
                let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                match id {
                    ID_OK => on_ok(window),
                    ID_CANCEL => on_cancel(window),
                    ID_AUTOMATIC => finish(window, IconChoice::Automatic),
                    ID_NONE => finish(window, IconChoice::None),
                    ID_SEARCH if code == EN_CHANGE => fill_list(window),
                    ID_SETS if code == CBN_SELCHANGE => {
                        let current = with_state(window, |state| state.current.clone()).flatten();
                        on_set_changed(
                            window,
                            HWND(lparam.0 as *mut core::ffi::c_void),
                            current.as_ref(),
                        );
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
                        let change = &*(lparam.0 as *const NMLISTVIEW);
                        if change.uChanged.0 & LVIF_STATE.0 != 0 {
                            set_ok_enabled(window);
                        }
                    }
                    Some(NM_DBLCLK) => on_ok(window),
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
                // The state belongs to `pick_icon`, which drops it after the
                // modal loop; only the pointer is cleared here.
                window_state::detach(window);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

fn ensure_class() -> Result<(), PlatformError> {
    window_class::register(WindowClass::new(CLASS_NAME, Some(wndproc)))
}

/// Whether `window` is the icon picker. A press on it is not an outside
/// click, so a rename behind it stays open (D16).
pub(crate) fn is_picker(window: HWND) -> bool {
    let mut class = [0u16; 32];
    // SAFETY: `window` is a live window; the buffer is sized for the call.
    let length = unsafe { GetClassNameW(window, &mut class) } as usize;
    String::from_utf16_lossy(&class[..length]) == "DeskSpaceIconPicker"
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut PickerState<'_>) -> R) -> Option<R> {
    // SAFETY: picker windows hold the `PickerState` `pick_icon` owns for as
    // long as the window exists; the caller rules out re-entry.
    unsafe { window_state::with_state(window, f) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(name: &str) -> Cell {
        Cell {
            name: name.to_owned(),
            label: cell_label(name),
            image: None,
        }
    }

    #[test]
    fn a_cell_label_reads_its_name_with_spaces() {
        assert_eq!(cell_label("arrow-left"), "arrow left");
        assert_eq!(cell_label("games"), "games");
    }

    #[test]
    fn the_picker_opens_on_the_current_icon() {
        let cells = [cell("games"), cell("arrow-left")];
        let icon = IconRef::parse("fluent:arrow-left").unwrap();
        assert_eq!(opening_cell(&cells, Some(&icon), "fluent"), Some(1));
        assert_eq!(opening_cell(&cells, Some(&icon), "other"), None);
        assert_eq!(opening_cell(&cells, None, "fluent"), None);
    }

    #[test]
    fn the_set_opens_on_the_current_icons_set() {
        let spec = |prefix: &'static str| PickerSpec {
            prefix,
            label: prefix,
            note: None,
            set: None,
            words: &[],
        };
        let sets = [spec("fluent"), spec("other")];
        let icon = IconRef::parse("other:games").unwrap();
        assert_eq!(opening_set(&sets, None), 0);
        assert_eq!(opening_set(&sets, Some(&icon)), 1);
    }
}
