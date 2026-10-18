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
        id: DebugId::SpacesVisible,
        section: DebugSectionId::Windows,
        label: "Show spaces",
    },
    DebugRow {
        id: DebugId::CreateTestSpaces,
        section: DebugSectionId::Spaces,
        label: "Create test spaces",
    },
    DebugRow {
        id: DebugId::DeleteAllSpaces,
        section: DebugSectionId::Spaces,
        label: "Delete all spaces",
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
