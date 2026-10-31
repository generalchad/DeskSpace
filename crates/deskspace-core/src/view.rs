//! A space's view: how its items are laid out and ordered, apart from its
//! look (D17).

use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer, Serialize};

use crate::item::SortKey;
use crate::sort::SortRule;

/// Whether a space's labels show filename extensions (D18, W28).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Extensions {
    /// Today's rule: Explorer's own option on a space that mirrors
    /// Explorer's display, else hidden.
    #[default]
    Automatic,
    /// Always show them, whatever Explorer does.
    Show,
    /// Never show them, whatever Explorer does.
    Hide,
}

impl Extensions {
    /// Whether this is the default, which old files leave out.
    fn is_default(&self) -> bool {
        *self == Self::Automatic
    }
}

/// How a space groups its items, as Explorer's Group by does (D18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupBy {
    pub key: SortKey,
    #[serde(default)]
    pub descending: bool,
}

/// Reads a hand-editable grouping so that no value in it can fail the config
/// load: one that makes no sense means no grouping.
fn lenient_group<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<GroupBy>, D::Error> {
    Ok(match Lenient::<GroupBy>::deserialize(deserializer)? {
        Lenient::Valid(group) => Some(group),
        Lenient::Invalid(_) => None,
    })
}

/// A Details column (D18). Its spelling is part of a space's config, so a
/// variant is never renamed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Column {
    Name,
    Modified,
    Kind,
    Size,
    Created,
    /// A column this version does not know, from a newer or hand-edited file.
    /// `ColumnSpec::sanitize` drops it.
    #[serde(other)]
    Unknown,
}

impl Column {
    /// Every column a space can show.
    pub const ALL: [Self; 5] = [
        Self::Name,
        Self::Modified,
        Self::Kind,
        Self::Size,
        Self::Created,
    ];
}

/// The widths a column can be given, in pixels at 96 DPI.
pub const COLUMN_WIDTH_RANGE: (u16, u16) = (40, 2000);

/// One Details column a space shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub id: Column,
    /// Its width at 96 DPI, or `None` for the column's default. Name's is
    /// `None`, so it takes the width the others leave, except while a divider
    /// drag holds it still; a file never brings one back.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "saturating_width"
    )]
    pub width: Option<u16>,
}

impl ColumnSpec {
    const fn new(id: Column) -> Self {
        Self { id, width: None }
    }

    /// `list` made safe to show: unknown and repeated ids dropped, Name first
    /// and always there, at most one entry per column, and widths in
    /// `COLUMN_WIDTH_RANGE`. Runs on load and after every edit, so every
    /// reader can trust a space's columns.
    pub fn sanitize(list: Vec<Self>) -> Vec<Self> {
        let mut kept = vec![Self::new(Column::Name)];
        let mut named = false;
        for spec in list {
            let spec = Self {
                width: spec
                    .width
                    .map(|width| width.clamp(COLUMN_WIDTH_RANGE.0, COLUMN_WIDTH_RANGE.1)),
                ..spec
            };
            if spec.id == Column::Name {
                if !named {
                    kept[0] = spec;
                    named = true;
                }
            } else if spec.id != Column::Unknown && !kept.iter().any(|other| other.id == spec.id) {
                kept.push(spec);
            }
        }
        kept
    }
}

/// What a new space shows, as a new Explorer folder does: Date created waits
/// until the user asks for it.
fn default_columns() -> Vec<ColumnSpec> {
    [Column::Name, Column::Modified, Column::Kind, Column::Size]
        .map(ColumnSpec::new)
        .into()
}

fn columns_are_default(columns: &[ColumnSpec]) -> bool {
    columns == default_columns()
}

/// A value of `T`, or whatever else the file holds there.
#[derive(Deserialize)]
#[serde(untagged)]
enum Lenient<T> {
    Valid(T),
    Invalid(IgnoredAny),
}

/// Reads a hand-editable column list so that no value in it can fail the
/// config load: an entry that makes no sense is dropped, and a field that is
/// no list at all means the default columns.
fn lenient_columns<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<ColumnSpec>, D::Error> {
    let Lenient::Valid(list) = Lenient::<Vec<Lenient<ColumnSpec>>>::deserialize(deserializer)?
    else {
        return Ok(default_columns());
    };
    let specs = list.into_iter().filter_map(|entry| match entry {
        Lenient::Valid(spec) => Some(spec),
        Lenient::Invalid(_) => None,
    });
    let mut columns = ColumnSpec::sanitize(specs.collect());
    columns[0].width = None;
    Ok(columns)
}

/// Reads any whole number, so a huge or negative width is clamped by
/// `sanitize` rather than failing the load.
fn saturating_width<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u16>, D::Error> {
    let width = Option::<i64>::deserialize(deserializer)?;
    Ok(width.map(|width| width.clamp(0, i64::from(u16::MAX)) as u16))
}

/// A space's view settings. Every space has one; a lens is not special.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpaceView {
    /// The layout the space uses.
    pub mode: ViewMode,
    /// How the space orders its items. A lens is always kept in this order;
    /// a normal space keeps the rule and sorts when asked.
    #[serde(skip_serializing_if = "SortRule::is_default")]
    pub sort: SortRule,
    /// Whether the space re-sorts its items whenever they change, holding the
    /// rule. A lens always does; a normal space only when this is set (D17).
    pub keep_sorted: bool,
    /// Whether the space shows Explorer's status bar along its bottom edge
    /// with the item counts and view buttons (W19).
    pub status_bar: bool,
    /// Whether the space's labels show filename extensions (W28).
    #[serde(skip_serializing_if = "Extensions::is_default")]
    pub extensions: Extensions,
    /// Whether the views that place items by order use Explorer's tighter
    /// rows. The icon views keep the Layout page's spacing (D18).
    pub compact: bool,
    /// How the space groups its items, if it does.
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_group"
    )]
    pub group: Option<GroupBy>,
    /// The Details columns the space shows, in display order (D18). Always
    /// sanitized: `ColumnSpec::sanitize` runs on load and after every edit.
    #[serde(
        skip_serializing_if = "columns_are_default",
        deserialize_with = "lenient_columns"
    )]
    pub columns: Vec<ColumnSpec>,
}

impl Default for SpaceView {
    fn default() -> Self {
        Self {
            mode: ViewMode::default(),
            sort: SortRule::default(),
            keep_sorted: false,
            status_bar: false,
            extensions: Extensions::default(),
            compact: false,
            group: None,
            columns: default_columns(),
        }
    }
}

impl SpaceView {
    /// Whether items are placed by their order, not on the icon grid, so
    /// `Item.position` is left alone: an order-placed view, or any grouped one,
    /// since groups have no place on a grid (D18).
    pub fn places_by_order(&self) -> bool {
        self.mode.places_by_order() || self.group.is_some()
    }

    /// This view shown as `mode`, as the View menu and its keys change it. The
    /// sort rule, Keep sorted, and the status bar stay, since a space keeps
    /// its last rule across views (D17).
    pub fn with_mode(&self, mode: ViewMode) -> Self {
        Self {
            mode,
            ..self.clone()
        }
    }

    /// This view with `column` shown if it was hidden, or hidden if it was
    /// shown. A column comes back at the end, at its default width, and Name
    /// stays (D18).
    pub fn with_column_toggled(&self, column: Column) -> Self {
        if column == Column::Name {
            return self.clone();
        }
        let mut columns = self.columns.clone();
        match columns.iter().position(|spec| spec.id == column) {
            Some(index) => {
                columns.remove(index);
            }
            None => columns.push(ColumnSpec::new(column)),
        }
        Self {
            columns: ColumnSpec::sanitize(columns),
            ..self.clone()
        }
    }

    /// This view with `column` moved to before the column now at `insert_at`,
    /// or to the end past the last. Name stays first and does not move (D18).
    pub fn with_column_moved(&self, column: Column, insert_at: usize) -> Self {
        let mut columns = self.columns.clone();
        let Some(from) = columns.iter().position(|spec| spec.id == column) else {
            return self.clone();
        };
        if column == Column::Name {
            return self.clone();
        }
        let spec = columns.remove(from);
        let to = if insert_at > from {
            insert_at - 1
        } else {
            insert_at
        };
        columns.insert(to.min(columns.len()), spec);
        Self {
            columns: ColumnSpec::sanitize(columns),
            ..self.clone()
        }
    }

    /// This view with `column` at `width` pixels at 96 DPI, in range.
    pub fn with_column_width(&self, column: Column, width: u16) -> Self {
        let columns = self
            .columns
            .iter()
            .map(|spec| ColumnSpec {
                width: if spec.id == column {
                    Some(width)
                } else {
                    spec.width
                },
                ..*spec
            })
            .collect();
        Self {
            columns: ColumnSpec::sanitize(columns),
            ..self.clone()
        }
    }

    /// This view with Name held at `width` at 96 DPI, or taking the width the
    /// others leave when `None`. Not clamped, so holding Name never moves it.
    pub fn with_name_held(&self, width: Option<u16>) -> Self {
        let mut view = self.clone();
        view.columns[0].width = width;
        view
    }

    /// Whether this is the default view, which old files leave out.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Explorer's views (D17). `Icons` follows the look's icon size; the fixed icon
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
    /// One row per item with a separator and its details on the right (W22).
    Content,
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
            Self::SmallIcons | Self::List | Self::Details | Self::Tiles | Self::Content => None,
        }
    }

    /// The icon size the view draws at 96 DPI, or `None` when the look decides.
    pub fn item_icon_px(self) -> Option<i32> {
        match self {
            Self::Icons => None,
            Self::ExtraLargeIcons => Some(EXTRA_LARGE_ICON_PX),
            Self::LargeIcons => Some(LARGE_ICON_PX),
            Self::MediumIcons => Some(MEDIUM_ICON_PX),
            Self::SmallIcons | Self::List | Self::Details | Self::Content => Some(SMALL_ICON_PX),
            Self::Tiles => Some(TILE_ICON_PX),
        }
    }

    /// Whether the view places items by their order instead of on the icon
    /// grid, so it never writes `Item.position` (D17, W16).
    pub fn places_by_order(self) -> bool {
        matches!(
            self,
            Self::SmallIcons | Self::List | Self::Details | Self::Tiles | Self::Content
        )
    }

    /// Whether every row spans the space's width, so a resize has no columns
    /// to line up and only its rows matter (D22).
    pub fn rows_fill_width(self) -> bool {
        matches!(self, Self::Details | Self::Content)
    }
}

/// Explorer's fixed icon sizes at 96 DPI (D17). `SmallIcons` is the one an
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
    use crate::space::Space;

    #[test]
    fn the_default_view_follows_the_look() {
        assert_eq!(ViewMode::default(), ViewMode::Icons);
        assert_eq!(SpaceView::default().mode, ViewMode::Icons);
        assert_eq!(ViewMode::Icons.icon_px(), None);
        assert_eq!(SpaceView::default().sort, SortRule::default());
    }

    #[test]
    fn an_extension_choice_round_trips_and_the_default_is_left_out() {
        let default = toml::to_string(&SpaceView::default()).unwrap();
        assert!(!default.contains("extensions"), "{default}");
        assert_eq!(
            toml::from_str::<SpaceView>("").unwrap(),
            SpaceView::default(),
            "a missing field loads as Automatic"
        );
        let shown = SpaceView {
            extensions: Extensions::Show,
            ..SpaceView::default()
        };
        assert_eq!(
            toml::from_str::<SpaceView>(&toml::to_string(&shown).unwrap()).unwrap(),
            shown
        );
    }

    fn ids(columns: &[ColumnSpec]) -> Vec<Column> {
        columns.iter().map(|spec| spec.id).collect()
    }

    fn spec(id: Column, width: u16) -> ColumnSpec {
        ColumnSpec {
            id,
            width: Some(width),
        }
    }

    #[test]
    fn a_new_view_shows_explorers_columns_and_leaves_them_out_of_the_file() {
        let view = SpaceView::default();
        assert_eq!(
            ids(&view.columns),
            [Column::Name, Column::Modified, Column::Kind, Column::Size]
        );
        let text = toml::to_string(&view).unwrap();
        assert!(!text.contains("columns"), "{text}");
        assert_eq!(toml::from_str::<SpaceView>("").unwrap(), view);
    }

    #[test]
    fn chosen_columns_survive_a_round_trip() {
        let view = SpaceView {
            columns: vec![
                ColumnSpec::new(Column::Name),
                spec(Column::Created, 200),
                ColumnSpec::new(Column::Size),
            ],
            ..SpaceView::default()
        };
        let text = toml::to_string(&view).unwrap();
        assert_eq!(toml::from_str::<SpaceView>(&text).unwrap(), view, "{text}");
    }

    #[test]
    fn chosen_columns_survive_a_round_trip_inside_a_space() {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(0, 0, 100, 100));
        space.view = SpaceView::default().with_column_toggled(Column::Created);
        let text = toml::to_string(&space).unwrap();
        let back: Space = toml::from_str(&text).unwrap();
        assert_eq!(*back.view(), space.view, "{text}");
    }

    #[test]
    fn sanitize_keeps_name_first_whatever_the_list_says() {
        for list in [
            vec![],
            vec![ColumnSpec::new(Column::Size)],
            vec![ColumnSpec::new(Column::Size), ColumnSpec::new(Column::Name)],
        ] {
            assert_eq!(ColumnSpec::sanitize(list)[0].id, Column::Name);
        }
        assert_eq!(
            ids(&ColumnSpec::sanitize(vec![
                ColumnSpec::new(Column::Size),
                ColumnSpec::new(Column::Name),
                ColumnSpec::new(Column::Kind),
            ])),
            [Column::Name, Column::Size, Column::Kind],
            "the others keep their order"
        );
    }

    #[test]
    fn sanitize_drops_unknown_and_repeated_columns() {
        assert_eq!(
            ColumnSpec::sanitize(vec![
                ColumnSpec::new(Column::Unknown),
                spec(Column::Size, 90),
                spec(Column::Size, 300),
                ColumnSpec::new(Column::Unknown),
            ]),
            [ColumnSpec::new(Column::Name), spec(Column::Size, 90)],
            "the first of a repeated column wins"
        );
        assert_eq!(
            ColumnSpec::sanitize(vec![spec(Column::Name, 90), spec(Column::Name, 300)]),
            [spec(Column::Name, 90)],
            "Name's first entry wins too"
        );
    }

    #[test]
    fn sanitize_clamps_widths_and_leaves_a_default_alone() {
        let (low, high) = COLUMN_WIDTH_RANGE;
        assert_eq!(
            ColumnSpec::sanitize(vec![
                spec(Column::Size, 0),
                spec(Column::Kind, u16::MAX),
                ColumnSpec::new(Column::Created),
            ]),
            [
                ColumnSpec::new(Column::Name),
                spec(Column::Size, low),
                spec(Column::Kind, high),
                ColumnSpec::new(Column::Created),
            ]
        );
    }

    #[test]
    fn sanitize_caps_a_thousand_entries_at_one_per_column() {
        let list: Vec<ColumnSpec> = Column::ALL
            .iter()
            .cycle()
            .take(1000)
            .map(|id| ColumnSpec::new(*id))
            .collect();
        assert_eq!(ids(&ColumnSpec::sanitize(list)), Column::ALL);
    }

    /// The columns a config's hand-edited text loads as.
    fn loaded(text: &str) -> Vec<ColumnSpec> {
        toml::from_str::<SpaceView>(text).unwrap().columns
    }

    #[test]
    fn a_hostile_column_list_never_fails_the_load() {
        let name = ColumnSpec::new(Column::Name);
        assert_eq!(
            loaded("columns = 'wide'"),
            SpaceView::default().columns,
            "no list at all means the default"
        );
        assert_eq!(loaded("columns = []"), [name], "Name is always shown");
        assert_eq!(
            loaded("columns = [1, 'x', { id = 7 }, { width = 3 }, { id = 'size', width = 'y' }]"),
            [name],
            "an entry that makes no sense is dropped"
        );
        assert_eq!(
            loaded("columns = [{ id = 'future' }, { id = 'kind' }]"),
            [name, ColumnSpec::new(Column::Kind)],
            "a column this version does not know is dropped"
        );
        let (low, high) = COLUMN_WIDTH_RANGE;
        assert_eq!(
            loaded("columns = [{ id = 'size', width = -5 }, { id = 'kind', width = 99999999 }]"),
            [name, spec(Column::Size, low), spec(Column::Kind, high)],
        );
    }

    #[test]
    fn moving_a_column_inserts_it_before_the_one_at_that_place() {
        let view = SpaceView::default();
        let moved = |column, insert_at| ids(&view.with_column_moved(column, insert_at).columns);
        let [name, date, kind, size] = [Column::Name, Column::Modified, Column::Kind, Column::Size];
        assert_eq!(moved(size, 1), [name, size, date, kind], "leftward");
        assert_eq!(moved(date, 3), [name, kind, date, size], "rightward");
        assert_eq!(moved(date, 4), [name, kind, size, date], "to the end");
        assert_eq!(moved(date, 99), [name, kind, size, date], "past the end");
        assert_eq!(moved(kind, 2), [name, date, kind, size], "before itself");
        assert_eq!(moved(kind, 3), [name, date, kind, size], "after itself");
    }

    #[test]
    fn name_never_moves_and_nothing_goes_before_it() {
        let view = SpaceView::default();
        assert_eq!(view.with_column_moved(Column::Name, 3), view);
        assert_eq!(
            ids(&view.with_column_moved(Column::Size, 0).columns),
            [Column::Name, Column::Size, Column::Modified, Column::Kind]
        );
    }

    #[test]
    fn moving_a_column_keeps_its_width_and_ignores_one_not_shown() {
        let view = SpaceView::default().with_column_width(Column::Size, 150);
        let moved = view.with_column_moved(Column::Size, 1);
        assert_eq!(moved.columns[1], spec(Column::Size, 150));
        assert_eq!(view.with_column_moved(Column::Created, 1), view);
    }

    #[test]
    fn a_file_never_brings_back_a_width_for_name() {
        assert_eq!(
            loaded("columns = [{ id = 'name', width = 300 }, { id = 'size', width = 90 }]"),
            [ColumnSpec::new(Column::Name), spec(Column::Size, 90)],
        );
    }

    #[test]
    fn setting_a_width_clamps_it_and_changes_only_that_column() {
        let (low, high) = COLUMN_WIDTH_RANGE;
        let view = SpaceView::default();
        let size = |view: &SpaceView| view.columns[3];
        assert_eq!(
            size(&view.with_column_width(Column::Size, 150)),
            spec(Column::Size, 150)
        );
        assert_eq!(
            size(&view.with_column_width(Column::Size, 0)),
            spec(Column::Size, low)
        );
        assert_eq!(
            size(&view.with_column_width(Column::Size, u16::MAX)),
            spec(Column::Size, high)
        );
        assert_eq!(
            view.with_column_width(Column::Size, 150).columns[1],
            view.columns[1]
        );
    }

    #[test]
    fn holding_name_is_not_clamped_and_letting_go_restores_it() {
        let view = SpaceView::default();
        let held = view.with_name_held(Some(3000));
        assert_eq!(held.columns[0], spec(Column::Name, 3000));
        assert_eq!(held.with_name_held(None), view);
    }

    #[test]
    fn toggling_a_column_hides_it_and_brings_it_back_last() {
        let view = SpaceView::default();
        let hidden = view.with_column_toggled(Column::Modified);
        assert_eq!(
            ids(&hidden.columns),
            [Column::Name, Column::Kind, Column::Size]
        );
        assert_eq!(
            ids(&hidden.with_column_toggled(Column::Modified).columns),
            [Column::Name, Column::Kind, Column::Size, Column::Modified]
        );
    }

    #[test]
    fn toggling_name_changes_nothing() {
        let view = SpaceView {
            columns: vec![spec(Column::Name, 300), ColumnSpec::new(Column::Size)],
            ..SpaceView::default()
        };
        assert_eq!(
            ids(&view.with_column_toggled(Column::Name).columns),
            [Column::Name, Column::Size]
        );
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
        assert_eq!(ViewMode::Content.item_icon_px(), Some(16));
        assert_eq!(ViewMode::Tiles.icon_px(), None);
        assert_eq!(ViewMode::Content.icon_px(), None);
        assert_eq!(ViewMode::Icons.item_icon_px(), None);
    }

    #[test]
    fn only_the_inline_views_place_items_by_order() {
        assert!(ViewMode::SmallIcons.places_by_order());
        assert!(ViewMode::List.places_by_order());
        assert!(ViewMode::Details.places_by_order());
        assert!(ViewMode::Tiles.places_by_order());
        assert!(ViewMode::Content.places_by_order());
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
    fn only_the_row_views_fill_their_width() {
        assert!(ViewMode::Details.rows_fill_width());
        assert!(ViewMode::Content.rows_fill_width());
        for mode in [
            ViewMode::Icons,
            ViewMode::ExtraLargeIcons,
            ViewMode::LargeIcons,
            ViewMode::MediumIcons,
            ViewMode::SmallIcons,
            ViewMode::List,
            ViewMode::Tiles,
        ] {
            assert!(!mode.rows_fill_width(), "{mode:?}");
        }
    }

    #[test]
    fn a_mode_change_keeps_the_rest_of_the_view() {
        let view = SpaceView {
            mode: ViewMode::Icons,
            sort: SortRule {
                key: SortKey::Modified,
                descending: true,
                folders_first: true,
            },
            keep_sorted: false,
            status_bar: true,
            extensions: Extensions::Hide,
            compact: true,
            group: Some(GroupBy {
                key: SortKey::Kind,
                descending: true,
            }),
            columns: vec![ColumnSpec::new(Column::Name)],
        };
        let next = view.with_mode(ViewMode::Details);
        assert_eq!(next.mode, ViewMode::Details);
        assert_eq!(
            SpaceView {
                mode: view.mode,
                ..next
            },
            view,
            "only the mode changes"
        );
    }

    #[test]
    fn small_icons_survive_a_round_trip() {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(0, 0, 100, 100));
        space.view = SpaceView::default().with_mode(ViewMode::SmallIcons);
        let text = toml::to_string(&space).unwrap();
        assert!(text.contains("small_icons"));
        let back: Space = toml::from_str(&text).unwrap();
        assert_eq!(back.view().mode, ViewMode::SmallIcons);
    }

    #[test]
    fn list_survives_a_round_trip() {
        let mut space = Space::new(SpaceId(1), "Work", Rect::new(0, 0, 100, 100));
        space.view = SpaceView::default().with_mode(ViewMode::List);
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
    fn a_grouping_round_trips_and_is_left_out_when_off() {
        assert!(
            !toml::to_string(&SpaceView::default())
                .unwrap()
                .contains("group")
        );
        let view = SpaceView {
            group: Some(GroupBy {
                key: SortKey::Size,
                descending: true,
            }),
            ..SpaceView::default()
        };
        let text = toml::to_string(&view).unwrap();
        assert_eq!(toml::from_str::<SpaceView>(&text).unwrap(), view, "{text}");
    }

    #[test]
    fn a_hostile_grouping_never_fails_the_load() {
        for text in [
            "group = 'wide'",
            "group = 3",
            "group = { key = 'future' }",
            "group = { descending = true }",
            "group = { key = 'size', descending = 'y' }",
        ] {
            assert_eq!(
                toml::from_str::<SpaceView>(text).unwrap().group,
                None,
                "{text}"
            );
        }
        assert_eq!(
            toml::from_str::<SpaceView>("group = { key = 'size' }")
                .unwrap()
                .group,
            Some(GroupBy {
                key: SortKey::Size,
                descending: false,
            })
        );
    }

    #[test]
    fn a_grouped_icon_view_places_by_order() {
        let view = SpaceView::default();
        assert!(!view.places_by_order());
        let grouped = SpaceView {
            group: Some(GroupBy {
                key: SortKey::Name,
                descending: false,
            }),
            ..view
        };
        assert!(grouped.places_by_order());
    }

    #[test]
    fn compact_starts_off_and_survives_a_round_trip() {
        assert!(!toml::from_str::<SpaceView>("").unwrap().compact);
        let view = SpaceView {
            compact: true,
            ..SpaceView::default()
        };
        let text = toml::to_string(&view).unwrap();
        assert_eq!(toml::from_str::<SpaceView>(&text).unwrap(), view, "{text}");
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
        space.view = SpaceView::default().with_mode(ViewMode::LargeIcons);
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
