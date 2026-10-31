use deskspace_core::geometry::Rect;
use deskspace_core::theme::{Backdrop, Color, EditColors};
use deskspace_core::view::ViewMode;

use crate::edit::TextEdit;

use super::{Icon, ItemMetrics};

/// Window-level appearance of a space. DWM applies what it can (material,
/// native 4 and 8 px corners); the painter draws the rest, the border
/// included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceStyle {
    pub backdrop: Backdrop,
    pub corner_radius: i32,
    pub dark: bool,
    pub border: bool,
    /// The border's thickness in pixels at 96 DPI; the window scales it to
    /// its own DPI.
    pub border_width: i32,
    /// Whether the window has space chrome: a title bar and a border. The
    /// desktop layer sets it false, so it reuses the space window and painter
    /// with neither.
    pub chrome: bool,
    /// The shadow the space casts, `None` for none. The window draws it
    /// only where Windows draws none (ROADMAP D15).
    pub shadow: Option<ShadowStyle>,
}

/// A space's shadow, as its settings give it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowStyle {
    /// How far the blur reaches past the space, in pixels at 96 DPI; the
    /// window scales it to its own DPI.
    pub size: i32,
    /// The shadow's opacity at its darkest, in percent.
    pub opacity: u8,
}

impl SpaceStyle {
    /// Whether DWM can draw these corners and the border itself. It only has
    /// square, 4 px, and 8 px corners, and its corners and border come with
    /// window chrome that would show on clear (invisible) spaces. A chrome-less
    /// window has no frame for DWM to draw.
    pub fn dwm_draws_frame(&self) -> bool {
        self.chrome && self.backdrop != Backdrop::Clear && matches!(self.corner_radius, 0 | 4 | 8)
    }
}

/// A space's title icon: the picture drawn left of the title, at `rect` in
/// window-local pixels. `image` stays `None` while the picture is not ready
/// (for example while a set loads), so the title keeps its room and does not
/// jump when the picture arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleIcon {
    pub rect: Rect,
    pub image: Option<Icon>,
}

/// What a drop on a window lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropInto {
    /// A space's own list of items, which can list a file where it is.
    #[default]
    Space,
    /// A real folder (a lens), which only holds the files moved or copied
    /// into it.
    Folder,
}

/// Everything drawn inside one space window. Coordinates are relative to the
/// window's client area.
#[derive(Debug, Clone, PartialEq)]
pub struct SpaceScene {
    /// Whether to draw space chrome: the title bar and the border. The desktop
    /// layer sets it false, so its items fill the whole window.
    pub chrome: bool,
    pub title: String,
    /// When the title is too long, cut it from the start (`… › Sub`) rather
    /// than the end, so a lens browsed into a subfolder keeps the folder it
    /// shows in view.
    pub title_keeps_end: bool,
    /// The small icon drawn left of the title text, if the space has one. A
    /// lens shows its folder's shell icon; another space shows the glyph its
    /// icon setting resolves to.
    pub title_icon: Option<TitleIcon>,
    /// The Back button's square at the left of the title bar, in the space's
    /// coordinates, while a lens shows a subfolder (D18); `None` otherwise.
    /// The icon and the text start after it.
    pub back_button: Option<Rect>,
    pub title_height: i32,
    pub tint: Color,
    pub title_fill: Color,
    pub title_text: Color,
    pub item_text: Color,
    pub hover_fill: Color,
    pub selected_fill: Color,
    /// The rubber-band selection box's color, opaque; the painter derives the
    /// translucent fill and the border alpha from it.
    pub marquee_color: Color,
    /// Whether the rubber-band selection box draws its border.
    pub show_marquee_border: bool,
    /// The fill of a checked item check box, the Windows accent.
    pub check_fill: Color,
    /// Drawn only where DWM does not draw the border (desktop-hosted spaces
    /// and radii DWM cannot draw); transparent for no border.
    pub border: Color,
    /// A shadow behind text so labels stay legible over a nearly
    /// transparent space, like desktop icon labels. `None` for no shadow.
    pub text_shadow: Option<Color>,
    /// Item label lines (1 or 2).
    pub label_lines: u8,
    /// Item label font size as a percent of the default (100).
    pub label_size: u8,
    pub locked: bool,
    pub folded: bool,
    /// Draws a highlight outline just inside the frame, following the corner
    /// radius, to point out the space selected in the settings window.
    pub highlight: bool,
    /// What a drop on this window lands in, which decides the modifiers and
    /// menu entries the drop target offers.
    pub drop_into: DropInto,
    /// Text drawn centered in the item area in place of items, in the label
    /// color at about 60 % opacity; a lens whose folder is unavailable says
    /// so here. `None` draws nothing.
    pub placeholder: Option<String>,
    /// A short message on a bar along the bottom of the item area, over the
    /// items, in the title bar's colors; a lens that followed its moved
    /// folder says where it went. The app clears it after a few seconds.
    pub notice: Option<String>,
    /// The item with keyboard focus, drawn with a dotted focus rectangle. Only
    /// the current space's scene sets it.
    pub focused_item: Option<u64>,
    /// The Details header and its columns, set only in Details (W18). The
    /// header stays put while the rows scroll beneath it.
    pub details: Option<DetailsView>,
    /// Explorer's status bar along the bottom edge, while the space shows one
    /// (W19).
    pub status: Option<StatusBar>,
    /// While set, the painter draws this text in the title bar instead of
    /// `title`, with the selection highlight and a caret.
    pub editing: Option<TextEdit>,
    /// While set, item `id`'s label is drawn as an editable buffer instead,
    /// with the selection highlight and a caret.
    pub editing_item: Option<(u64, TextEdit)>,
    /// Colors of the rename field drawn while `editing` or `editing_item` is
    /// set, matching Explorer's editable box.
    pub edit: EditColors,
    /// Whether the caret is currently visible, so it can blink while editing.
    pub caret_visible: bool,
    /// Scaled item sizes (icon, label gap, check box) the painter draws with;
    /// the app's hit tests use the same metrics.
    pub metrics: ItemMetrics,
    /// How each item cell places its icon and label (W16). The painter's icon,
    /// label, and rename-box positions and the check-box hit test both read it.
    pub item_layout: ItemLayout,
    /// Items are clipped to the area below the title bar, so the app can
    /// place scrolled items partly outside it.
    pub items: Vec<ItemVisual>,
    /// Draw a check box on every item, as Explorer does when its "item check
    /// boxes" view option is on. Display only.
    pub show_check_boxes: bool,
    /// Show the check boxes always (independent item check boxes) rather than
    /// only on hover and while checked (auto-check-select).
    pub check_boxes_persistent: bool,
    /// The rubber-band selection rectangle, in window-local coordinates, while
    /// the user drags over empty space. Drawn translucent above the items.
    pub marquee: Option<Rect>,
    /// The scroll position indicator, when the content overflows.
    pub scroll_thumb: Option<Rect>,
}

/// How an item cell places its icon and label (W16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemLayout {
    /// The icon grid: the icon centered at the top, with its label below it.
    #[default]
    Grid,
    /// One row of a list: the icon at the left and one label line beside it.
    Inline,
    /// One Details row: the icon at the left, the name beside it, then the
    /// other column texts (W18).
    Details,
    /// One tile: a 48 px icon at the left and three lines beside it (W21).
    Tiles,
    /// One content row: the icon and name at the left, details at the right,
    /// with a separator between rows (W22).
    Content,
}

/// The Details header and its columns (W18). Rects are relative to the content
/// area's top-left corner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailsView {
    /// The header row's height; the rows scroll beneath it.
    pub header_height: i32,
    /// The gap between a column's text and its edges.
    pub cell_pad: i32,
    /// One per column, in display order. Name is first, takes the width left
    /// over, and always shows.
    pub columns: Vec<DetailsColumnVisual>,
    /// The column whose header shows Explorer's sort arrow, and whether it
    /// points up (ascending).
    pub sorted: Option<(usize, bool)>,
    /// Where a dragged header would land: the insertion mark's x, relative to
    /// the content area.
    pub drop_mark: Option<i32>,
}

/// One Details column's header rect, label, and value alignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailsColumnVisual {
    /// The column's x range, relative to the content area, and the header row.
    pub rect: Rect,
    pub label: String,
    pub align: ColumnAlign,
}

/// How a Details column's values sit in their cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnAlign {
    #[default]
    Left,
    Right,
}

/// Explorer's status bar (W19): the item counts on the left and its view
/// buttons on the right.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusBar {
    /// The bar's height and its rect in window coordinates.
    pub height: i32,
    pub rect: Rect,
    /// The counts, in order: items, selection, then the selected size.
    pub segments: Vec<String>,
    pub buttons: Vec<StatusButtonVisual>,
}

/// One view button at the right of the status bar.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusButtonVisual {
    /// The button's rect in window coordinates.
    pub rect: Rect,
    /// The view the button switches to.
    pub mode: ViewMode,
    /// The Segoe Fluent Icons / Segoe MDL2 Assets glyph, from the app's table.
    pub glyph: &'static [u16],
    /// Whether the space is already in this view.
    pub checked: bool,
    pub hovered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemState {
    #[default]
    Normal,
    Hovered,
    Selected,
    /// Being dragged within its space: its slot is drawn faded while the
    /// drag image follows the pointer, as Explorer does.
    Dragged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemVisual {
    pub id: u64,
    pub rect: Rect,
    pub label: String,
    pub icon: Option<Icon>,
    pub state: ItemState,
    /// Whether the pointer is over the item, independent of `state` (an item
    /// can be both selected and hovered). Drives the check box's hover reveal.
    pub hovered: bool,
    /// Whether the item's check box is ticked; only drawn when the scene shows
    /// check boxes.
    pub checked: bool,
    /// Whether the item was cut; drawn ghosted until it is pasted, as Explorer
    /// dims the items awaiting a move.
    pub cut: bool,
    /// An override for the label color (Explorer's compressed/encrypted
    /// colors); `None` uses the scene's item text color.
    pub label_color: Option<Color>,
    /// The text for the view's secondary lines: Details column texts after
    /// Name (W18), a tile's type and size (W21), or a content row's right-hand
    /// texts (W22). Empty outside those views.
    pub cells: Vec<String>,
}
