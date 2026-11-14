//! The thread DeskSpace's low-level hooks run on.
//!
//! Windows calls a low-level hook on the thread that installed it, and every
//! key or mouse event in the session waits for its answer. A thread that
//! takes longer than `LowLevelHooksTimeout` (a second at most) loses the
//! hook, and nothing tells it so. Both hooks used to run on the UI thread,
//! where one stall (a slow network lens, a save to a redirected profile)
//! ended every key on a space until the hook was next installed. This thread
//! only waits for messages, so no work elsewhere can time a hook out, and
//! the pointer and the keyboard never wait for the UI thread.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
//!
//! The UI thread says which hooks it wants and goes on; it never waits for
//! this thread. A hook is therefore installed, and removed, a moment after
//! it is asked for, so a hook procedure decides what a key or press means
//! from its module's own state, not from being installed.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::thread;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, HHOOK, HOOKPROC, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
    SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_NULL,
    WM_USER,
};

use crate::win32::{keyboard, outside_click};

/// A low-level hook the thread can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hook {
    /// `keyboard`'s: the keys of a rename, a selection, and a current space.
    Keyboard,
    /// `outside_click`'s: presses outside every space.
    Mouse,
}

impl Hook {
    const ALL: [Self; 2] = [Self::Keyboard, Self::Mouse];

    fn bit(self) -> u8 {
        1 << self as u8
    }

    fn kind(self) -> WINDOWS_HOOK_ID {
        match self {
            Self::Keyboard => WH_KEYBOARD_LL,
            Self::Mouse => WH_MOUSE_LL,
        }
    }

    fn procedure(self) -> HOOKPROC {
        match self {
            Self::Keyboard => Some(keyboard::hook_proc),
            Self::Mouse => Some(outside_click::hook_proc),
        }
    }
}

// Process state: the hook thread, the hooks the UI thread wants, and the
// hooks the thread holds. A hook procedure gets no context and the thread
// outlives every window, so neither can be handed this. `set` changes what
// is wanted; the thread and its state last for the process.
static HOOKS: HookThread = HookThread::new();

/// Asks for `hook` to be installed while `wanted`, and returns at once: the
/// thread installs or removes it a moment later. Idempotent.
pub(crate) fn set(hook: Hook, wanted: bool) {
    HOOKS.set(hook, wanted);
}

/// Whether the thread holds `hook` now.
pub(crate) fn holds(hook: Hook) -> bool {
    HOOKS.holds(hook)
}

/// What brings one hook in line with what is wanted.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    Install,
    Remove,
}

fn step(wanted: bool, held: bool) -> Option<Step> {
    match (wanted, held) {
        (true, false) => Some(Step::Install),
        (false, true) => Some(Step::Remove),
        _ => None,
    }
}

/// The thread and what it is asked for. The hook handles themselves stay on
/// the thread, in `run`, because only it may use them.
struct HookThread {
    /// The hooks the UI thread wants, as `Hook::bit`s.
    wanted: AtomicU8,
    /// The hooks the thread holds, as `Hook::bit`s. Only the thread writes
    /// it.
    held: AtomicU8,
    /// Whether the thread was spawned.
    started: AtomicBool,
    /// The thread's id once it has a message queue to post to, else 0.
    thread: AtomicU32,
}

impl HookThread {
    const fn new() -> Self {
        Self {
            wanted: AtomicU8::new(0),
            held: AtomicU8::new(0),
            started: AtomicBool::new(false),
            thread: AtomicU32::new(0),
        }
    }

    fn set(&'static self, hook: Hook, wanted: bool) {
        let before = if wanted {
            self.wanted.fetch_or(hook.bit(), Ordering::SeqCst)
        } else {
            self.wanted.fetch_and(!hook.bit(), Ordering::SeqCst)
        };
        // The app asks after every event, so only a change wakes the thread.
        if (before & hook.bit() != 0) != wanted {
            self.wake();
        }
    }

    fn holds(&self, hook: Hook) -> bool {
        self.held.load(Ordering::SeqCst) & hook.bit() != 0
    }

    /// Has the thread read `wanted` again. A thread that has not published
    /// its id yet needs no message: it reads `wanted` after publishing, and
    /// this ran after `wanted` changed, so it sees the change either way.
    fn wake(&'static self) {
        let thread = self.thread.load(Ordering::SeqCst);
        if thread == 0 {
            self.start();
            return;
        }
        // SAFETY: posting copies plain values into the thread's queue; a
        // thread that is gone makes it fail, nothing more.
        let posted = unsafe { PostThreadMessageW(thread, WM_NULL, WPARAM(0), LPARAM(0)) };
        if let Err(error) = posted {
            tracing::warn!(%error, "could not wake the hook thread");
        }
    }

    fn start(&'static self) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let spawned = thread::Builder::new()
            .name("hooks".into())
            .spawn(move || self.run());
        if let Err(error) = spawned {
            self.started.store(false, Ordering::SeqCst);
            tracing::warn!(%error, "the hook thread did not start; keys on spaces will not work");
        }
    }

    fn run(&self) {
        let mut message = MSG::default();
        // A thread gets its message queue at its first call for a message,
        // and a post to a thread without one fails, so the queue is made
        // before the id is published.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postthreadmessagew#remarks
        // SAFETY: `message` is a valid buffer, and nothing is removed.
        unsafe {
            let _ = PeekMessageW(&mut message, None, WM_USER, WM_USER, PM_NOREMOVE);
            self.thread.store(GetCurrentThreadId(), Ordering::SeqCst);
        }
        let mut handles = [None; Hook::ALL.len()];
        loop {
            self.sync(&mut handles);
            // SAFETY: `message` is a valid buffer. The system calls the hook
            // procedures from inside this wait.
            let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
            if status.0 <= 0 {
                break;
            }
        }
        // Nothing posts WM_QUIT here, so the wait itself failed. Windows
        // removes a thread's hooks when it ends; the next change to what is
        // wanted starts a new thread.
        tracing::error!("the hook thread's message wait failed; its hooks are gone");
        self.held.store(0, Ordering::SeqCst);
        self.thread.store(0, Ordering::SeqCst);
        self.started.store(false, Ordering::SeqCst);
    }

    /// Installs and removes hooks until the thread holds what is wanted. A
    /// hook that would not install is tried again at the next wake.
    fn sync(&self, handles: &mut [Option<HHOOK>; Hook::ALL.len()]) {
        let wanted = self.wanted.load(Ordering::SeqCst);
        let mut held = 0;
        for hook in Hook::ALL {
            let handle = &mut handles[hook as usize];
            match step(wanted & hook.bit() != 0, handle.is_some()) {
                Some(Step::Install) => *handle = install(hook),
                Some(Step::Remove) => remove(hook, handle.take()),
                None => {}
            }
            if handle.is_some() {
                held |= hook.bit();
            }
        }
        self.held.store(held, Ordering::SeqCst);
    }
}

fn install(hook: Hook) -> Option<HHOOK> {
    // SAFETY: the procedure is a function of this crate, so it lives for the
    // process, and a low-level hook is called on the installing thread, this
    // one, which waits for messages for as long as it holds the hook.
    let installed = unsafe {
        GetModuleHandleW(None).and_then(|module| {
            SetWindowsHookExW(hook.kind(), hook.procedure(), Some(HINSTANCE(module.0)), 0)
        })
    };
    // Success is not logged: a thread that holds hooks should not wait on a
    // log's writer when nothing went wrong.
    installed
        .inspect_err(|error| tracing::warn!(%error, ?hook, "could not install a low-level hook"))
        .ok()
}

fn remove(hook: Hook, handle: Option<HHOOK>) {
    let Some(handle) = handle else {
        return;
    };
    // SAFETY: `handle` came from SetWindowsHookExW on this thread and is
    // removed once: `sync` took it out of its slot.
    let removed = unsafe { UnhookWindowsHookEx(handle) };
    if let Err(error) = removed {
        // Windows had already dropped it, which a hook is never told.
        tracing::debug!(%error, ?hook, "a low-level hook was already gone");
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn a_hook_is_installed_or_removed_only_when_it_differs_from_what_is_wanted() {
        assert_eq!(step(true, false), Some(Step::Install));
        assert_eq!(step(false, true), Some(Step::Remove));
        assert_eq!(step(true, true), None);
        assert_eq!(step(false, false), None);
    }

    #[test]
    fn each_hook_has_its_own_bit() {
        assert_eq!(Hook::Keyboard.bit() & Hook::Mouse.bit(), 0);
        for (index, hook) in Hook::ALL.into_iter().enumerate() {
            assert_eq!(hook as usize, index, "`sync` indexes its handles by hook");
        }
    }

    #[test]
    fn asking_for_nothing_starts_no_thread() {
        static HOOKS: HookThread = HookThread::new();
        HOOKS.set(Hook::Keyboard, false);
        HOOKS.set(Hook::Mouse, false);
        assert!(!HOOKS.started.load(Ordering::SeqCst));
        assert!(!HOOKS.holds(Hook::Keyboard) && !HOOKS.holds(Hook::Mouse));
    }

    /// Waits up to 5 s for `condition`.
    fn eventually(condition: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        condition()
    }

    /// A real thread and a real mouse hook, held for a moment. Its procedure
    /// passes every event on, and posts presses nowhere without a controller.
    #[test]
    fn the_thread_installs_and_removes_a_hook_as_it_is_asked() {
        static HOOKS: HookThread = HookThread::new();
        HOOKS.set(Hook::Mouse, true);
        assert!(eventually(|| HOOKS.holds(Hook::Mouse)), "the first ask");
        assert!(!HOOKS.holds(Hook::Keyboard));
        HOOKS.set(Hook::Mouse, false);
        assert!(eventually(|| !HOOKS.holds(Hook::Mouse)), "the removal");
        // The thread is running now, so this ask reaches it as a message.
        HOOKS.set(Hook::Mouse, true);
        assert!(eventually(|| HOOKS.holds(Hook::Mouse)), "the second ask");
        HOOKS.set(Hook::Mouse, false);
        assert!(eventually(|| !HOOKS.holds(Hook::Mouse)));
    }
}
