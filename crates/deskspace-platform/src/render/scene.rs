use deskspace_core::geometry::Rect;
use deskspace_core::theme::{Backdrop, Color};

use crate::edit::TextEdit;

use super::Icon;

/// Window-level appearance of a space. DWM applies what it can (material,
/// native 4 and 8 px corners, border); the painter draws the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceStyle {
    pub backdrop: Backdrop,
    pub corner_radius: i32,
    pub dark: bool,
    pub border: bool,
}

impl SpaceStyle {
    /// Whether DWM can draw these corners and the border itself. It only has
    /// square, 4 px, and 8 px corners, and its corners and border come with
    /// window chrome that would show on clear (invisible) spaces.
    pub fn dwm_draws_frame(&self) -> bool {
        self.backdrop != Backdrop::Clear && matches!(self.corner_radius, 0 | 4 | 8)
    }
}

/// Everything drawn inside one space window. Coordinates are relative to the
/// window's client area.
#[derive(Debug, Clone, PartialEq)]
pub struct SpaceScene {
    pub title: String,
    pub title_height: i32,
    pub tint: Color,
    pub title_fill: Color,
    pub title_text: Color,
    pub item_text: Color,
    pub hover_fill: Color,
    pub selected_fill: Color,
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
    pub rolled_up: bool,
    /// While set, the painter draws this text in the title bar instead of
    /// `title`, with the selection highlight and a caret.
    pub editing: Option<TextEdit>,
    /// While set, item `id`'s label is drawn as an editable buffer instead,
    /// with the selection highlight and a caret.
    pub editing_item: Option<(u64, TextEdit)>,
    /// Whether the caret is currently visible, so it can blink while editing.
    pub caret_visible: bool,
    /// Items are clipped to the area below the title bar, so the app can
    /// place scrolled items partly outside it.
    pub items: Vec<ItemVisual>,
    /// Draw a check box on every item, as Explorer does when its "item check
    /// boxes" view option is on. Display only.
    pub show_check_boxes: bool,
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
}
