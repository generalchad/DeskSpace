//! The debug window's sections and the one row per control that places it.

use super::ids::DebugId;

/// The debug window's sections, in the order its section list shows them.
/// A section with no rows is left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DebugSectionId {
    Spaces,
    Config,
    Diagnostics,
    Windows,
    System,
    Animations,
    Takeover,
    Simulate,
}

impl DebugSectionId {
    pub(crate) const ALL: [Self; 8] = [
        Self::Spaces,
        Self::Config,
        Self::Diagnostics,
        Self::Windows,
        Self::System,
        Self::Animations,
        Self::Takeover,
        Self::Simulate,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Spaces => "Spaces",
            Self::Config => "Config",
            Self::Diagnostics => "Diagnostics",
            Self::Windows => "Windows",
            Self::System => "System",
            Self::Animations => "Animations",
            Self::Takeover => "Takeover",
            Self::Simulate => "Simulate",
        }
    }
}

/// Where a control goes and what it is called. Its kind and value come
/// from `App::debug_control`.
pub(crate) struct DebugRow {
    pub id: DebugId,
    pub section: DebugSectionId,
    pub label: &'static str,
}

/// Every debug control, in the order each section shows them.
pub(crate) const DEBUG_CONTROLS: &[DebugRow] = &[
    DebugRow {
        id: DebugId::SpaceTable,
        section: DebugSectionId::Spaces,
        label: "Spaces",
    },
    DebugRow {
        id: DebugId::SpacesVisible,
        section: DebugSectionId::Windows,
        label: "Show spaces",
    },
    DebugRow {
        id: DebugId::OnTop,
        section: DebugSectionId::Windows,
        label: "Keep spaces above other windows",
    },
    DebugRow {
        id: DebugId::WindowTable,
        section: DebugSectionId::Windows,
        label: "Windows",
    },
    DebugRow {
        id: DebugId::MonitorTable,
        section: DebugSectionId::Windows,
        label: "Monitors",
    },
    DebugRow {
        id: DebugId::CreateTestSpaces,
        section: DebugSectionId::Spaces,
        label: "Create test spaces",
    },
    DebugRow {
        id: DebugId::StressSize,
        section: DebugSectionId::Spaces,
        label: "Stress space size",
    },
    DebugRow {
        id: DebugId::CreateStress,
        section: DebugSectionId::Spaces,
        label: "Create a stress space",
    },
    DebugRow {
        id: DebugId::UnfoldAll,
        section: DebugSectionId::Spaces,
        label: "Unfold every space",
    },
    DebugRow {
        id: DebugId::UnlockAll,
        section: DebugSectionId::Spaces,
        label: "Unlock every space",
    },
    DebugRow {
        id: DebugId::RefreshAll,
        section: DebugSectionId::Spaces,
        label: "Refresh every space",
    },
    DebugRow {
        id: DebugId::ReflowAll,
        section: DebugSectionId::Spaces,
        label: "Re-flow every space",
    },
    DebugRow {
        id: DebugId::ClearIconCache,
        section: DebugSectionId::Spaces,
        label: "Clear the icon cache",
    },
    DebugRow {
        id: DebugId::SaveNow,
        section: DebugSectionId::Spaces,
        label: "Save now",
    },
    DebugRow {
        id: DebugId::PruneNow,
        section: DebugSectionId::Spaces,
        label: "Prune missing items",
    },
    DebugRow {
        id: DebugId::DeleteAllSpaces,
        section: DebugSectionId::Spaces,
        label: "Delete all spaces",
    },
    DebugRow {
        id: DebugId::ConfigPath,
        section: DebugSectionId::Config,
        label: "Configuration",
    },
    DebugRow {
        id: DebugId::StoragePath,
        section: DebugSectionId::Config,
        label: "Storage",
    },
    DebugRow {
        id: DebugId::DumpConfig,
        section: DebugSectionId::Config,
        label: "Write the configuration to the log",
    },
    DebugRow {
        id: DebugId::OpenConfigFolder,
        section: DebugSectionId::Config,
        label: "Open the configuration folder",
    },
    DebugRow {
        id: DebugId::OpenStorageFolder,
        section: DebugSectionId::Config,
        label: "Open the storage folder",
    },
    DebugRow {
        id: DebugId::Build,
        section: DebugSectionId::Diagnostics,
        label: "Build",
    },
    DebugRow {
        id: DebugId::Model,
        section: DebugSectionId::Diagnostics,
        label: "Model",
    },
    DebugRow {
        id: DebugId::Resources,
        section: DebugSectionId::Diagnostics,
        label: "Resources",
    },
    DebugRow {
        id: DebugId::Updates,
        section: DebugSectionId::Diagnostics,
        label: "Updates",
    },
    DebugRow {
        id: DebugId::CopyDiagnostics,
        section: DebugSectionId::Diagnostics,
        label: "Copy diagnostics",
    },
    DebugRow {
        id: DebugId::Theme,
        section: DebugSectionId::System,
        label: "Theme",
    },
    DebugRow {
        id: DebugId::Explorer,
        section: DebugSectionId::System,
        label: "Explorer",
    },
    DebugRow {
        id: DebugId::Hotkeys,
        section: DebugSectionId::System,
        label: "Hotkeys",
    },
    DebugRow {
        id: DebugId::Hooks,
        section: DebugSectionId::System,
        label: "Hooks",
    },
    DebugRow {
        id: DebugId::Speed,
        section: DebugSectionId::Animations,
        label: "Speed",
    },
    DebugRow {
        id: DebugId::IgnoreWindows,
        section: DebugSectionId::Animations,
        label: "Animate even when Windows' Animation effects is off",
    },
    DebugRow {
        id: DebugId::PreviewSpace,
        section: DebugSectionId::Animations,
        label: "Space",
    },
    DebugRow {
        id: DebugId::PreviewKind,
        section: DebugSectionId::Animations,
        label: "Kind",
    },
    DebugRow {
        id: DebugId::PreviewPlay,
        section: DebugSectionId::Animations,
        label: "Play on the space",
    },
    DebugRow {
        id: DebugId::Takeover,
        section: DebugSectionId::Takeover,
        label: "Take over the desktop (this session)",
    },
    DebugRow {
        id: DebugId::ResyncLayer,
        section: DebugSectionId::Takeover,
        label: "Re-sync the desktop layer",
    },
    DebugRow {
        id: DebugId::TakeoverState,
        section: DebugSectionId::Takeover,
        label: "State",
    },
    DebugRow {
        id: DebugId::ThemeChanged,
        section: DebugSectionId::Simulate,
        label: "Windows theme changed",
    },
    DebugRow {
        id: DebugId::AnimationsChanged,
        section: DebugSectionId::Simulate,
        label: "Windows Animation effects changed",
    },
    DebugRow {
        id: DebugId::ExplorerRestarted,
        section: DebugSectionId::Simulate,
        label: "Explorer restarted",
    },
    DebugRow {
        id: DebugId::ReadOnlyConfig,
        section: DebugSectionId::Simulate,
        label: "Make the configuration file read-only",
    },
    DebugRow {
        id: DebugId::CrashNow,
        section: DebugSectionId::Simulate,
        label: "Crash now",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_has_one_row() {
        for control in DebugId::ALL {
            let rows = DEBUG_CONTROLS
                .iter()
                .filter(|row| row.id == control)
                .count();
            assert_eq!(rows, 1, "{control:?} needs exactly one DEBUG_CONTROLS row");
        }
    }
}
