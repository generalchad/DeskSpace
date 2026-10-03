use deskspace_core::{GridSpec, ItemId, Palette, Rect, Space, Theme};
use deskspace_platform::{IconCache, ItemState, ItemVisual, SpaceScene, SpaceStyle};

const SCROLL_THUMB_WIDTH: i32 = 4;
const SCROLL_THUMB_MARGIN: i32 = 3;
const SCROLL_THUMB_MIN_HEIGHT: i32 = 24;

/// Transient view state that affects how one space is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewState {
    pub hovered: Option<ItemId>,
    pub selected: Option<ItemId>,
    /// Pixels the content is scrolled down; already clamped by the caller.
    pub scroll: i32,
}

pub fn space_style(theme: &Theme, system_dark: bool) -> SpaceStyle {
    SpaceStyle {
        backdrop: theme.backdrop,
        corners: theme.corners,
        dark: theme.is_dark(system_dark),
        border: theme.show_border,
    }
}

/// Builds the draw list for one space window. Item rects are relative to the
/// window, which starts at the space's top-left corner. Only items at least
/// partly inside the content area are included, so icons of scrolled-away
/// items are neither drawn nor loaded.
pub fn space_scene(
    space: &Space,
    theme: &Theme,
    palette: &Palette,
    layout: GridSpec,
    view: ViewState,
    icons: &mut IconCache,
) -> SpaceScene {
    let cell = layout.cell();
    let content_top = space.title_height;
    let content_bottom = space.rect.height;
    let items = if space.rolled_up {
        Vec::new()
    } else {
        space
            .items
            .iter()
            .map(|item| {
                let rect = Rect::new(
                    item.position.x,
                    content_top + item.position.y - view.scroll,
                    cell.width,
                    cell.height,
                );
                (item, rect)
            })
            .filter(|(_, rect)| rect.bottom() > content_top && rect.y < content_bottom)
            .map(|(item, rect)| ItemVisual {
                id: item.id.0,
                rect,
                label: item.label.clone(),
                icon: icons.icon_for(&item.path),
                state: if view.selected == Some(item.id) {
                    ItemState::Selected
                } else if view.hovered == Some(item.id) {
                    ItemState::Hovered
                } else {
                    ItemState::Normal
                },
            })
            .collect()
    };

    SpaceScene {
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
        scroll_thumb: scroll_thumb(space, layout, view.scroll),
    }
}

/// A thin bar along the right edge whose length and position show which part
/// of the content is visible. `None` when everything fits.
fn scroll_thumb(space: &Space, layout: GridSpec, scroll: i32) -> Option<Rect> {
    if space.rolled_up {
        return None;
    }
    let max_scroll = space.max_scroll(layout.cell(), layout.padding);
    if max_scroll == 0 {
        return None;
    }
    let viewport = space.content_rect().height;
    let extent = viewport + max_scroll;
    let track_top = space.title_height + SCROLL_THUMB_MARGIN;
    let track_height = viewport - SCROLL_THUMB_MARGIN * 2;
    if track_height <= 0 {
        return None;
    }
    let thumb_height = (track_height * viewport / extent)
        .max(SCROLL_THUMB_MIN_HEIGHT)
        .min(track_height);
    let travel = track_height - thumb_height;
    let offset = travel * scroll.clamp(0, max_scroll) / max_scroll;
    Some(Rect::new(
        space.rect.width - SCROLL_THUMB_MARGIN - SCROLL_THUMB_WIDTH,
        track_top + offset,
        SCROLL_THUMB_WIDTH,
        thumb_height,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::{Item, Point, SpaceId};

    fn space() -> Space {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(500, 300, 240, 200));
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
            GridSpec::default(),
            ViewState::default(),
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
            GridSpec::default(),
            ViewState::default(),
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
            GridSpec::default(),
            ViewState {
                hovered: Some(ItemId(4)),
                selected: Some(ItemId(4)),
                scroll: 0,
            },
            &mut IconCache::new(32),
        );
        assert_eq!(scene.items[0].state, ItemState::Selected);
        assert_eq!(scene.tint.a, palette.tint.fade(128).a);
    }

    fn tall_space() -> Space {
        let layout = GridSpec::default();
        let mut space = Space::new(SpaceId(1), "Tall", Rect::new(0, 0, 240, 200));
        space.user_sized = true;
        for index in 0..10u64 {
            let mut item = Item::new(ItemId(index + 1), format!("C:/missing/{index}.txt"));
            item.position = layout.grid_position_in(index as usize, 2);
            space.push_item(item);
        }
        space
    }

    #[test]
    fn only_items_inside_the_content_area_are_drawn() {
        let space = tall_space();
        let theme = Theme::default();
        let build = |scroll| {
            space_scene(
                &space,
                &theme,
                &theme.palette(true),
                GridSpec::default(),
                ViewState {
                    scroll,
                    ..ViewState::default()
                },
                &mut IconCache::new(32),
            )
        };
        let top = build(0);
        // Content is 172 px tall: row 0 (12..116) and row 1 (126..230).
        assert_eq!(
            top.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        // Scrolled exactly past row 0, whose bottom now meets the title bar.
        let scrolled = build(116);
        assert_eq!(scrolled.items[0].id, 3);
        assert_eq!(scrolled.items[0].rect.y, space.title_height + 10);
    }

    #[test]
    fn scroll_thumb_tracks_position_and_hides_when_content_fits() {
        let theme = Theme::default();
        let layout = GridSpec::default();
        let tall = tall_space();
        let max = tall.max_scroll(layout.cell(), layout.padding);
        let thumb = |scroll| {
            space_scene(
                &tall,
                &theme,
                &theme.palette(true),
                layout,
                ViewState {
                    scroll,
                    ..ViewState::default()
                },
                &mut IconCache::new(32),
            )
            .scroll_thumb
            .unwrap()
        };
        let (start, end) = (thumb(0), thumb(max));
        assert_eq!(start.y, tall.title_height + SCROLL_THUMB_MARGIN);
        assert_eq!(end.bottom(), tall.rect.height - SCROLL_THUMB_MARGIN);
        assert_eq!(start.right(), tall.rect.width - SCROLL_THUMB_MARGIN);

        let short = space();
        let scene = space_scene(
            &short,
            &theme,
            &theme.palette(true),
            layout,
            ViewState::default(),
            &mut IconCache::new(32),
        );
        assert_eq!(scene.scroll_thumb, None);
    }
}
