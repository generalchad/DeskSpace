use deskspace_core::geometry::Rect;
use deskspace_core::theme::{Backdrop, Color, EditColors};

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

/// Everything drawn inside one space window. Coordinates are relative to the
/// window's client area.
#[derive(Debug, Clone, PartialEq)]
pub struct SpaceScene {
    /// Whether to draw space chrome: the title bar and the border. The desktop
    /// layer sets it false, so its items fill the whole window.
    pub chrome: bool,
    pub title: String,
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
    /// The item with keyboard focus, drawn with a dotted focus rectangle. Only
    /// the current space's scene sets it.
    pub focused_item: Option<u64>,
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
}
