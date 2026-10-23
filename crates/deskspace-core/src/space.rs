use serde::{Deserialize, Serialize, Serializer};

use crate::geometry::{Point, Rect, Size};
use crate::id::{ItemId, SpaceId};
use crate::item::Item;
use crate::layout::{GAP_RANGE, GridSpec};
use crate::source::SpaceSource;
use crate::theme::{Color, Theme};
use crate::view::SpaceView;

pub const DEFAULT_TITLE_HEIGHT: i32 = 28;

/// A space's own look: every look-related setting (the theme, plus the grid's
/// icon spacing and edge padding). It starts as a copy of the global settings
/// when the space is first customized. `theme.ui_mode` is not used: the
/// settings window and menus follow the global setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpaceLook {
    /// The space's own copy of the whole theme; its `ui_mode` is unused.
    pub theme: Theme,
    /// Icon spacing (`GridSpec::gap`).
    pub gap: i32,
    /// Edge padding (`GridSpec::padding`).
    pub padding: i32,
}

impl Default for SpaceLook {
    fn default() -> Self {
        Self::from_global(&Theme::default(), &GridSpec::default())
    }
}

impl SpaceLook {
    /// A look that matches the global settings.
    pub fn from_global(theme: &Theme, layout: &GridSpec) -> Self {
        Self {
            theme: theme.clone(),
            gap: layout.gap,
            padding: layout.padding,
        }
    }

    /// `layout` with this look's spacing.
    pub fn grid(&self, layout: GridSpec) -> GridSpec {
        GridSpec {
            gap: self.gap,
            padding: self.padding,
            ..layout
        }
    }

    /// The look with every ranged value inside its bounds.
    pub fn clamped(mut self) -> Self {
        self.theme = self.theme.clamped();
        self.gap = self.gap.clamp(GAP_RANGE.0, GAP_RANGE.1);
        self.padding = self.padding.clamp(GAP_RANGE.0, GAP_RANGE.1);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Space {
    pub id: SpaceId,
    pub(crate) title: String,
    /// The whole space, title bar included, in desktop coordinates.
    pub(crate) rect: Rect,
    /// What fills the space: its own items, or a folder it mirrors (a lens).
    /// A lens's items are a live cache and are never written to the config.
    #[serde(default)]
    pub(crate) source: SpaceSource,
    /// How the space lays its items out (D15). Views are not part of the look,
    /// so a normal space and a lens share this.
    #[serde(default)]
    pub(crate) view: SpaceView,
    #[serde(default)]
    pub(crate) items: Vec<Item>,
    #[serde(default)]
    pub(crate) locked: bool,
    /// Folded to its title bar. The key keeps its old name, `rolled_up`, so
    /// configs from before the rename load and older versions read new ones.
    #[serde(default, rename = "rolled_up")]
    pub(crate) folded: bool,
    /// Whether the space uses its own `look` instead of the global settings.
    #[serde(default)]
    pub(crate) custom: bool,
    /// The space's own settings. Kept while `custom` is off, so turning it
    /// back on restores them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) look: Option<SpaceLook>,
    // The per-space overrides that `look` replaced: read from older files,
    // merged into `look` by `merge_legacy_overrides`, and never written.
    #[serde(default = "default_opacity", rename = "opacity", skip_serializing)]
    legacy_opacity: u8,
    #[serde(default, rename = "tint", skip_serializing)]
    legacy_tint: Option<Color>,
    #[serde(default, rename = "text", skip_serializing)]
    legacy_text: Option<Color>,
    #[serde(default, rename = "icon_size", skip_serializing)]
    legacy_icon_size: Option<u8>,
    #[serde(default = "default_title_height")]
    pub title_height: i32,
    /// Set once the user resizes the space. A user-sized space keeps its
    /// size, flows its items into as many columns as its width allows, and
    /// scrolls when they do not fit. Otherwise it grows to fit its items.
    #[serde(default)]
    pub(crate) user_sized: bool,
}

/// Which edges of a space a resize gesture moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResizeEdges {
    pub left: bool,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
}

impl ResizeEdges {
    pub const NONE: Self = Self {
        left: false,
        top: false,
        right: false,
        bottom: false,
    };

    pub const fn any(self) -> bool {
        self.left || self.top || self.right || self.bottom
    }
}

fn default_opacity() -> u8 {
    255
}

fn default_title_height() -> i32 {
    DEFAULT_TITLE_HEIGHT
}

/// The fields of `Space` that are saved. A lens's items are a view of its
/// folder, so they are omitted: `SyncLens` fills them again on startup.
#[derive(Serialize)]
struct PersistedSpace<'a> {
    id: SpaceId,
    title: &'a str,
    rect: Rect,
    locked: bool,
    #[serde(rename = "rolled_up")]
    folded: bool,
    custom: bool,
    title_height: i32,
    user_sized: bool,
    #[serde(skip_serializing_if = "SpaceView::is_default")]
    view: SpaceView,
    #[serde(skip_serializing_if = "Option::is_none")]
    look: Option<&'a SpaceLook>,
    /// Omitted for an ordinary space, so its file looks as it always did.
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<&'a SpaceSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<&'a [Item]>,
}

impl Serialize for Space {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        PersistedSpace {
            id: self.id,
            title: &self.title,
            rect: self.rect,
            locked: self.locked,
            folded: self.folded,
            custom: self.custom,
            title_height: self.title_height,
            user_sized: self.user_sized,
            view: self.view,
            look: self.look.as_ref(),
            source: (!self.source.is_items()).then_some(&self.source),
            items: (!self.is_lens()).then_some(self.items.as_slice()),
        }
        .serialize(serializer)
    }
}

impl Space {
    pub fn new(id: SpaceId, title: impl Into<String>, rect: Rect) -> Self {
        Self {
            id,
            title: title.into(),
            rect,
            source: SpaceSource::Items,
            view: SpaceView::default(),
            items: Vec::new(),
            locked: false,
            folded: false,
            custom: false,
            look: None,
            legacy_opacity: default_opacity(),
            legacy_tint: None,
            legacy_text: None,
            legacy_icon_size: None,
            title_height: default_title_height(),
            user_sized: false,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// What fills the space.
    pub fn source(&self) -> &SpaceSource {
        &self.source
    }

    /// How the space lays its items out (D15).
    pub fn view(&self) -> SpaceView {
        self.view
    }

    /// Whether the space mirrors a folder (M4).
    pub fn is_lens(&self) -> bool {
        self.source.folder().is_some()
    }

    /// Whether the space holds its sort as items come and go: a lens always
    /// does, and a normal space when its view asks (D15).
    pub fn keeps_sorted(&self) -> bool {
        self.is_lens() || self.view.keep_sorted
    }

    /// The lens's folder and view settings, or `None` for an ordinary space.
    pub fn folder_view(&self) -> Option<&crate::source::FolderView> {
        self.source.folder()
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    pub fn folded(&self) -> bool {
        self.folded
    }

    pub fn custom(&self) -> bool {
        self.custom
    }

    pub fn look(&self) -> Option<&SpaceLook> {
        self.look.as_ref()
    }

    pub fn user_sized(&self) -> bool {
        self.user_sized
    }

    pub fn title_bar(&self) -> Rect {
        Rect::new(self.rect.x, self.rect.y, self.rect.width, self.title_height)
    }

    /// The area the space occupies on screen: only the title bar while rolled up.
    pub fn visible_rect(&self) -> Rect {
        if self.folded {
            self.title_bar()
        } else {
            self.rect
        }
    }

    pub fn content_rect(&self) -> Rect {
        if self.folded {
            return Rect::new(
                self.rect.x,
                self.rect.y + self.title_height,
                self.rect.width,
                0,
            );
        }
        Rect::new(
            self.rect.x,
            self.rect.y + self.title_height,
            self.rect.width,
            (self.rect.height - self.title_height).max(0),
        )
    }

    /// The item whose grid cell contains `point` (desktop coordinates) when
    /// the content is scrolled down by `scroll` pixels.
    pub fn item_at(&self, point: Point, cell: Size, scroll: i32) -> Option<ItemId> {
        let content = self.content_rect();
        if !content.contains(point) {
            return None;
        }
        let local = Point::new(point.x - content.x, point.y - content.y + scroll);
        self.items
            .iter()
            .find(|item| Rect::from_point_size(item.position, cell).contains(local))
            .map(|item| item.id)
    }

    /// Height of the laid-out items including the bottom padding.
    pub fn content_extent(&self, cell: Size, padding: i32) -> i32 {
        self.items
            .iter()
            .map(|item| item.position.y + cell.height + padding)
            .max()
            .unwrap_or(0)
    }

    /// How far the content can scroll before its end reaches the bottom.
    pub fn max_scroll(&self, cell: Size, padding: i32) -> i32 {
        (self.content_extent(cell, padding) - self.content_rect().height).max(0)
    }

    /// The edges a resize grabbed at `point` would move. Corners are wider
    /// than `border` so they are easy to hit. Locked and folded spaces
    /// cannot be resized.
    pub fn resize_edges_at(&self, point: Point, border: i32) -> ResizeEdges {
        if self.locked || self.folded || !self.rect.contains(point) {
            return ResizeEdges::NONE;
        }
        let corner = border * 2;
        let from_left = point.x - self.rect.x;
        let from_right = self.rect.right() - 1 - point.x;
        let from_top = point.y - self.rect.y;
        let from_bottom = self.rect.bottom() - 1 - point.y;
        let near_vertical_edge = from_left < corner || from_right < corner;
        let near_horizontal_edge = from_top < corner || from_bottom < corner;
        ResizeEdges {
            left: from_left < border || (from_left < corner && near_horizontal_edge),
            right: from_right < border || (from_right < corner && near_horizontal_edge),
            top: from_top < border || (from_top < corner && near_vertical_edge),
            bottom: from_bottom < border || (from_bottom < corner && near_vertical_edge),
        }
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn item_mut(&mut self, id: ItemId) -> Option<&mut Item> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    pub fn contains_item(&self, id: ItemId) -> bool {
        self.items.iter().any(|item| item.id == id)
    }

    pub fn push_item(&mut self, item: Item) {
        if !self.contains_item(item.id) {
            self.items.push(item);
        }
    }

    pub(crate) fn remove_item(&mut self, id: ItemId) -> Option<Item> {
        let index = self.items.iter().position(|item| item.id == id)?;
        Some(self.items.remove(index))
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn set_position(&mut self, origin: Point) {
        self.rect = self.rect.with_position(origin);
    }

    /// The space's own look while it is customized; `None` follows the global
    /// settings.
    pub fn active_look(&self) -> Option<&SpaceLook> {
        self.look.as_ref().filter(|_| self.custom)
    }

    /// The theme the space is drawn with.
    pub fn theme<'a>(&'a self, global: &'a Theme) -> &'a Theme {
        self.active_look().map_or(global, |look| &look.theme)
    }

    /// The base grid the space lays out on: `global` with the space's own
    /// spacing while it is customized.
    pub fn grid(&self, global: GridSpec) -> GridSpec {
        self.active_look().map_or(global, |look| look.grid(global))
    }

    /// Drops the per-space overrides read from an older file without applying
    /// them, for files whose values meant something else (config v1).
    pub fn clear_legacy_overrides(&mut self) {
        self.legacy_opacity = default_opacity();
        self.legacy_tint = None;
        self.legacy_text = None;
        self.legacy_icon_size = None;
    }

    /// Converts the per-space overrides of older files (opacity, tint, text,
    /// icon size) into a customized `look` that starts from the global
    /// settings. Opacity becomes the tint opacity it produced, and a tint or
    /// text color turns on custom colors, taking the other color from the
    /// palette shown at the time. Returns whether anything was merged.
    pub fn merge_legacy_overrides(
        &mut self,
        global: &Theme,
        layout: &GridSpec,
        system_dark: bool,
    ) -> bool {
        let opacity = std::mem::replace(&mut self.legacy_opacity, default_opacity());
        let tint = self.legacy_tint.take();
        let text = self.legacy_text.take();
        let icon_size = self.legacy_icon_size.take();
        if opacity == default_opacity() && tint.is_none() && text.is_none() && icon_size.is_none() {
            return false;
        }
        let mut look = SpaceLook::from_global(global, layout);
        if let Some(icon_size) = icon_size {
            look.theme.icon_size = icon_size;
        }
        if opacity != default_opacity() {
            look.theme.tint_opacity =
                ((global.tint_opacity as u32 * opacity as u32 + 127) / 255) as u8;
        }
        if tint.is_some() || text.is_some() {
            let palette = global.palette(system_dark);
            look.theme.custom_colors = true;
            look.theme.tint = tint.unwrap_or(palette.tint).with_alpha(255);
            look.theme.text = text.unwrap_or(palette.title_text);
        }
        self.look = Some(look.clamped());
        self.custom = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemFacts;

    #[test]
    fn folded_space_collapses_to_title_bar() {
        let mut space = Space::new(SpaceId(1), "F", Rect::new(10, 20, 200, 300));
        assert_eq!(space.visible_rect(), space.rect);
        space.folded = true;
        assert_eq!(
            space.visible_rect(),
            Rect::new(10, 20, 200, DEFAULT_TITLE_HEIGHT)
        );
        assert!(space.content_rect().is_empty());
    }

    #[test]
    fn item_at_uses_content_relative_cells() {
        let mut space = Space::new(SpaceId(1), "F", Rect::new(100, 100, 300, 300));
        let mut item = Item::new(ItemId(7), "a.txt", ItemFacts::default());
        item.position = Point::new(10, 10);
        space.push_item(item);
        let cell = Size::new(50, 50);
        let inside = Point::new(100 + 20, 100 + DEFAULT_TITLE_HEIGHT + 20);
        assert_eq!(space.item_at(inside, cell, 0), Some(ItemId(7)));
        assert_eq!(space.item_at(Point::new(100 + 200, 300), cell, 0), None);
        space.folded = true;
        assert_eq!(space.item_at(inside, cell, 0), None);
    }

    #[test]
    fn item_at_accounts_for_scroll() {
        let mut space = Space::new(SpaceId(1), "F", Rect::new(0, 0, 300, 100));
        let mut item = Item::new(ItemId(3), "deep.txt", ItemFacts::default());
        item.position = Point::new(10, 200);
        space.push_item(item);
        let cell = Size::new(50, 50);
        let point = Point::new(20, DEFAULT_TITLE_HEIGHT + 20);
        assert_eq!(space.item_at(point, cell, 0), None);
        assert_eq!(space.item_at(point, cell, 190), Some(ItemId(3)));
    }

    #[test]
    fn max_scroll_is_overflow_beyond_the_content_area() {
        let mut space = Space::new(SpaceId(1), "F", Rect::new(0, 0, 300, 128));
        assert_eq!(space.max_scroll(Size::new(50, 50), 10), 0);
        let mut item = Item::new(ItemId(1), "a", ItemFacts::default());
        item.position = Point::new(10, 200);
        space.push_item(item);
        // Extent 200 + 50 + 10 = 260; content height 128 - 28 = 100.
        assert_eq!(space.max_scroll(Size::new(50, 50), 10), 160);
    }

    #[test]
    fn resize_edges_cover_borders_and_wider_corners() {
        let space = Space::new(SpaceId(1), "F", Rect::new(100, 100, 200, 200));
        let edges = |x, y| space.resize_edges_at(Point::new(x, y), 6);
        assert_eq!(edges(200, 200), ResizeEdges::NONE);
        assert!(edges(101, 200).left && !edges(101, 200).top);
        assert!(edges(299, 200).right);
        assert!(edges(200, 299).bottom);
        let corner = edges(290, 297);
        assert!(corner.right && corner.bottom && !corner.left && !corner.top);
        let mut locked = space.clone();
        locked.locked = true;
        assert!(!locked.resize_edges_at(Point::new(101, 200), 6).any());
    }

    #[test]
    fn a_look_applies_only_while_customized() {
        let global = Theme::default();
        let grid = GridSpec::default();
        let mut space = Space::new(SpaceId(1), "F", Rect::new(0, 0, 200, 200));
        assert!(space.active_look().is_none());
        assert_eq!(space.theme(&global), &global);

        let mut look = SpaceLook::from_global(&global, &grid);
        look.theme.icon_size = 150;
        look.gap = 2;
        space.look = Some(look);
        assert!(space.active_look().is_none(), "kept but inactive");
        assert_eq!(space.grid(grid), grid);

        space.custom = true;
        assert_eq!(space.theme(&global).icon_size, 150);
        assert_eq!(space.grid(grid).gap, 2);
        assert_eq!(space.grid(grid).cell_width, grid.cell_width);
    }

    #[test]
    fn legacy_overrides_merge_into_a_custom_look() {
        let global = Theme::default();
        let grid = GridSpec::default();
        let mut space = Space::new(SpaceId(1), "Old", Rect::new(0, 0, 200, 200));
        space.legacy_opacity = 51;
        space.legacy_text = Some(Color::rgb(1, 2, 3));
        space.legacy_icon_size = Some(150);
        assert!(space.merge_legacy_overrides(&global, &grid, true));
        assert!(space.custom);
        let look = space.look.as_ref().unwrap();
        assert_eq!(look.theme.icon_size, 150);
        assert_eq!(look.theme.tint_opacity, 8, "40 % at a 51/255 multiplier");
        assert!(look.theme.custom_colors);
        assert_eq!(look.theme.text, Color::rgb(1, 2, 3));
        assert_eq!(look.theme.tint, global.palette(true).tint.with_alpha(255));
        assert!(!space.merge_legacy_overrides(&global, &grid, true), "once");
    }

    #[test]
    fn a_plain_space_merges_nothing() {
        let mut space = Space::new(SpaceId(1), "F", Rect::new(0, 0, 200, 200));
        assert!(!space.merge_legacy_overrides(&Theme::default(), &GridSpec::default(), false));
        assert!(!space.custom && space.look.is_none());
    }

    #[test]
    fn clamped_look_bounds_theme_and_spacing() {
        let look = SpaceLook {
            theme: Theme {
                icon_size: 1,
                overlay_size: 5,
                ..Theme::default()
            },
            gap: 999,
            padding: -10,
        }
        .clamped();
        assert_eq!(look.theme.icon_size, crate::theme::ICON_SIZE_RANGE.0);
        assert_eq!(look.gap, GAP_RANGE.1);
        assert_eq!(look.padding, GAP_RANGE.0);
    }
}
