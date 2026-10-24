//! A space's view: how its items are laid out and ordered, apart from its
//! look (D15).

use serde::{Deserialize, Serialize};

use crate::sort::SortRule;

/// A space's view settings. Every space has one; a lens is not special.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SpaceView {
    /// The layout the space uses.
    pub mode: ViewMode,
    /// How the space orders its items. A lens is always kept in this order;
    /// a normal space keeps the rule and sorts when asked.
    #[serde(skip_serializing_if = "SortRule::is_default")]
    pub sort: SortRule,
    /// Whether the space re-sorts its items whenever they change, holding the
    /// rule. A lens always does; a normal space only when this is set (D15).
    pub keep_sorted: bool,
    /// Whether the space shows Explorer's status bar along its bottom edge
    /// with the item counts and view buttons (W19).
    pub status_bar: bool,
}

impl SpaceView {
    /// The view with `mode` and the default rule, as the View menu and its
    /// keys change it.
    pub fn with_mode(mode: ViewMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    /// Whether this is the default view, which old files leave out.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Explorer's views (D15). `Icons` follows the look's icon size; the fixed icon
/// views set it for the space. `SmallIcons`, `List`, and `Details` place items
/// by their order, in inline cells, rather than on the icon grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    /// "Settings icon size": the look's size, Ctrl+wheel, and Match Explorer.
    #[default]
    Icons,
    /// 256 px at 96 DPI.
    ExtraLargeIcons,
    /// 96 px at 96 DPI.
    LargeIcons,
    /// 48 px at 96 DPI.
    MediumIcons,
    /// 16 px at 96 DPI, in an inline cell (W16).
    SmallIcons,
    /// 16 px at 96 DPI, in columns that fill top to bottom and scroll
    /// sideways (W17).
    List,
    /// One row per item at full width, under a header row (W18).
    Details,
    /// 48 px icons in wrapping tiles with three lines beside them (W21).
    Tiles,
}

impl ViewMode {
    /// The fixed icon size of a grid view at 96 DPI, or `None` when the look
    /// decides or the view places items by order.
    pub fn icon_px(self) -> Option<i32> {
        match self {
            Self::Icons => None,
            Self::ExtraLargeIcons => Some(EXTRA_LARGE_ICON_PX),
            Self::LargeIcons => Some(LARGE_ICON_PX),
            Self::MediumIcons => Some(MEDIUM_ICON_PX),
            Self::SmallIcons | Self::List | Self::Details | Self::Tiles => None,
        }
    }

    /// The icon size the view draws at 96 DPI, or `None` when the look decides.
    pub fn item_icon_px(self) -> Option<i32> {
        match self {
            Self::Icons => None,
            Self::ExtraLargeIcons => Some(EXTRA_LARGE_ICON_PX),
            Self::LargeIcons => Some(LARGE_ICON_PX),
            Self::MediumIcons => Some(MEDIUM_ICON_PX),
            Self::SmallIcons | Self::List | Self::Details => Some(SMALL_ICON_PX),
            Self::Tiles => Some(TILE_ICON_PX),
        }
    }

    /// Whether the view places items by their order instead of on the icon
    /// grid, so it never writes `Item.position` (D15, W16).
    pub fn places_by_order(self) -> bool {
        matches!(
            self,
            Self::SmallIcons | Self::List | Self::Details | Self::Tiles
        )
    }
}

/// Explorer's fixed icon sizes at 96 DPI (D15). `SmallIcons` is the one an
/// inline cell draws; the rest are grid views.
const EXTRA_LARGE_ICON_PX: i32 = 256;
const LARGE_ICON_PX: i32 = 96;
const MEDIUM_ICON_PX: i32 = 48;
pub const SMALL_ICON_PX: i32 = 16;
/// Explorer's Tiles icon at 96 DPI (W21).
pub const TILE_ICON_PX: i32 = 48;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::id::SpaceId;
    use crate::item::SortKey;
    use crate::space::Space;

    #[test]
    fn the_default_view_follows_the_look() {
        assert_eq!(ViewMode::default(), ViewMode::Icons);
        assert_eq!(SpaceView::default().mode, ViewMode::Icons);
        assert_eq!(ViewMode::Icons.icon_px(), None);
        assert_eq!(SpaceView::default().sort, SortRule::default());
    }

    #[test]
    fn the_icon_views_have_fixed_sizes() {
        assert_eq!(ViewMode::ExtraLargeIcons.icon_px(), Some(256));
        assert_eq!(ViewMode::LargeIcons.icon_px(), Some(96));
        assert_eq!(ViewMode::MediumIcons.icon_px(), Some(48));
        assert_eq!(ViewMode::SmallIcons.icon_px(), None);
        assert_eq!(ViewMode::List.icon_px(), None);
        assert_eq!(ViewMode::Details.icon_px(), None);
        assert_eq!(ViewMode::SmallIcons.item_icon_px(), Some(16));
        assert_eq!(ViewMode::List.item_icon_px(), Some(16));
        assert_eq!(ViewMode::Details.item_icon_px(), Some(16));
        assert_eq!(ViewMode::Tiles.item_icon_px(), Some(48));
        assert_eq!(ViewMode::Tiles.icon_px(), None);
        assert_eq!(ViewMode::Icons.item_icon_px(), None);
    }

    #[test]
    fn only_the_inline_views_place_items_by_order() {
        assert!(ViewMode::SmallIcons.places_by_order());
        assert!(ViewMode::List.places_by_order());
        assert!(ViewMode::Details.places_by_order());
        assert!(ViewMode::Tiles.places_by_order());
        for mode in [
            ViewMode::Icons,
            ViewMode::ExtraLargeIcons,
            ViewMode::LargeIcons,
            ViewMode::MediumIcons,
        ] {
            assert!(!mode.places_by_order(), "{mode:?}");
        }
    }

    #[test]
    fn small_icons_survive_a_round_trip() {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(0, 0, 100, 100));
        space.view = SpaceView::with_mode(ViewMode::SmallIcons);
        let text = toml::to_string(&space).unwrap();
        assert!(text.contains("small_icons"));
        let back: Space = toml::from_str(&text).unwrap();
        assert_eq!(back.view().mode, ViewMode::SmallIcons);
    }

    #[test]
    fn list_survives_a_round_trip() {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(0, 0, 100, 100));
        space.view = SpaceView::with_mode(ViewMode::List);
        let text = toml::to_string(&space).unwrap();
        assert!(text.contains("list"));
        let back: Space = toml::from_str(&text).unwrap();
        assert_eq!(back.view().mode, ViewMode::List);
    }

    #[test]
    fn a_view_loads_without_a_mode() {
        let view: SpaceView = toml::from_str("").unwrap();
        assert_eq!(view.mode, ViewMode::Icons);
        assert_eq!(view.sort, SortRule::default());
        assert!(
            !view.keep_sorted,
            "a space does not hold its sort by default"
        );
        assert!(!view.status_bar, "the status bar starts off (W19)");
    }

    #[test]
    fn a_view_loads_without_a_sort_rule() {
        let view: SpaceView = toml::from_str("mode = 'large_icons'").unwrap();
        assert_eq!(view.mode, ViewMode::LargeIcons);
        assert_eq!(
            view.sort,
            SortRule {
                key: SortKey::Name,
                descending: false,
                folders_first: false,
            },
            "an older file's view sorts by name"
        );
    }

    #[test]
    fn a_sort_rule_survives_a_round_trip() {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(0, 0, 100, 100));
        space.view = SpaceView {
            mode: ViewMode::Icons,
            sort: SortRule {
                key: SortKey::Size,
                descending: true,
                folders_first: true,
            },
            ..SpaceView::default()
        };
        let text = toml::to_string(&space).unwrap();
        assert!(
            text.contains("sort"),
            "a rule that is not the default shows"
        );
        let back: Space = toml::from_str(&text).unwrap();
        assert_eq!(back.view().sort.key, SortKey::Size);
        assert!(back.view().sort.descending);
        assert!(back.view().sort.folders_first);
    }

    #[test]
    fn keep_sorted_survives_a_round_trip() {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(0, 0, 100, 100));
        space.view = SpaceView {
            keep_sorted: true,
            ..SpaceView::default()
        };
        let text = toml::to_string(&space).unwrap();
        assert!(text.contains("keep_sorted"), "the choice shows in the file");
        let back: Space = toml::from_str(&text).unwrap();
        assert!(back.view().keep_sorted);
    }

    #[test]
    fn a_status_bar_survives_a_round_trip() {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(0, 0, 100, 100));
        space.view = SpaceView {
            status_bar: true,
            ..SpaceView::default()
        };
        let text = toml::to_string(&space).unwrap();
        assert!(text.contains("status_bar"));
        let back: Space = toml::from_str(&text).unwrap();
        assert!(back.view().status_bar);
    }

    #[test]
    fn a_space_without_a_view_loads_as_icons() {
        let space: Space = toml::from_str(
            r#"
            id = 1
            title = "Work"
            rect = { x = 0, y = 0, width = 100, height = 100 }
            "#,
        )
        .unwrap();
        assert_eq!(space.view().mode, ViewMode::Icons);
    }

    #[test]
    fn a_fixed_view_survives_a_round_trip() {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(0, 0, 100, 100));
        space.view = SpaceView::with_mode(ViewMode::LargeIcons);
        let text = toml::to_string(&space).unwrap();
        let back: Space = toml::from_str(&text).unwrap();
        assert_eq!(back.view().mode, ViewMode::LargeIcons);

        let default = Space::new(SpaceId(2), "Default", Rect::new(0, 0, 100, 100));
        assert!(
            !toml::to_string(&default).unwrap().contains("view"),
            "the default view is left out of the file"
        );
    }
}
