use deskspace_core::{Desktop, Item, Rect, layout::arrange_space};
use deskspace_platform::enumerate_desktop_items;

const INITIAL_space: &str = "Desktop";
const INITIAL_RECT: Rect = Rect::new(60, 60, 440, 320);

pub fn seed_default_desktop(desktop: &mut Desktop, with_icons: bool) {
    let id = desktop.add_space(INITIAL_space, INITIAL_RECT);

    if with_icons {
        match enumerate_desktop_items() {
            Ok(entries) => {
                for entry in entries {
                    let item = Item::new(desktop.next_item_id(), entry.path)
                        .with_label(entry.label)
                        .with_kind(entry.kind);
                    desktop.add_item(id, item);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "could not enumerate desktop items");
            }
        }
    }

    let layout = desktop.layout;
    if let Some(space) = desktop.space_mut(id) {
        arrange_space(space, layout);
    }
}
