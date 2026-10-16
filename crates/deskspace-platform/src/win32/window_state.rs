//! A window's Rust state, kept behind its `GWLP_USERDATA` pointer.
//!
//! Every DeskSpace window class stores a `*mut T` in its user-data slot and
//! reaches it through `with_state`. Two ownership models exist:
//!
//! - **Owned**: the window owns a `Box<T>` (space windows, the settings
//!   window, the settings preview). It is attached in `WM_NCCREATE`, from
//!   `CreateWindowExW`'s `lpCreateParams` (`attach_create_param`) or a fresh
//!   box (`attach`), and freed exactly once in `WM_NCDESTROY` (`free`).
//! - **Borrowed**: a modal function keeps the state on its own stack for the
//!   life of its message loop (the color picker and the dialogs). The window
//!   only points at it and clears the pointer in `WM_NCDESTROY` (`detach`).
//!
//! Callers name `T` at each call. Every window class must use one `T` for
//! every call on its windows; that is the contract the `unsafe` functions
//! here rely on.
//!
//! <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowlongptrw>

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW,
};

/// Stores `state` in `window`'s user-data slot.
///
/// # Safety
///
/// `state` must be null or point at a `T` that stays valid until `free` or
/// `detach` runs for `window`, and `T` must be the type every other call on
/// this window class uses.
pub(crate) unsafe fn attach<T>(window: HWND, state: *mut T) {
    // SAFETY: writing a window long has no memory-safety preconditions.
    unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, state as isize) };
}

/// In `WM_NCCREATE`, stores the `*mut T` passed as `CreateWindowExW`'s
/// `lpCreateParams`. Does nothing when either pointer is null.
///
/// # Safety
///
/// `lparam` must be `WM_NCCREATE`'s, and the create parameter must follow
/// `attach`'s rules.
pub(crate) unsafe fn attach_create_param<T>(window: HWND, lparam: LPARAM) {
    let create = lparam.0 as *const CREATESTRUCTW;
    // SAFETY: in WM_NCCREATE, `lparam` is null or a valid CREATESTRUCTW for
    // the duration of the message.
    let Some(create) = (unsafe { create.as_ref() }) else {
        return;
    };
    let state = create.lpCreateParams as *mut T;
    if !state.is_null() {
        // SAFETY: the caller vouches for the create parameter.
        unsafe { attach(window, state) };
    }
}

/// Runs `f` with a short-lived borrow of `window`'s state, or returns `None`
/// before `attach` or after `free` / `detach`.
///
/// # Safety
///
/// `T` must be the type attached to this window class, and `f` must not call
/// anything that can re-enter the window procedure (which could borrow the
/// state a second time). Sending a message to the window, creating or
/// destroying windows, and dispatching an app event all re-enter.
pub(crate) unsafe fn with_state<T, R>(window: HWND, f: impl FnOnce(&mut T) -> R) -> Option<R> {
    // SAFETY: reading a window long has no memory-safety preconditions.
    let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut T;
    // SAFETY: a non-null slot is a live `T` (the slot is cleared before the
    // state goes away), and the caller rules out a second borrow.
    unsafe { state.as_mut() }.map(f)
}

/// In `WM_NCDESTROY`, clears the slot of a window whose state is borrowed.
///
/// # Safety
///
/// `window` must be a live window of a class that uses this module.
pub(crate) unsafe fn detach(window: HWND) {
    // SAFETY: writing a window long has no memory-safety preconditions.
    unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, 0) };
}

/// In `WM_NCDESTROY`, clears the slot and drops the `Box<T>` it pointed at.
/// Safe to call twice: the second call finds the slot empty.
///
/// # Safety
///
/// The slot must be null or hold a pointer from `Box::<T>::into_raw`, and no
/// borrow from `with_state` may be live.
pub(crate) unsafe fn free<T>(window: HWND) {
    // SAFETY: reading a window long has no memory-safety preconditions.
    let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut T;
    if state.is_null() {
        return;
    }
    // SAFETY: the slot is cleared before the box is dropped, so nothing can
    // reach the state while it is freed; the caller vouches it is a box.
    unsafe {
        detach(window);
        drop(Box::from_raw(state));
    }
}
