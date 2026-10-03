use deskspace_core::{space, ItemId, Palette, Rect, Size, Theme};
use deskspace_platform::{spaceScene, spaceStyle, IconCache, ItemState, ItemVisual};

/// Pointer state that affects how one space is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Highlight {
    pub hovered: Option<ItemId>,
    pub selected: Option<ItemId>,
}

pub fn space_style(theme: &Theme, system_dark: bool) -> spaceStyle {
    spaceStyle {
        backdrop: theme.backdrop,
        corners: theme.corners,
        dark: theme.is_dark(system_dark),
        border: theme.show_border,
    }
}

/// Builds the draw list for one space window. Item rects are relative to the
/// window, which starts at the space's top-left corner.
pub fn space_scene(
    space: &space,
    theme: &Theme,
    palette: &Palette,
    cell: Size,
    highlight: Highlight,
    icons: &mut IconCache,
) -> spaceScene {
    let items = if space.rolled_up {
        Vec::new()
    } else {
        space
            .items
            .iter()
            .map(|item| ItemVisual {
                id: item.id.0,
                rect: Rect::new(
                    item.position.x,
                    space.title_height + item.position.y,
                    cell.width,
                    cell.height,
                ),
                label: item.label.clone(),
                icon: icons.icon_for(&item.path),
                state: if highlight.selected == Some(item.id) {
                    ItemState::Selected
                } else if highlight.hovered == Some(item.id) {
                    ItemState::Hovered
                } else {
                    ItemState::Normal
                },
            })
            .collect()
    };

    spaceScene {
        title: space.title.clone(),
        title_height: space.title_height,
        tint: palette.tint.fade(space.opacity),
        title_fill: palette.title_fill,
        title_text: palette.title_text,
        item_text: palette.item_text,
        hover_fill: palette.hover_fill,
        selected_fill: palette.selected_fill,
        border: palette.border,
        corner_radius: theme.corners.radius(),
        locked: space.locked,
        rolled_up: space.rolled_up,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::{spaceId, Item, Point};

    fn space() -> space {
        let mut space = space::new(spaceId(1), "Work", Rect::new(500, 300, 240, 200));
        let mut item = Item::new(ItemId(4), "C:/missing/report.txt");
        item.position = Point::new(12, 12);
        space.push_item(item);
        space
    }

    #[test]
    fn items_are_placed_below_the_title_in_window_coordinates() {
        let space = space();
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            &theme,
            &theme.palette(true),
            Size::new(96, 104),
            Highlight::default(),
            &mut IconCache::new(32),
        );
        assert_eq!(
            scene.items[0].rect,
            Rect::new(12, space.title_height + 12, 96, 104)
        );
    }

    #[test]
    fn rolled_up_spaces_draw_no_items() {
        let mut space = space();
        space.rolled_up = true;
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            &theme,
            &theme.palette(true),
            Size::new(96, 104),
            Highlight::default(),
            &mut IconCache::new(32),
        );
        assert!(scene.items.is_empty());
        assert!(scene.rolled_up);
    }

    #[test]
    fn selection_wins_over_hover_and_opacity_scales_tint() {
        let mut space = space();
        space.opacity = 128;
        let theme = Theme::default();
        let palette = theme.palette(true);
        let scene = space_scene(
            &space,
            &theme,
            &palette,
            Size::new(96, 104),
            Highlight {
                hovered: Some(ItemId(4)),
                selected: Some(ItemId(4)),
            },
            &mut IconCache::new(32),
        );
        assert_eq!(scene.items[0].state, ItemState::Selected);
        assert_eq!(scene.tint.a, palette.tint.fade(128).a);
    }
}
