//! The icon cache's load worker: a thread that reads icons from the shell and
//! reports them to the UI thread in batches. With no icon waiting it reads
//! the items' own files, for their facts and to find the ones that are gone:
//! the other file reads that must stay off the UI thread.

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::fact_jobs::FactJobs;
use crate::icon_queue::IconQueue;
use crate::render::Icon;
use crate::win32::desktop_items::file_state;
use crate::win32::messages;

use super::load::{load_folder_icon, load_icon};
use super::{IconKind, Key};

/// How often a busy worker reports what it loaded, so a page of icons fills
/// in a few steps rather than one repaint per icon.
const LOADED_REPORT_INTERVAL: Duration = Duration::from_millis(33);

/// What the UI thread and the load worker share.
pub(super) struct Worker {
    pub(super) state: Mutex<WorkerState>,
    pub(super) ready: Condvar,
}

pub(super) struct WorkerState {
    pub(super) queue: IconQueue<Key, super::IconUser>,
    pub(super) loaded: Vec<(Key, Option<Icon>)>,
    /// A report is on its way to the UI thread, which takes everything loaded
    /// by then, so another would be wasted.
    pub(super) reported: bool,
    /// Items' files to read while no icon is waiting.
    pub(super) facts: FactJobs,
    pub(super) stop: bool,
}

/// What the worker does next.
#[derive(Debug, PartialEq, Eq)]
enum Request {
    Icon(Key),
    /// Read the file at this path, for `WorkerState::facts`.
    Facts(PathBuf),
}

impl Worker {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(WorkerState {
                queue: IconQueue::new(),
                loaded: Vec::new(),
                reported: false,
                facts: FactJobs::default(),
                stop: false,
            }),
            ready: Condvar::new(),
        }
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, WorkerState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub(super) fn start_worker(target: HWND) -> Option<Arc<Worker>> {
    let worker = Arc::new(Worker::new());
    let shared = worker.clone();
    let target = target.0 as isize;
    let spawned = std::thread::Builder::new()
        .name("deskspace-icons".into())
        .spawn(move || run_worker(&shared, target));
    match spawned {
        Ok(_) => Some(worker),
        Err(error) => {
            tracing::warn!(%error, "could not start loading icons");
            None
        }
    }
}

fn run_worker(worker: &Worker, target: isize) {
    // SAFETY: this thread enters its own single-threaded apartment, as the
    // image factory needs. It is never left: the thread-local `Graphics` holds
    // COM objects and is dropped after this function returns, when releasing
    // them still needs COM.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let mut last_report = Instant::now();
    let mut served = false;
    let mut idle_logged = false;
    while let Some(request) = next_request(
        worker,
        target,
        &mut last_report,
        &mut served,
        &mut idle_logged,
    ) {
        match request {
            Request::Icon(key) => {
                let icon = match key.2 {
                    IconKind::Item { overlay_size } => load_icon(&key.0, key.1, overlay_size),
                    IconKind::Folder { available } => {
                        load_folder_icon(&key.0, key.1.clamp(1, 1024) as u32, available)
                    }
                };
                let mut state = worker.lock();
                state.loaded.push((key, icon));
                served = true;
                if should_report(&state, last_report.elapsed()) {
                    report_loaded(&mut state, target, &mut last_report);
                }
            }
            Request::Facts(path) => {
                let state = file_state(&path);
                if worker.lock().facts.record(state) {
                    report_facts(target);
                }
            }
        }
    }
}

/// Blocks until the UI thread asks for something, reporting what is left to
/// report first. `None` means stop.
fn next_request(
    worker: &Worker,
    target: isize,
    last_report: &mut Instant,
    served: &mut bool,
    idle_logged: &mut bool,
) -> Option<Request> {
    let mut state = worker.lock();
    loop {
        if state.stop {
            return None;
        }
        // Icons already loaded reach the UI thread before any fact is read.
        if state.queue.is_empty() && !state.loaded.is_empty() && !state.reported {
            report_loaded(&mut state, target, last_report);
        }
        if let Some(request) = take_request(&mut state) {
            return Some(request);
        }
        // Idle once it has served something and the UI thread has taken
        // everything loaded, which is the end of the startup batch.
        if *served && !*idle_logged && state.loaded.is_empty() {
            *idle_logged = true;
            tracing::debug!(
                elapsed_ms = crate::startup_millis(),
                "startup: icon worker idle"
            );
        }
        state = worker
            .ready
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// The work to do now. An icon goes ahead of every fact, and facts are handed
/// out a file at a time, so an icon a scroll asks for waits for one read at
/// most.
fn take_request(state: &mut WorkerState) -> Option<Request> {
    if let Some(key) = state.queue.pop() {
        return Some(Request::Icon(key));
    }
    state.facts.next().map(Request::Facts)
}

/// Whether to tell the UI thread now: once the queue has run dry, or when it
/// has waited long enough for a batch.
fn should_report(state: &WorkerState, since_last: Duration) -> bool {
    !state.reported && (state.queue.is_empty() || since_last >= LOADED_REPORT_INTERVAL)
}

fn report_loaded(state: &mut WorkerState, target: isize, last_report: &mut Instant) {
    // SAFETY: posting takes no pointers; a window that has gone just fails.
    let posted = unsafe {
        PostMessageW(
            Some(HWND(target as *mut _)),
            messages::ICONS_LOADED,
            WPARAM(0),
            LPARAM(0),
        )
    };
    state.reported = posted.is_ok();
    *last_report = Instant::now();
}

/// Tells the UI thread a batch of item facts is ready. Each batch posts its
/// own message: there is one per space, and only when the app asks.
fn report_facts(target: isize) {
    // SAFETY: posting takes no pointers; a window that has gone just fails.
    let _ = unsafe {
        PostMessageW(
            Some(HWND(target as *mut _)),
            messages::ITEM_FACTS_READ,
            WPARAM(0),
            LPARAM(0),
        )
    };
}

#[cfg(test)]
mod tests {
    use super::super::IconUser;
    use super::*;

    const SPACE: IconUser = IconUser::Space(1);

    fn key(path: &str) -> Key {
        (PathBuf::from(path), 32, IconKind::Item { overlay_size: 80 })
    }

    #[test]
    fn a_report_waits_for_a_batch_unless_the_queue_is_dry() {
        let worker = Worker::new();
        let mut state = worker.lock();
        let soon = Duration::ZERO;
        assert!(should_report(&state, soon), "nothing else is coming");
        state.queue.set_wanted(&SPACE, &[key("next")]);
        assert!(!should_report(&state, soon));
        assert!(should_report(&state, LOADED_REPORT_INTERVAL));
        state.reported = true;
        assert!(!should_report(&state, LOADED_REPORT_INTERVAL));
    }

    #[test]
    fn an_icon_goes_ahead_of_every_fact() {
        use crate::fact_jobs::FileState;
        use deskspace_core::{ItemId, SpaceId};

        let worker = Worker::new();
        let mut state = worker.lock();
        let fact = |name: &str| Some(Request::Facts(PathBuf::from(name)));
        state.facts.push(
            SpaceId(1),
            vec![
                (ItemId(1), PathBuf::from("a")),
                (ItemId(2), PathBuf::from("b")),
            ],
        );
        assert_eq!(take_request(&mut state), fact("a"));
        state.facts.record(FileState::Unreachable);
        // A scene asks for an icon while the batch is half read.
        state.queue.set_wanted(&SPACE, &[key("icon")]);
        assert_eq!(take_request(&mut state), Some(Request::Icon(key("icon"))));
        assert_eq!(take_request(&mut state), fact("b"));
        state.facts.record(FileState::Unreachable);
        assert_eq!(take_request(&mut state), None);
    }
}
