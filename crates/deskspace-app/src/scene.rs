use deskspace_core::item::ItemKind;
use deskspace_core::theme::LABEL_SIZE_RANGE;
use deskspace_core::view_layout::{ViewLayout, ViewMetrics};
use deskspace_core::{
    Backdrop, Color, DesktopLayer, GridSpec, Item, ItemId, LabelShadow, Palette, Point, Rect,
    Space, Theme, ViewMode,
};
use deskspace_platform::{
    DropInto, IconCache, ItemLayout, ItemMetrics, ItemState, ItemVisual, SpaceScene, SpaceStyle,
};

const SCROLL_THUMB_WIDTH: i32 = 4;
const SCROLL_THUMB_MARGIN: i32 = 3;
const SCROLL_THUMB_MIN_HEIGHT: i32 = 24;
/// Below this tint alpha the backdrop shows through so much that labels need
/// a shadow to stay readable on busy wallpapers.
const TEXT_SHADOW_BELOW_ALPHA: u8 = 64;

/// How a surface is styled and sized: everything a scene builder needs besides
/// the surface's items and its transient `ViewState`.
#[derive(Clone, Copy)]
pub struct SurfaceLook<'a> {
    pub theme: &'a Theme,
    pub palette: &'a Palette,
    pub layout: GridSpec,
    pub metrics: ItemMetrics,
    /// The display scale (1.0 is 96 DPI) that order-placed views size their
    /// inline cells with. Icon views already carry it in `layout`.
    pub scale: f32,
}

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
    /// Draw a highlight outline, marking the space selected in the settings
    /// window.
    pub highlight: bool,
    /// The item with keyboard focus, drawn with a focus rectangle.
    pub focused: Option<ItemId>,
    /// A lens whose folder cannot be listed (missing, or refused because it
    /// is DeskSpace's storage): the scene says so in place of items.
    pub folder_unavailable: bool,
}

pub fn space_style(theme: &Theme, system_dark: bool) -> SpaceStyle {
    SpaceStyle {
        backdrop: theme.backdrop,
        corner_radius: theme.effective_corner_radius() as i32,
        dark: theme.is_dark(system_dark),
        border: theme.show_border,
        border_width: theme.border_width.into(),
        chrome: true,
    }
}

/// The window style for the chrome-less desktop layer: no border or rounded
/// corners, and a clear backdrop so only the loose items are drawn.
pub fn layer_style(theme: &Theme, system_dark: bool) -> SpaceStyle {
    SpaceStyle {
        backdrop: Backdrop::Clear,
        corner_radius: 0,
        dark: theme.is_dark(system_dark),
        border: false,
        border_width: 0,
        chrome: false,
    }
}

/// Builds the draw list for one space window. Item rects are relative to the
/// window, which starts at the space's top-left corner. Only items at least
/// partly inside the content area are included, so icons of scrolled-away
/// items are neither drawn nor loaded.
pub fn space_scene(
    space: &Space,
    look: SurfaceLook,
    view: ViewState,
    icons: &mut IconCache,
) -> SpaceScene {
    let SurfaceLook {
        theme,
        palette,
        layout,
        metrics,
        scale,
    } = look;
    let view_layout = ViewLayout::for_space(
        space,
        ViewMetrics::for_view(space.view().mode, layout, scale),
    );
    let content_top = space.title_height;
    let content_bottom = space.rect().height;
    let items = if space.folded() {
        Vec::new()
    } else {
        let mut items = Vec::new();
        for index in view_layout.visible(view.scroll, space.content_rect().height) {
            let Some(item) = space.items().get(index) else {
                continue;
            };
            let cell = view_layout.rect(index);
            let rect = Rect::new(
                cell.x,
                content_top + cell.y - view.scroll,
                cell.width,
                cell.height,
            );
            if rect.bottom() <= content_top || rect.y >= content_bottom {
                continue;
            }
            items.push(ItemVisual {
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
            });
        }
        items
    };

    let tint = palette.tint;
    let title_text = palette.title_text;
    let item_text = palette.item_text;
    let text_shadow = match theme.label_shadow {
        LabelShadow::Auto => auto_text_shadow(item_text, tint),
        LabelShadow::On => Some(contrast_shadow(item_text)),
        LabelShadow::Off => None,
    };

    SpaceScene {
        chrome: true,
        title: space.title().to_owned(),
        title_height: space.title_height,
        tint,
        title_fill: palette.title_fill,
        title_text,
        item_text,
        hover_fill: palette.hover_fill,
        selected_fill: palette.selected_fill,
        marquee_color: palette.marquee,
        show_marquee_border: theme.show_marquee_border,
        check_fill: palette.check_fill,
        border: palette.border,
        text_shadow,
        label_lines: theme.label_lines.clamp(1, 2),
        label_size: theme
            .label_size
            .clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1),
        locked: space.locked(),
        folded: space.folded(),
        highlight: view.highlight,
        drop_into: if space.is_lens() {
            DropInto::Folder
        } else {
            DropInto::Space
        },
        placeholder: space
            .folder_view()
            .filter(|_| view.folder_unavailable)
            .map(|view| format!("Folder not available\n{}", view.path.display())),
        focused_item: view.focused.map(|id| id.0),
        editing: None,
        editing_item: None,
        edit: palette.edit,
        caret_visible: true,
        metrics,
        item_layout: item_layout(space.view().mode),
        items,
        show_check_boxes: view.show_check_boxes,
        check_boxes_persistent: view.check_boxes_persistent,
        marquee: view.marquee,
        scroll_thumb: scroll_thumb(
            space,
            &view_layout,
            view.scroll,
            theme.effective_corner_radius() as i32,
        ),
    }
}

/// Builds the draw list for the chrome-less desktop layer (ROADMAP D7). A
/// layer item's position is absolute, in virtual-desktop coordinates, so
/// `origin` (the layer window's top-left) is subtracted for window-local
/// rects. It has no title, border, or scroll.
pub fn desktop_layer_scene(
    layer: &DesktopLayer,
    origin: Point,
    look: SurfaceLook,
    view: ViewState,
    icons: &mut IconCache,
) -> SpaceScene {
    let SurfaceLook {
        theme,
        palette,
        layout,
        metrics,
        ..
    } = look;
    let cell = layout.cell();
    let items = layer
        .items()
        .iter()
        .map(|item| ItemVisual {
            id: item.id.0,
            rect: Rect::new(
                item.position.x - origin.x,
                item.position.y - origin.y,
                cell.width,
                cell.height,
            ),
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
            checked: view.checked.contains(&item.id),
            cut: view.cut.contains(&item.id),
            label_color: compressed_label(item, view.show_compressed_color),
        })
        .collect();

    // The layer is transparent, so its labels always carry Explorer's desktop
    // shadow to stay readable over the wallpaper, unless shadows are off.
    let text_shadow = match theme.label_shadow {
        LabelShadow::Off => None,
        LabelShadow::Auto | LabelShadow::On => Some(contrast_shadow(palette.item_text)),
    };

    SpaceScene {
        chrome: false,
        title: String::new(),
        title_height: 0,
        tint: Color::TRANSPARENT,
        title_fill: Color::TRANSPARENT,
        title_text: palette.title_text,
        item_text: palette.item_text,
        hover_fill: palette.hover_fill,
        selected_fill: palette.selected_fill,
        marquee_color: palette.marquee,
        show_marquee_border: theme.show_marquee_border,
        check_fill: palette.check_fill,
        border: Color::TRANSPARENT,
        text_shadow,
        label_lines: theme.label_lines.clamp(1, 2),
        label_size: theme
            .label_size
            .clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1),
        locked: false,
        folded: false,
        highlight: false,
        drop_into: DropInto::Space,
        placeholder: None,
        focused_item: view.focused.map(|id| id.0),
        editing: None,
        editing_item: None,
        edit: palette.edit,
        caret_visible: true,
        metrics,
        item_layout: ItemLayout::Grid,
        items,
        show_check_boxes: view.show_check_boxes,
        check_boxes_persistent: view.check_boxes_persistent,
        marquee: view.marquee,
        scroll_thumb: None,
    }
}

/// Builds the scene used to render a drag image from the loose desktop layer:
/// the layer's draw list reduced to the dragged `ids`, at their absolute
/// positions. Mirrors `drag_scene` for spaces.
pub fn desktop_layer_drag_scene(
    layer: &DesktopLayer,
    origin: Point,
    ids: &[ItemId],
    look: SurfaceLook,
    view: ViewState,
    icons: &mut IconCache,
) -> SpaceScene {
    let show_extensions = view.show_extensions;
    let cell = look.layout.cell();
    let (icon_size, overlay_size) = (look.metrics.icon_size, look.theme.overlay_size);
    let mut scene = desktop_layer_scene(layer, origin, look, view, icons);
    scene.items = ids
        .iter()
        .filter_map(|id| {
            let item = layer.items().iter().find(|item| item.id == *id)?;
            Some(ItemVisual {
                id: item.id.0,
                rect: Rect::new(
                    item.position.x - origin.x,
                    item.position.y - origin.y,
                    cell.width,
                    cell.height,
                ),
                label: item_label(item, show_extensions),
                icon: icons.icon_for(&item.path, icon_size, overlay_size),
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

/// Builds the scene used to render a drag image: the space's draw list reduced
/// to the dragged `ids`. A selected item scrolled out of view is included too,
/// at the grid position it would occupy, so a group drag shows the whole
/// selection exactly where Explorer would.
pub fn drag_scene(
    space: &Space,
    ids: &[ItemId],
    look: SurfaceLook,
    view: ViewState,
    icons: &mut IconCache,
) -> SpaceScene {
    let scroll = view.scroll;
    let show_extensions = view.show_extensions;
    let (icon_size, overlay_size) = (look.metrics.icon_size, look.theme.overlay_size);
    let view_layout = ViewLayout::for_space(
        space,
        ViewMetrics::for_view(space.view().mode, look.layout, look.scale),
    );
    let mut scene = space_scene(space, look, view, icons);
    let content_top = space.title_height;
    scene.items = ids
        .iter()
        .filter_map(|id| {
            let index = space.items().iter().position(|item| item.id == *id)?;
            let item = &space.items()[index];
            let cell = view_layout.rect(index);
            Some(ItemVisual {
                id: item.id.0,
                rect: Rect::new(
                    cell.x,
                    content_top + cell.y - scroll,
                    cell.width,
                    cell.height,
                ),
                label: item_label(item, show_extensions),
                icon: icons.icon_for(&item.path, icon_size, overlay_size),
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

/// How `mode`'s item cells place their icon and label. The painter and the
/// check-box hit test both read it, so they always agree.
pub(crate) fn item_layout(mode: ViewMode) -> ItemLayout {
    match mode {
        ViewMode::SmallIcons => ItemLayout::Inline,
        ViewMode::Icons
        | ViewMode::ExtraLargeIcons
        | ViewMode::LargeIcons
        | ViewMode::MediumIcons => ItemLayout::Grid,
    }
}

/// The item's display label. Explorer never shows a shortcut's extension, even
/// when "File name extensions" is on, so only real files can gain one; both
/// forms come from the path, so toggling the option is reflected live and a
/// label captured while extensions were on does not keep its extension.
pub(crate) fn item_label(item: &Item, show_extensions: bool) -> String {
    if item.kind == ItemKind::File
        && let Some(name) = item.file_name()
    {
        if show_extensions {
            return name.to_string();
        }
        if let Some(stem) = item.path.file_stem().and_then(|stem| stem.to_str()) {
            return stem.to_string();
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
fn scroll_thumb(
    space: &Space,
    layout: &ViewLayout,
    scroll: i32,
    corner_radius: i32,
) -> Option<Rect> {
    if space.folded() {
        return None;
    }
    let viewport = space.content_rect().height;
    let max_scroll = layout.max_scroll(viewport);
    if max_scroll == 0 {
        return None;
    }
    let extent = layout.extent();
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
        space.rect().width - SCROLL_THUMB_MARGIN - SCROLL_THUMB_WIDTH,
        track_top + offset,
        SCROLL_THUMB_WIDTH,
        thumb_height,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::apply;
    use deskspace_core::ItemFacts;
    use deskspace_core::{Command, Desktop, Item, Point, Size, SpaceId, SpaceLook};

    /// A one-item "Work" space built the way the app builds one, through
    /// `Command`s, with `change` applied last.
    fn work_space_after(change: impl FnOnce(SpaceId) -> Command) -> Space {
        let mut desktop = Desktop::new();
        let id = desktop.next_space_id();
        apply(
            &mut desktop,
            Command::CreateSpace {
                title: "Work".to_owned(),
                at: Point::new(500, 300),
                size: Size::new(240, 200),
                locked: false,
            },
        );
        apply(
            &mut desktop,
            Command::AddItem {
                space: id,
                path: "C:/missing/report.txt".into(),
                facts: ItemFacts::default(),
            },
        );
        apply(&mut desktop, change(id));
        desktop.space(id).unwrap().clone()
    }

    fn space() -> Space {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(500, 300, 240, 200));
        let mut item = Item::new(ItemId(4), "C:/missing/report.txt", ItemFacts::default());
        item.position = Point::new(12, 12);
        space.push_item(item);
        space
    }

    fn surface_look<'a>(theme: &'a Theme, palette: &'a Palette) -> SurfaceLook<'a> {
        SurfaceLook {
            theme,
            palette,
            layout: GridSpec::default(),
            metrics: ItemMetrics::for_scale(1.0),
            scale: 1.0,
        }
    }

    #[test]
    fn compressed_and_encrypted_items_get_explorer_colors() {
        let mut compressed = Item::new(ItemId(4), "C:/missing/a.txt", ItemFacts::default());
        compressed.compressed = true;
        assert_eq!(compressed_label(&compressed, true), Some(COMPRESSED_LABEL));
        let mut encrypted = Item::new(ItemId(5), "C:/missing/b.txt", ItemFacts::default());
        encrypted.encrypted = true;
        assert_eq!(compressed_label(&encrypted, true), Some(ENCRYPTED_LABEL));
        assert_eq!(compressed_label(&encrypted, false), None);
        assert_eq!(
            compressed_label(
                &Item::new(ItemId(6), "C:/missing/c.txt", ItemFacts::default()),
                true
            ),
            None
        );
    }

    #[test]
    fn extensions_are_shown_only_when_requested() {
        let file = Item::new(ItemId(4), "C:/missing/report.txt", ItemFacts::default());
        assert_eq!(item_label(&file, false), "report");
        assert_eq!(item_label(&file, true), "report.txt");
        let folder = Item::new(ItemId(5), "C:/missing/folder", ItemFacts::default())
            .with_kind(ItemKind::Folder);
        assert_eq!(item_label(&folder, true), "folder");
    }

    #[test]
    fn shortcuts_never_show_their_extension() {
        let lnk = Item::new(ItemId(4), "C:/missing/Notepad.lnk", ItemFacts::default());
        assert_eq!(item_label(&lnk, false), "Notepad");
        assert_eq!(item_label(&lnk, true), "Notepad");
        let url = Item::new(ItemId(5), "C:/missing/example.url", ItemFacts::default());
        assert_eq!(item_label(&url, true), "example");
    }

    #[test]
    fn a_stored_extension_is_hidden_after_the_option_turns_off() {
        let mut file = Item::new(ItemId(4), "C:/missing/report.txt", ItemFacts::default());
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
            surface_look(&theme, &theme.palette(true)),
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
            surface_look(&theme, &theme.palette(true)),
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
            surface_look(&theme, &theme.palette(true)),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(
            scene.items[0].rect,
            Rect::new(12, space.title_height + 12, 96, 104)
        );
    }

    #[test]
    fn a_small_icons_space_draws_inline_cells() {
        let mut desktop = Desktop::new();
        let id = crate::fixtures::create_space(&mut desktop, "Work", Rect::new(500, 300, 240, 200));
        crate::fixtures::add_item(&mut desktop, id, "C:/missing/report.txt");
        crate::fixtures::apply(
            &mut desktop,
            Command::SetSpaceView {
                id,
                view: deskspace_core::SpaceView::with_mode(ViewMode::SmallIcons),
            },
        );
        let space = desktop.space(id).unwrap();
        let theme = Theme::default();
        let look = SurfaceLook {
            theme: &theme,
            palette: &theme.palette(true),
            layout: GridSpec::default(),
            metrics: ItemMetrics {
                icon_size: 16,
                ..ItemMetrics::for_scale(1.0)
            },
            scale: 1.0,
        };
        let scene = space_scene(space, look, ViewState::default(), &mut IconCache::new());
        assert_eq!(scene.item_layout, ItemLayout::Inline);
        assert_eq!(scene.metrics.icon_size, 16);
        assert_eq!(
            scene.items[0].rect,
            Rect::new(12, space.title_height + 12, 200, 22)
        );
    }

    #[test]
    fn folded_spaces_draw_no_items() {
        let space = work_space_after(|id| Command::ToggleFold { id });
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            surface_look(&theme, &theme.palette(true)),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert!(scene.items.is_empty());
        assert!(scene.folded);
    }

    #[test]
    fn selection_wins_over_hover() {
        let space = space();
        let theme = Theme::default();
        let palette = theme.palette(true);
        let scene = space_scene(
            &space,
            surface_look(&theme, &palette),
            ViewState {
                hovered: Some(ItemId(4)),
                selected: vec![ItemId(4)],
                ..ViewState::default()
            },
            &mut IconCache::new(),
        );
        assert_eq!(scene.items[0].state, ItemState::Selected);
        assert_eq!(scene.tint, palette.tint);
    }

    #[test]
    fn selection_ticks_the_check_box() {
        let space = space();
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            surface_look(&theme, &theme.palette(true)),
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
            surface_look(&theme, &theme.palette(true)),
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
    fn the_settings_highlight_reaches_the_scene() {
        let space = space();
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            surface_look(&theme, &theme.palette(true)),
            ViewState {
                highlight: true,
                ..ViewState::default()
            },
            &mut IconCache::new(),
        );
        assert!(scene.highlight);
        assert!(!ViewState::default().highlight);
    }

    #[test]
    fn a_lens_drops_into_its_folder_and_everything_else_into_a_space() {
        let theme = Theme::default();
        let palette = theme.palette(true);
        let look = surface_look(&theme, &palette);
        let drop_into = |space: &Space| {
            space_scene(space, look, ViewState::default(), &mut IconCache::new()).drop_into
        };
        let mut desktop = Desktop::new();
        let lens = crate::fixtures::create_lens(
            &mut desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            "C:/Users/me/Downloads",
        );
        assert_eq!(drop_into(desktop.space(lens).unwrap()), DropInto::Folder);
        assert_eq!(drop_into(&space()), DropInto::Space);
        let layer = desktop_layer_scene(
            &DesktopLayer::default(),
            Point::new(0, 0),
            look,
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(layer.drop_into, DropInto::Space);
    }

    #[test]
    fn only_an_unavailable_lens_shows_the_placeholder() {
        let theme = Theme::default();
        let palette = theme.palette(true);
        let look = surface_look(&theme, &palette);
        let placeholder = |space: &Space, folder_unavailable| {
            let view = ViewState {
                folder_unavailable,
                ..ViewState::default()
            };
            space_scene(space, look, view, &mut IconCache::new()).placeholder
        };
        let mut desktop = Desktop::new();
        let lens = crate::fixtures::create_lens(
            &mut desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            "C:/Users/me/Downloads",
        );
        let lens = desktop.space(lens).unwrap();
        let text = placeholder(lens, true).expect("an unavailable lens says so");
        assert!(text.starts_with("Folder not available\n"));
        assert!(text.contains("Downloads"));
        assert_eq!(placeholder(lens, false), None, "an available lens");
        assert_eq!(placeholder(&space(), true), None, "a normal space");
    }

    #[test]
    fn the_selection_box_color_and_border_reach_the_scene() {
        let space = space();
        let theme = Theme {
            marquee_color: Color::rgb(10, 20, 30),
            show_marquee_border: false,
            ..Theme::default()
        };
        let scene = space_scene(
            &space,
            surface_look(&theme, &theme.palette(true)),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(scene.marquee_color, Color::rgb(10, 20, 30));
        assert!(!scene.show_marquee_border);
    }

    #[test]
    fn the_focused_item_reaches_the_scene() {
        let space = space();
        let theme = Theme::default();
        let scene = space_scene(
            &space,
            surface_look(&theme, &theme.palette(true)),
            ViewState {
                focused: Some(ItemId(4)),
                ..ViewState::default()
            },
            &mut IconCache::new(),
        );
        assert_eq!(scene.focused_item, Some(4));
    }

    #[test]
    fn desktop_layer_scene_maps_absolute_positions() {
        let mut layer = DesktopLayer::default();
        let mut item = Item::new(ItemId(4), "C:/missing/report.txt", ItemFacts::default());
        item.position = Point::new(500, 300);
        layer.push_item(item);
        let theme = Theme::default();
        let scene = desktop_layer_scene(
            &layer,
            Point::new(100, 100),
            surface_look(&theme, &theme.palette(true)),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert!(!scene.chrome);
        assert!(scene.scroll_thumb.is_none());
        assert_eq!(scene.items.len(), 1);
        assert_eq!(scene.items[0].rect.x, 400);
        assert_eq!(scene.items[0].rect.y, 200);
        assert!(scene.text_shadow.is_some());
    }

    #[test]
    fn desktop_layer_drag_scene_keeps_only_the_dragged_ids() {
        let mut layer = DesktopLayer::default();
        for (n, position) in [(1u64, Point::new(500, 300)), (2, Point::new(700, 300))] {
            let mut item = Item::new(
                ItemId(n),
                format!("C:/missing/{n}.txt"),
                ItemFacts::default(),
            );
            item.position = position;
            layer.push_item(item);
        }
        let theme = Theme::default();
        let scene = desktop_layer_drag_scene(
            &layer,
            Point::new(100, 100),
            &[ItemId(2)],
            surface_look(&theme, &theme.palette(true)),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(scene.items.len(), 1);
        assert_eq!(scene.items[0].id, 2);
        assert_eq!(scene.items[0].rect.x, 600);
        assert_eq!(scene.items[0].rect.y, 200);
    }

    #[test]
    fn a_customized_space_draws_with_its_own_theme() {
        let global = Theme::default();
        let mut look = SpaceLook::from_global(&global, &GridSpec::default());
        look.theme.custom_colors = true;
        look.theme.tint = Color::rgb(10, 20, 30);
        look.theme.text = Color::rgb(1, 2, 3);
        look.theme.label_lines = 1;
        let space = work_space_after(|id| Command::SetSpaceLook {
            id,
            custom: true,
            look: Some(look),
        });
        let theme = space.theme(&global);
        let palette = theme.palette(true);
        let scene = space_scene(
            &space,
            surface_look(theme, &palette),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(scene.tint.r, 10);
        assert_eq!(scene.title_text, Color::rgb(1, 2, 3));
        assert_eq!(scene.item_text, Color::rgb(1, 2, 3));
        assert_eq!(scene.label_lines, 1);
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
            surface_look(&theme, &theme.palette(true)),
            ViewState::default(),
            &mut IconCache::new(),
        );
        assert_eq!(scene.label_lines, 1);
        assert_eq!(scene.label_size, 140);
        assert!(scene.text_shadow.is_some());
    }

    /// A user-sized space two columns wide holding ten items (ids 1 to 10),
    /// so they overflow it and it scrolls.
    fn tall_space() -> Space {
        let mut desktop = Desktop::new();
        let id = desktop.next_space_id();
        apply(
            &mut desktop,
            Command::CreateSpace {
                title: "Tall".to_owned(),
                at: Point::ZERO,
                size: Size::new(240, 200),
                locked: false,
            },
        );
        apply(&mut desktop, Command::KeepSize { id });
        for index in 0..10 {
            apply(
                &mut desktop,
                Command::AddItem {
                    space: id,
                    path: format!("C:/missing/{index}.txt").into(),
                    facts: ItemFacts::default(),
                },
            );
        }
        desktop.space(id).unwrap().clone()
    }

    #[test]
    fn only_items_inside_the_content_area_are_drawn() {
        let space = tall_space();
        let theme = Theme::default();
        let build = |scroll| {
            space_scene(
                &space,
                surface_look(&theme, &theme.palette(true)),
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
            surface_look(&theme, &theme.palette(true)),
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
        let max = ViewLayout::for_space(&tall, ViewMetrics::for_grid(layout))
            .max_scroll(tall.content_rect().height);
        let thumb = |scroll| {
            space_scene(
                &tall,
                surface_look(&theme, &theme.palette(true)),
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
            tall.rect().height - thumb_bottom_margin(theme.effective_corner_radius() as i32)
        );
        assert_eq!(start.right(), tall.rect().width - SCROLL_THUMB_MARGIN);

        let short = space();
        let scene = space_scene(
            &short,
            surface_look(&theme, &theme.palette(true)),
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
