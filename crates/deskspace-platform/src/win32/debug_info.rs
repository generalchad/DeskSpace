//! Facts the debug window shows about this process and Windows (ROADMAP D14).

use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetGuiResources,
};

/// What this process holds right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessStats {
    /// Physical memory in use, in bytes.
    pub working_set: u64,
    pub gdi_objects: u32,
    pub user_objects: u32,
}

/// This process's working set and GDI and USER object counts. A count that
/// cannot be read is 0.
/// https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getguiresources
pub fn process_stats() -> ProcessStats {
    // SAFETY: the pseudo handle for this process needs no closing, and the
    // counters struct is sized for the call.
    unsafe {
        let process = GetCurrentProcess();
        let mut memory = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        let working_set = match GetProcessMemoryInfo(process, &mut memory, memory.cb) {
            Ok(()) => memory.WorkingSetSize as u64,
            Err(_) => 0,
        };
        ProcessStats {
            working_set,
            gdi_objects: GetGuiResources(process, GR_GDIOBJECTS),
            user_objects: GetGuiResources(process, GR_USEROBJECTS),
        }
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
    };
    use windows::core::w;

    use super::*;

    #[test]
    fn a_live_process_reports_memory_and_a_user_object() {
        // SAFETY: a message-only window of the system "Static" class, made
        // and destroyed on this thread.
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("Static"),
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .expect("a message-only window");
        let stats = process_stats();
        // SAFETY: the window was created above on this thread.
        let _ = unsafe { DestroyWindow(window) };
        assert!(stats.working_set > 0);
        assert!(stats.user_objects >= 1);
    }
}
