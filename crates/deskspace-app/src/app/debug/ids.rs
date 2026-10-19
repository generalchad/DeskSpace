//! The debug window's control ids.

/// A debug control, passed to the platform as the variant's value
/// (`DebugControl::id`). Never renumber one; a new control takes the highest
/// value plus one, joins `ALL`, and gets a `DEBUG_CONTROLS` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum DebugId {
    SpacesVisible = 1,
    DeleteAllSpaces = 2,
    CreateTestSpaces = 3,
    UnfoldAll = 4,
    UnlockAll = 5,
    RefreshAll = 6,
    PruneNow = 7,
    StressSize = 8,
    CreateStress = 9,
    ConfigPath = 10,
    StoragePath = 11,
    DumpConfig = 12,
    OpenConfigFolder = 13,
    OpenStorageFolder = 14,
    Build = 15,
    Model = 16,
    Resources = 17,
    Updates = 18,
    CopyDiagnostics = 19,
    OnTop = 20,
    WindowTable = 21,
    MonitorTable = 22,
    Theme = 23,
    Explorer = 24,
    Hotkeys = 25,
    Hooks = 26,
    Speed = 27,
    IgnoreWindows = 28,
    PreviewSpace = 29,
    PreviewKind = 30,
    PreviewPlay = 31,
    Takeover = 32,
    ResyncLayer = 33,
    TakeoverState = 34,
    ThemeChanged = 35,
    ExplorerRestarted = 36,
    ReadOnlyConfig = 37,
    CrashNow = 38,
    SpaceTable = 39,
    ReflowAll = 40,
    SaveNow = 41,
    ClearIconCache = 42,
    AnimationsChanged = 43,
    MarkLog = 44,
    LogEvents = 45,
    InputState = 46,
    Taskbars = 47,
    PreviewPlayAll = 48,
    DeleteStress = 49,
    OutlineShadows = 50,
    HideShadows = 51,
}

impl DebugId {
    pub(crate) const ALL: [Self; 51] = [
        Self::SpacesVisible,
        Self::DeleteAllSpaces,
        Self::CreateTestSpaces,
        Self::UnfoldAll,
        Self::UnlockAll,
        Self::RefreshAll,
        Self::PruneNow,
        Self::StressSize,
        Self::CreateStress,
        Self::ConfigPath,
        Self::StoragePath,
        Self::DumpConfig,
        Self::OpenConfigFolder,
        Self::OpenStorageFolder,
        Self::Build,
        Self::Model,
        Self::Resources,
        Self::Updates,
        Self::CopyDiagnostics,
        Self::OnTop,
        Self::WindowTable,
        Self::MonitorTable,
        Self::Theme,
        Self::Explorer,
        Self::Hotkeys,
        Self::Hooks,
        Self::Speed,
        Self::IgnoreWindows,
        Self::PreviewSpace,
        Self::PreviewKind,
        Self::PreviewPlay,
        Self::Takeover,
        Self::ResyncLayer,
        Self::TakeoverState,
        Self::ThemeChanged,
        Self::ExplorerRestarted,
        Self::ReadOnlyConfig,
        Self::CrashNow,
        Self::SpaceTable,
        Self::ReflowAll,
        Self::SaveNow,
        Self::ClearIconCache,
        Self::AnimationsChanged,
        Self::MarkLog,
        Self::LogEvents,
        Self::InputState,
        Self::Taskbars,
        Self::PreviewPlayAll,
        Self::DeleteStress,
        Self::OutlineShadows,
        Self::HideShadows,
    ];

    pub(crate) fn id(self) -> u32 {
        self as u32
    }

    pub(crate) fn from_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|control| control.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_ids_round_trip() {
        for control in DebugId::ALL {
            assert_eq!(DebugId::from_id(control.id()), Some(control));
        }
        assert_eq!(DebugId::from_id(0), None);
    }

    #[test]
    fn debug_ids_are_unique() {
        let mut ids: Vec<u32> = DebugId::ALL.iter().map(|control| control.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), DebugId::ALL.len());
    }
}
