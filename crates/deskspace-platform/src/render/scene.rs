use deskspace_core::geometry::Rect;
use deskspace_core::theme::{Backdrop, Color};

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
    pub locked: bool,
    pub rolled_up: bool,
    /// Items are clipped to the area below the title bar, so the app can
    /// place scrolled items partly outside it.
    pub items: Vec<ItemVisual>,
    /// The scroll position indicator, when the content overflows.
    pub scroll_thumb: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemState {
    #[default]
    Normal,
    Hovered,
    Selected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemVisual {
    pub id: u64,
    pub rect: Rect,
    pub label: String,
    pub icon: Option<Icon>,
    pub state: ItemState,
}
