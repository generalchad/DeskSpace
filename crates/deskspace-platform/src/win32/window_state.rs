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

use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW,
};

thread_local! {
    // Process state: windows whose state is currently borrowed by `with_state`
    // on this thread. `Vec` retains its capacity across pops, so after the
    // first borrow the paint path allocates nothing.
    static BORROWED: RefCell<Vec<HWND>> = const { RefCell::new(Vec::new()) };
}

/// Removes `self.0` from `BORROWED` when the `with_state` call ends or unwinds.
struct BorrowGuard(HWND);

impl Drop for BorrowGuard {
    fn drop(&mut self) {
        BORROWED.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            if let Some(index) = borrowed.iter().rposition(|&hwnd| hwnd == self.0) {
                borrowed.swap_remove(index);
            }
        });
    }
}

/// Marks `window` as borrowed on this thread, or returns `None` if a borrow
/// for `window` is already active.
fn enter_borrow(window: HWND) -> Option<BorrowGuard> {
    BORROWED.with(|cell| {
        let mut borrowed = cell.borrow_mut();
        if borrowed.contains(&window) {
            return None;
        }
        borrowed.push(window);
        Some(BorrowGuard(window))
    })
}

fn is_borrowed(window: HWND) -> bool {
    BORROWED.with(|cell| cell.borrow().contains(&window))
}

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
/// before `attach`, after `free` / `detach`, or if `window` is already
/// borrowed on this thread (logged at `error`).
///
/// # Safety
///
/// `T` must be the type attached to this window class, and `f` must not call
/// anything that can re-enter the window procedure. Sending a message to the
/// window, creating or destroying windows, and dispatching an app event all
/// re-enter; a nested `with_state` on the same window returns `None` at run
/// time rather than aliasing `&mut T`.
pub(crate) unsafe fn with_state<T, R>(window: HWND, f: impl FnOnce(&mut T) -> R) -> Option<R> {
    // SAFETY: reading a window long has no memory-safety preconditions.
    let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut T;
    if state.is_null() {
        return None;
    }
    let Some(guard) = enter_borrow(window) else {
        tracing::error!(window = ?window.0, "window state re-entered while borrowed");
        return None;
    };
    // SAFETY: `state` is non-null (attached and not yet detached or freed),
    // `guard` ensures no other `with_state` borrow on `window` is active on
    // this thread and `free` will not drop `state` while `guard` is live,
    // and the caller vouches for `T`.
    let result = f(unsafe { &mut *state });
    drop(guard);
    Some(result)
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
/// If `window`'s state is currently borrowed by `with_state` on this thread,
/// clears the slot, logs at `error`, and leaks the `Box<T>` instead of
/// freeing it under the live borrow.
///
/// # Safety
///
/// The slot must be null or hold a pointer from `Box::<T>::into_raw`, and `T`
/// must match the pointer's type.
pub(crate) unsafe fn free<T>(window: HWND) {
    // SAFETY: reading a window long has no memory-safety preconditions.
    let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut T;
    if state.is_null() {
        return;
    }
    // SAFETY: clearing the slot before freeing (or leaking) keeps any
    // subsequent lookup from reaching `state`.
    unsafe { detach(window) };
    if is_borrowed(window) {
        tracing::error!(
            window = ?window.0,
            "window state freed while borrowed; leaking it to avoid use-after-free"
        );
        return;
    }
    // SAFETY: the slot is cleared, no `with_state` borrow for `window` is
    // live on this thread, and the caller vouches it came from
    // `Box::<T>::into_raw`.
    unsafe { drop(Box::from_raw(state)) };
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use windows::Win32::Foundation::{LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
    };
    use windows::core::{PCWSTR, w};

    use super::*;
    use crate::win32::window_class::{self, WindowClass};

    const TEST_CLASS: PCWSTR = w!("DeskSpaceWindowStateTest");

    unsafe extern "system" fn test_wndproc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: forwards Windows' own arguments unchanged.
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    struct TestWindow(HWND);

    impl TestWindow {
        fn new() -> Self {
            let class = WindowClass::new(TEST_CLASS, Some(test_wndproc));
            window_class::register(class).expect("register test window class");
            let instance = window_class::instance().expect("module handle");
            // SAFETY: `TEST_CLASS` was just registered with `instance` on this
            // thread, and a message-only window is owned by this thread.
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    TEST_CLASS,
                    w!(""),
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    Some(HWND_MESSAGE),
                    None,
                    Some(instance),
                    None,
                )
            }
            .expect("create message-only test window");
            Self(hwnd)
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            // SAFETY: `self.0` was created on this thread and is destroyed once.
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }

    struct TrackDrop {
        value: i32,
        dropped: Arc<AtomicBool>,
    }

    impl Drop for TrackDrop {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn nested_with_state_on_one_window_returns_none_and_keeps_outer_borrow() {
        let win = TestWindow::new();
        // SAFETY: `win.0` is a live window of our test class and holds `Box<i32>`.
        unsafe { attach(win.0, Box::into_raw(Box::new(10_i32))) };

        // SAFETY: `win.0` holds a `Box<i32>` attached above.
        let result = unsafe {
            with_state::<i32, _>(win.0, |outer| {
                *outer += 1;
                // SAFETY: testing re-entrancy on the same window; returns `None`.
                let inner = with_state::<i32, _>(win.0, |value| {
                    *value += 100;
                    *value
                });
                assert_eq!(inner, None);
                *outer += 2;
                *outer
            })
        };
        assert_eq!(result, Some(13));
        // SAFETY: outer borrow ended; subsequent borrow works again.
        let after = unsafe { with_state::<i32, _>(win.0, |value| *value) };
        assert_eq!(after, Some(13));

        // SAFETY: `win.0` still owns the `Box<i32>` and no borrow is active.
        unsafe { free::<i32>(win.0) };
        // SAFETY: freed above; reading detached state returns `None`.
        let detached = unsafe { with_state::<i32, _>(win.0, |value| *value) };
        assert_eq!(detached, None);
    }

    #[test]
    fn nested_borrows_of_two_different_windows_both_work() {
        let win_a = TestWindow::new();
        let win_b = TestWindow::new();
        // SAFETY: each window is live and gets its own `Box<i32>`.
        unsafe {
            attach(win_a.0, Box::into_raw(Box::new(1_i32)));
            attach(win_b.0, Box::into_raw(Box::new(2_i32)));
        }

        // SAFETY: `win_a` and `win_b` each hold a `Box<i32>`.
        let sum = unsafe {
            with_state::<i32, _>(win_a.0, |a| {
                *a += 10;
                // SAFETY: `win_b` is a distinct window not currently borrowed.
                let b_result = with_state::<i32, _>(win_b.0, |b| {
                    *b += 20;
                    // SAFETY: re-entering either borrowed window returns `None`.
                    assert_eq!(with_state::<i32, _>(win_a.0, |v| *v), None);
                    // SAFETY: re-entering either borrowed window returns `None`.
                    assert_eq!(with_state::<i32, _>(win_b.0, |v| *v), None);
                    *b
                });
                assert_eq!(b_result, Some(22));
                *a + b_result.unwrap()
            })
        };
        assert_eq!(sum, Some(33));

        // SAFETY: both borrows ended; both windows remain independently usable.
        unsafe {
            assert_eq!(with_state::<i32, _>(win_a.0, |a| *a), Some(11));
            assert_eq!(with_state::<i32, _>(win_b.0, |b| *b), Some(22));
            free::<i32>(win_a.0);
            free::<i32>(win_b.0);
        }
    }

    #[test]
    fn free_while_borrowed_leaks_instead_of_dropping_under_live_borrow() {
        let win = TestWindow::new();
        let dropped_normal = Arc::new(AtomicBool::new(false));
        // SAFETY: `win.0` gets an owned `Box<TrackDrop>`.
        unsafe {
            attach(
                win.0,
                Box::into_raw(Box::new(TrackDrop {
                    value: 1,
                    dropped: Arc::clone(&dropped_normal),
                })),
            );
            free::<TrackDrop>(win.0);
        }
        assert!(dropped_normal.load(Ordering::SeqCst));

        let dropped_leaked = Arc::new(AtomicBool::new(false));
        // SAFETY: `win.0` gets a second owned `Box<TrackDrop>`.
        unsafe {
            attach(
                win.0,
                Box::into_raw(Box::new(TrackDrop {
                    value: 42,
                    dropped: Arc::clone(&dropped_leaked),
                })),
            );
        }

        // SAFETY: `win.0` holds the `Box<TrackDrop>` attached above.
        let value = unsafe {
            with_state::<TrackDrop, _>(win.0, |state| {
                // SAFETY: `free` during a live borrow detaches and leaks rather
                // than dropping `state` out from under this reference.
                free::<TrackDrop>(win.0);
                assert!(!state.dropped.load(Ordering::SeqCst));
                state.value += 1;
                // SAFETY: detached above, so a nested lookup returns `None`.
                assert_eq!(with_state::<TrackDrop, _>(win.0, |v| v.value), None);
                state.value
            })
        };
        assert_eq!(value, Some(43));
        assert!(
            !dropped_leaked.load(Ordering::SeqCst),
            "borrowed state must be leaked rather than freed early"
        );
        assert!(!is_borrowed(win.0));

        // SAFETY: `win.0` was detached; a fresh state attaches and frees cleanly.
        unsafe {
            assert_eq!(with_state::<TrackDrop, _>(win.0, |v| v.value), None);
            attach(win.0, Box::into_raw(Box::new(99_i32)));
            assert_eq!(with_state::<i32, _>(win.0, |v| *v), Some(99));
            free::<i32>(win.0);
        }
    }

    #[test]
    fn borrow_is_released_when_closure_panics() {
        let win = TestWindow::new();
        // SAFETY: `win.0` gets an owned `Box<i32>`.
        unsafe { attach(win.0, Box::into_raw(Box::new(5_i32))) };

        let panicked = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: `win.0` holds the `Box<i32>` attached above.
            unsafe {
                with_state::<i32, ()>(win.0, |value| {
                    *value = 9;
                    panic!("simulated panic inside with_state");
                });
            }
        }));
        assert!(panicked.is_err());
        assert!(!is_borrowed(win.0));

        // SAFETY: the RAII guard released the borrow when unwinding.
        unsafe {
            assert_eq!(with_state::<i32, _>(win.0, |value| *value), Some(9));
            free::<i32>(win.0);
        }
    }

    #[test]
    fn repeated_borrows_do_not_allocate_after_first_use() {
        let win = TestWindow::new();
        // SAFETY: `win.0` gets an owned `Box<i32>`.
        unsafe {
            attach(win.0, Box::into_raw(Box::new(0_i32)));
            assert_eq!(with_state::<i32, _>(win.0, |value| *value += 1), Some(()));
        }

        let capacity = BORROWED.with(|cell| cell.borrow().capacity());
        assert!(capacity > 0);

        for _ in 0..100 {
            // SAFETY: `win.0` holds the `Box<i32>` attached above.
            unsafe {
                assert_eq!(with_state::<i32, _>(win.0, |value| *value += 1), Some(()));
            }
        }
        assert_eq!(
            BORROWED.with(|cell| (cell.borrow().len(), cell.borrow().capacity())),
            (0, capacity)
        );

        // SAFETY: `win.0` still owns the `Box<i32>` and no borrow is active.
        unsafe { free::<i32>(win.0) };
    }
}
