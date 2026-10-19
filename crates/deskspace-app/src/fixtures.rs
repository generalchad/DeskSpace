//! Test fixtures that build the model the way the app does, through `Command`s.

use deskspace_core::{Command, Desktop, ItemFacts, ItemId, Rect, SpaceId};

/// A work area that holds every fixture rect, so `CreateSpace` keeps it as is.
pub const BOUNDS: Rect = Rect::new(0, 0, 1920, 1040);

/// Applies `command`, failing the test if it did not apply.
pub fn apply(desktop: &mut Desktop, command: Command) {
    let outcome = desktop.apply(command.clone(), BOUNDS);
    assert!(outcome.is_applied(), "{command:?} was {outcome:?}");
}

/// Creates an unlocked space at `rect` and returns its id.
pub fn create_space(desktop: &mut Desktop, title: &str, rect: Rect) -> SpaceId {
    let id = desktop.next_space_id();
    apply(
        desktop,
        Command::CreateSpace {
            title: title.to_owned(),
            at: rect.origin(),
            size: rect.size(),
            locked: false,
            icon: deskspace_core::SpaceIcon::None,
        },
    );
    id
}

/// Lists `path` in `space`, laid out on the grid, and returns the item's id.
pub fn add_item(desktop: &mut Desktop, space: SpaceId, path: &str) -> ItemId {
    let id = desktop.next_item_id();
    apply(
        desktop,
        Command::AddItem {
            space,
            path: path.into(),
            facts: ItemFacts::default(),
        },
    );
    id
}
