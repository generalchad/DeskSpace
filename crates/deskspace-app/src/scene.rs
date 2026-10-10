use deskspace_core::item::ItemKind;
use deskspace_core::{Color, GridSpec, Item, ItemId, LabelShadow, Palette, Rect, Space, Theme};
use deskspace_platform::{IconCache, ItemMetrics, ItemState, ItemVisual, SpaceScene, SpaceStyle};

const SCROLL_THUMB_WIDTH: i32 = 4;
const SCROLL_THUMB_MARGIN: i32 = 3;
const SCROLL_THUMB_MIN_HEIGHT: i32 = 24;
/// Below this tint alpha the backdrop shows through so much that labels need
/// a shadow to stay readable on busy wallpapers.
const TEXT_SHADOW_BELOW_ALPHA: u8 = 64;

/// Transient view state that affects how one space is drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewState {
    pub hovered: Option<ItemId>,
    /// The selected items, which may be several (Ctrl/Shift click).
    pub selected: Vec<ItemId>,
    /// Being dragged within the space; their slots are drawn faded, ahead of
    /// the selection highlight. A group drag fades every member.
    pub dragged: Vec<ItemId>,
    /// Pixels the content is scrolled down; already clamped by the caller.
    pub scroll: i32,
    /// Draw a check box on each item, from Explorer's view options.
    pub show_check_boxes: bool,
    /// Show check boxes always (independent item check boxes) rather than only
    /// on hover and while checked (auto-check-select).
    pub check_boxes_persistent: bool,
    /// The ticked items when check boxes are independent.
    pub checked: Vec<ItemId>,
    /// Show filename extensions in labels, from Explorer's folder options.
    pub show_extensions: bool,
    /// Items that were cut; drawn ghosted until they are pasted.
    pub cut: Vec<ItemId>,
    /// Color compressed/encrypted labels, from Explorer's view options.
    pub show_compressed_color: bool,
    /// The rubber-band selection rectangle, in window-local coordinates.
    pub marquee: Option<Rect>,
}

pub fn space_style(theme: &Theme, system_dark: bool) -> SpaceStyle {
    SpaceStyle {
        backdrop: theme.backdrop,
        corner_radius: theme.effective_corner_radius() as i32,
        dark: theme.is_dark(system_dark),
        border: theme.show_border,
    }
}

/// Builds the draw list for one space window. Item rects are relative to the
/// window, which starts at the space's top-left corner. Only items at least
/// partly inside the content area are included, so icons of scrolled-away
/// items are neither drawn nor loaded.
#[allow(clippy::too_many_arguments)]
pub fn space_scene(
    space: &Space,
    theme: &Theme,
    palette: &Palette,
    layout: GridSpec,
    metrics: ItemMetrics,
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
                label: item_label(item, view.show_extensions),
                icon: icons.icon_for(&item.path, metrics.icon_size, theme.overlay_size),
                state: if view.dragged.contains(&item.id) {
                    ItemState::Dragged
                } else if view.selected.contains(&item.id) {
                    ItemState::Selected
                } else if view.hovered == Some(item.id) {
                    ItemState::Hovered
                } else {
                    ItemState::Normal
                },
                hovered: view.hovered == Some(item.id),
                // The tick mirrors selection in auto-check-select mode, and the
                // independent check set when item check boxes are separate.
                checked: if view.check_boxes_persistent {
                    view.checked.contains(&item.id)
                } else {
                    view.selected.contains(&item.id)
                },
                cut: view.cut.contains(&item.id),
                label_color: compressed_label(item, view.show_compressed_color),
            })
            .collect()
    };

    // A per-space tint keeps the theme's alpha; text overrides the title and
    // label color together.
    let tint = space
        .tint
        .map_or(palette.tint, |color| color.with_alpha(palette.tint.a))
        .fade(space.opacity);
    let title_text = space.text.unwrap_or(palette.title_text);
    let item_text = space.text.unwrap_or(palette.item_text);
    let text_shadow = match theme.label_shadow {
        LabelShadow::Auto => auto_text_shadow(item_text, tint),
        LabelShadow::On => Some(contrast_shadow(item_text)),
        LabelShadow::Off => None,
    };

    SpaceScene {
        title: space.title.clone(),
        title_height: space.title_height,
        tint,
        title_fill: palette.title_fill,
        title_text,
        item_text,
        hover_fill: palette.hover_fill,
        selected_fill: palette.selected_fill,
        border: palette.border,
        text_shadow,
        label_lines: theme.label_lines.clamp(1, 2),
        label_size: theme.label_size.clamp(60, 160),
        locked: space.locked,
        rolled_up: space.rolled_up,
        editing: None,
        editing_item: None,
        edit: palette.edit,
        caret_visible: true,
        metrics,
        items,
        show_check_boxes: view.show_check_boxes,
        check_boxes_persistent: view.check_boxes_persistent,
        marquee: view.marquee,
        scroll_thumb: scroll_thumb(
            space,
            layout,
            view.scroll,
            theme.effective_corner_radius() as i32,
        ),
    }
}

/// Builds the scene used to render a drag image: the space's draw list reduced
/// to the dragged `ids`. A selected item scrolled out of view is included too,
/// at the grid position it would occupy, so a group drag shows the whole
/// selection exactly where Explorer would.
#[allow(clippy::too_many_arguments)]
pub fn drag_scene(
    space: &Space,
    ids: &[ItemId],
    theme: &Theme,
    palette: &Palette,
    layout: GridSpec,
    metrics: ItemMetrics,
    view: ViewState,
    icons: &mut IconCache,
) -> SpaceScene {
    let scroll = view.scroll;
    let show_extensions = view.show_extensions;
    let mut scene = space_scene(space, theme, palette, layout, metrics, view, icons);
    let cell = layout.cell();
    let content_top = space.title_height;
    scene.items = ids
        .iter()
        .filter_map(|id| {
            let item = space.items.iter().find(|item| item.id == *id)?;
            Some(ItemVisual {
                id: item.id.0,
                rect: Rect::new(
                    item.position.x,
                    content_top + item.position.y - scroll,
                    cell.width,
                    cell.height,
                ),
                label: item_label(item, show_extensions),
                icon: icons.icon_for(&item.path, metrics.icon_size, theme.overlay_size),
                state: ItemState::Normal,
                hovered: false,
                checked: false,
                cut: false,
                label_color: None,
            })
        })
        .collect();
    scene
}

/// The item's display label. Explorer never shows a shortcut's extension, even
/// when "File name extensions" is on, so only real files can gain one; both
/// forms come from the path, so toggling the option is reflected live and a
/// label captured while extensions were on does not keep its extension.
pub(crate) fn item_label(item: &Item, show_extensions: bool) -> String {
    if item.kind == ItemKind::File {
        if let Some(name) = item.file_name() {
            if show_extensions {
                return name.to_string();
            }
            if let Some(stem) = item.path.file_stem().and_then(|stem| stem.to_str()) {
                return stem.to_string();
            }
        }
    }
    item.label.clone()
}

/// Explorer's compressed (blue) and encrypted (green) label colors, used when
/// the folder option `SSF_SHOWCOMPCOLOR` is on.
const COMPRESSED_LABEL: Color = Color::rgb(0, 0, 255);
const ENCRYPTED_LABEL: Color = Color::rgb(0, 128, 0);

/// The label color override for a compressed or encrypted item.
fn compressed_label(item: &Item, enabled: bool) -> Option<Color> {
    if !enabled {
        return None;
    }
    if item.encrypted {
        Some(ENCRYPTED_LABEL)
    } else if item.compressed {
        Some(COMPRESSED_LABEL)
    } else {
        None
    }
}

/// A dark shadow under light text (or a light one under dark text) when the
/// space is nearly transparent, like the labels of desktop icons. `text` and
/// `tint` are the space's effective colors, after any per-space overrides.
fn auto_text_shadow(text: Color, tint: Color) -> Option<Color> {
    (tint.a < TEXT_SHADOW_BELOW_ALPHA).then(|| contrast_shadow(text))
}

/// A shadow that contrasts with `text`, so it stays legible on any backdrop.
fn contrast_shadow(text: Color) -> Color {
    if text.luminance() > 0.5 {
        Color::rgba(0, 0, 0, 170)
    } else {
        Color::rgba(255, 255, 255, 140)
    }
}

/// Keeps the scroll indicator's bottom clear of the rounded corner.
fn thumb_bottom_margin(corner_radius: i32) -> i32 {
    SCROLL_THUMB_MARGIN + corner_radius / 2
}

/// A thin bar along the right edge whose length and position show which part
/// of the content is visible. `None` when everything fits.
fn scroll_thumb(space: &Space, layout: GridSpec, scroll: i32, corner_radius: i32) -> Option<Rect> {
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
    let track_height = viewport - SCROLL_THUMB_MARGIN - thumb_bottom_margin(corner_radius);
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
    fn compressed_and_encrypted_items_get_explorer_colors() {
        let mut compressed = Item::new(ItemId(4), "C:/missing/a.txt");
        compressed.compressed = true;
        assert_eq!(compressed_label(&compressed, true), Some(COMPRESSED_LABEL));
        let mut encrypted = Item::new(ItemId(5), "C:/missing/b.txt");
        encrypted.encrypted = true;
        assert_eq!(compressed_label(&encrypted, true), Some(ENCRYPTED_LABEL));
        assert_eq!(compressed_label(&encrypted, false), None);
        assert_eq!(
            compressed_label(&Item::new(ItemId(6), "C:/missing/c.txt"), true),
            None
        );
    }

    #[test]
    fn extensions_are_shown_only_when_requested() {
        let file = Item::new(ItemId(4), "C:/missing/report.txt");
        assert_eq!(item_label(&file, false), "report");
        assert_eq!(item_label(&file, true), "report.txt");
        let folder = Item::new(ItemId(5), "C:/missing/folder").with_kind(ItemKind::Folder);
        assert_eq!(item_label(&folder, true), "folder");
    }

    #[test]
    fn shortcuts_never_show_their_extension() {
        let lnk = Item::new(ItemId(4), "C:/missing/Notepad.lnk");
        assert_eq!(item_label(&lnk, false), "Notepad");
        assert_eq!(item_label(&lnk, true), "Notepad");
        let url = Item::new(ItemId(5), "C:/missing/example.url");
        assert_eq!(item_label(&url, true), "example");
    }

    #[test]
    fn a_stored_extension_is_hidden_after_the_option_turns_off() {
        let mut file = Item::new(ItemId(4), "C:/missing/report.txt");
        // As captured from the shell while Explorer showed extensions.
        file.label = "report.txt".to_string();
        assert_eq!(item_label(&file, false), "report");
        assert_eq!(item_label(&file, true), "report.txt");
    }

    #[test]
    fn independent_check_boxes_use_the_checked_set() {
        let space = space();
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            &theme,
            &theme.palette(true),
            GridSpec::default(),
            ItemMetrics::for_scale(1.0),
            ViewState {
                show_check_boxes: true,
                check_boxes_persistent: true,
                checked: vec![ItemId(4)],
                ..ViewState::default()
            },
            &mut IconCache::new(),
        );
        assert!(scene.check_boxes_persistent);
        assert!(scene.items[0].checked);
    }

    #[test]
    fn a_cut_item_is_marked_in_the_scene() {
        let space = space();
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            &theme,
            &theme.palette(true),
            GridSpec::default(),
            ItemMetrics::for_scale(1.0),
            ViewState {
                cut: vec![ItemId(4)],
                ..ViewState::default()
            },
            &mut IconCache::new(),
        );
        assert!(scene.items[0].cut);
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
            ItemMetrics::for_scale(1.0),
            ViewState::default(),
            &mut IconCache::new(),
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
            ItemMetrics::for_scale(1.0),
            ViewState::default(),
            &mut IconCache::new(),
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
            ItemMetrics::for_scale(1.0),
            ViewState {
                hovered: Some(ItemId(4)),
                selected: vec![ItemId(4)],
                ..ViewState::default()
            },
            &mut IconCache::new(),
        );
        assert_eq!(scene.items[0].state, ItemState::Selected);
        assert_eq!(scene.tint.a, palette.tint.fade(128).a);
    }

    #[test]
    fn selection_ticks_the_check_box() {
        let space = space();
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            &theme,
            &theme.palette(true),
            GridSpec::default(),
            ItemMetrics::for_scale(1.0),
            ViewState {
                selected: vec![ItemId(4)],
                ..ViewState::default()
            },
            &mut IconCache::new(),
        );
        assert!(scene.items[0].checked);
    }

    #[test]
    fn a_dragged_item_is_drawn_as_dragged_even_when_selected() {
        let space = space();
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            &theme,
            &theme.palette(true),
            GridSpec::default(),
            ItemMetrics::for_scale(1.0),
            ViewState {
                selected: vec![ItemId(4)],
                dragged: vec![ItemId(4)],
                ..ViewState::default()
            },
            &mut IconCache::new(),
        );
        assert_eq!(scene.items[0].state, ItemState::Dragged);
    }

    #[test]
    fn per_space_appearance_overrides_the_palette() {
        let mut space = space();
        space.tint = Some(Color::rgb(10, 20, 30));
        space.text = Some(Color::rgb(1, 2, 3));
        let theme = Theme::default();
        let palette = theme.palette(true);
        let scene = space_scene(
            &space,
            &theme,
            &palette,
            GridSpec::default(),
            ItemMetrics::for_scale(1.0),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(scene.tint.r, 10);
        assert_eq!(scene.tint.a, palette.tint.a);
        assert_eq!(scene.title_text, Color::rgb(1, 2, 3));
        assert_eq!(scene.item_text, Color::rgb(1, 2, 3));
    }

    #[test]
    fn label_style_flows_into_the_scene() {
        let space = space();
        let theme = Theme {
            label_lines: 1,
            label_size: 140,
            label_shadow: LabelShadow::On,
            ..Theme::default()
        };
        let scene = space_scene(
            &space,
            &theme,
            &theme.palette(true),
            GridSpec::default(),
            ItemMetrics::for_scale(1.0),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(scene.label_lines, 1);
        assert_eq!(scene.label_size, 140);
        assert!(scene.text_shadow.is_some());
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
                ItemMetrics::for_scale(1.0),
                ViewState {
                    scroll,
                    ..ViewState::default()
                },
                &mut IconCache::new(),
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
    fn drag_scene_keeps_a_scrolled_out_member_at_its_grid_position() {
        let space = tall_space();
        let theme = Theme::default();
        // Item 1 is in the first row; item 10 is several rows down, past the
        // content area, so the space's own scene would not draw it.
        let ids = vec![ItemId(1), ItemId(10)];
        let scene = drag_scene(
            &space,
            &ids,
            &theme,
            &theme.palette(true),
            GridSpec::default(),
            ItemMetrics::for_scale(1.0),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(
            scene.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            [1, 10]
        );
        assert!(scene.items[1].rect.y > scene.items[0].rect.bottom());
        assert_eq!(scene.items[0].state, ItemState::Normal);
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
                ItemMetrics::for_scale(1.0),
                ViewState {
                    scroll,
                    ..ViewState::default()
                },
                &mut IconCache::new(),
            )
            .scroll_thumb
            .unwrap()
        };
        let (start, end) = (thumb(0), thumb(max));
        assert_eq!(start.y, tall.title_height + SCROLL_THUMB_MARGIN);
        assert_eq!(
            end.bottom(),
            tall.rect.height - thumb_bottom_margin(theme.effective_corner_radius() as i32)
        );
        assert_eq!(start.right(), tall.rect.width - SCROLL_THUMB_MARGIN);

        let short = space();
        let scene = space_scene(
            &short,
            &theme,
            &theme.palette(true),
            layout,
            ItemMetrics::for_scale(1.0),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(scene.scroll_thumb, None);
    }

    #[test]
    fn nearly_transparent_spaces_shadow_their_text() {
        let clear = Theme {
            backdrop: deskspace_core::Backdrop::Clear,
            tint_opacity: 5,
            ..Theme::default()
        };
        let dark = clear.palette(true);
        assert_eq!(
            auto_text_shadow(dark.item_text, dark.tint.fade(255)).map(|c| c.r),
            Some(0)
        );
        let light = clear.palette(false);
        assert_eq!(
            auto_text_shadow(light.item_text, light.tint.fade(255)).map(|c| c.r),
            Some(255)
        );
        let default = Theme::default().palette(true);
        assert_eq!(
            auto_text_shadow(default.item_text, default.tint.fade(255)),
            None
        );
        // A low per-space opacity makes an otherwise opaque tint transparent.
        assert!(auto_text_shadow(default.item_text, default.tint.fade(20)).is_some());
    }
}
