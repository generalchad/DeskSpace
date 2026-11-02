//! What a `Command` will change, so the app knows what to repaint (B28).

use super::Command;
use crate::desktop::Desktop;
use crate::id::{ItemId, SpaceId};

/// The parts of the model a command changes. `spaces` lists the destination
/// first when a command moves items between spaces, because its grid lays out
/// the result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Touched {
    pub spaces: Vec<SpaceId>,
    /// The loose desktop layer.
    pub layer: bool,
}

impl Touched {
    fn spaces(spaces: impl IntoIterator<Item = SpaceId>) -> Self {
        let mut touched = Self::default();
        for id in spaces {
            if !touched.spaces.contains(&id) {
                touched.spaces.push(id);
            }
        }
        touched
    }

    fn layer() -> Self {
        Self {
            spaces: Vec::new(),
            layer: true,
        }
    }
}

impl Desktop {
    /// What `command` will change. Call it before applying the command, while
    /// an item still names the space that holds it. A space or item that does
    /// not exist touches nothing.
    pub fn touched_by(&self, command: &Command) -> Touched {
        let holder = |item: ItemId| self.space_containing(item);
        // No `_` arm: a new variant does not compile until someone decides
        // what it touches.
        match command {
            // `apply_create_space` always takes the next id.
            Command::CreateSpace { .. } | Command::CreateLens { .. } => {
                Touched::spaces([self.next_space_id()])
            }
            Command::DeleteSpace { id }
            | Command::RenameSpace { id, .. }
            | Command::SetSpaceView { id, .. }
            | Command::SetSpaceIcon { id, .. }
            | Command::MoveSpace { id, .. }
            | Command::SetSpaceRect { id, .. }
            | Command::ResizeSpace { id, .. }
            | Command::FitToContents { id, .. }
            | Command::KeepSize { id }
            | Command::Reflow { id }
            | Command::SortSpace { id, .. }
            | Command::CustomizeSpace { id, .. }
            | Command::SetSpaceLook { id, .. }
            | Command::ToggleLock { id }
            | Command::ToggleFold { id }
            | Command::SetLensFolder { id, .. }
            | Command::SetLensTitle { id, .. }
            | Command::SetLensView { id, .. }
            | Command::SyncLens { id, .. }
            | Command::AutoArrange { id } => Touched::spaces([*id]),
            Command::RestoreSpaces { spaces } => Touched::spaces(spaces.iter().map(|(id, _)| *id)),
            Command::AddItem { space, .. } => Touched::spaces([*space]),
            Command::RemoveItem { id }
            | Command::ReorderItem { id, .. }
            | Command::RenameItem { id, .. } => Touched::spaces(holder(*id)),
            // `apply_reorder_items` works in the first item's space.
            Command::ReorderItems { ids, .. } => {
                Touched::spaces(ids.first().and_then(|id| holder(*id)))
            }
            Command::MoveItem { id, target } => Touched::spaces(
                holder(*id)
                    .map(|source| [*target, source])
                    .into_iter()
                    .flatten(),
            ),
            // Snapping pushes overlapping spaces apart, so any space can move.
            Command::SnapAll => Touched::spaces(self.spaces.iter().map(|space| space.id)),
            Command::SyncDesktop { .. } | Command::SetDesktopItemPosition { .. } => {
                Touched::layer()
            }
            Command::MoveToDesktop { item } => Touched {
                spaces: holder(*item).into_iter().collect(),
                layer: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Point, Rect, Size};
    use crate::item::{Item, ItemFacts};

    fn bounds() -> Rect {
        Rect::new(0, 0, 1920, 1080)
    }

    fn desktop_with_item() -> (Desktop, SpaceId, SpaceId, ItemId) {
        let mut desktop = Desktop::new();
        let a = desktop.add_space("A", Rect::new(0, 0, 300, 300));
        let b = desktop.add_space("B", Rect::new(400, 0, 300, 300));
        let item = desktop.next_item_id();
        let added = desktop.apply(
            Command::AddItem {
                space: a,
                path: "C:/missing/a.txt".into(),
                facts: ItemFacts::default(),
            },
            bounds(),
        );
        assert!(added.is_applied());
        (desktop, a, b, item)
    }

    #[test]
    fn a_space_command_touches_its_space() {
        let (desktop, a, _, _) = desktop_with_item();
        let touched = desktop.touched_by(&Command::AutoArrange { id: a });
        assert_eq!(touched, Touched::spaces([a]));
        assert!(!touched.layer);
    }

    #[test]
    fn a_lens_command_touches_its_space() {
        let (desktop, a, _, _) = desktop_with_item();
        let touched = desktop.touched_by(&Command::SyncLens {
            id: a,
            items: Vec::new(),
        });
        assert_eq!(touched, Touched::spaces([a]));
    }

    #[test]
    fn creating_a_space_touches_the_id_it_will_get() {
        let mut desktop = Desktop::new();
        let create = Command::CreateSpace {
            title: "New".into(),
            at: Point::ZERO,
            size: Size::new(200, 200),
            locked: false,
            icon: crate::icon::SpaceIcon::None,
            view: crate::view::SpaceView::default(),
        };
        let touched = desktop.touched_by(&create);
        assert!(desktop.apply(create, bounds()).is_applied());
        assert_eq!(touched.spaces, [desktop.spaces()[0].id]);
    }

    #[test]
    fn an_item_command_touches_the_space_that_holds_it() {
        let (desktop, a, _, item) = desktop_with_item();
        let touched = desktop.touched_by(&Command::RemoveItem { id: item });
        assert_eq!(touched, Touched::spaces([a]));
        let touched = desktop.touched_by(&Command::ReorderItems {
            ids: vec![item],
            index: 0,
        });
        assert_eq!(touched, Touched::spaces([a]));
    }

    #[test]
    fn a_missing_item_touches_nothing() {
        let (desktop, ..) = desktop_with_item();
        let touched = desktop.touched_by(&Command::RemoveItem { id: ItemId(99) });
        assert_eq!(touched, Touched::default());
    }

    #[test]
    fn a_move_touches_the_destination_first_then_the_source() {
        let (desktop, a, b, item) = desktop_with_item();
        let touched = desktop.touched_by(&Command::MoveItem {
            id: item,
            target: b,
        });
        assert_eq!(touched.spaces, [b, a]);
    }

    #[test]
    fn a_restore_touches_every_listed_space() {
        let (desktop, a, b, _) = desktop_with_item();
        let restore = Command::RestoreSpaces {
            spaces: vec![(b, None), (a, None)],
        };
        assert_eq!(desktop.touched_by(&restore), Touched::spaces([b, a]));
    }

    #[test]
    fn snapping_touches_every_space() {
        let (desktop, a, b, _) = desktop_with_item();
        assert_eq!(desktop.touched_by(&Command::SnapAll).spaces, [a, b]);
    }

    #[test]
    fn layer_commands_touch_only_the_layer() {
        let (desktop, _, _, item) = desktop_with_item();
        let sync = Command::SyncDesktop {
            items: vec![Item::new(item, "C:/missing/b.txt", ItemFacts::default())],
        };
        assert_eq!(desktop.touched_by(&sync), Touched::layer());
        let place = Command::SetDesktopItemPosition {
            item,
            position: Point::ZERO,
        };
        assert_eq!(desktop.touched_by(&place), Touched::layer());
    }

    #[test]
    fn moving_to_the_desktop_touches_the_source_and_the_layer() {
        let (desktop, a, _, item) = desktop_with_item();
        let touched = desktop.touched_by(&Command::MoveToDesktop { item });
        assert_eq!(touched.spaces, [a]);
        assert!(touched.layer);
    }
}
