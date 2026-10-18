//! The debug window's control ids.

/// A debug control, passed to the platform as the variant's value
/// (`DebugControl::id`). Never renumber one; a new control takes the highest
/// value plus one, joins `ALL`, and gets a `DEBUG_CONTROLS` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum DebugId {
    SpacesVisible = 1,
    DeleteAllSpaces = 2,
}

impl DebugId {
    pub(crate) const ALL: [Self; 2] = [Self::SpacesVisible, Self::DeleteAllSpaces];

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
