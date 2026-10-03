use deskspace_core::geometry::Rect;
use deskspace_core::theme::{Backdrop, Color, CornerStyle};

use super::IconHandle;

/// Window-level appearance of a space, applied through DWM rather than drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct spaceStyle {
    pub backdrop: Backdrop,
    pub corners: CornerStyle,
    pub dark: bool,
    pub border: bool,
}

/// Everything drawn inside one space window. Coordinates are relative to the
/// window's client area.
#[derive(Debug, Clone, PartialEq)]
pub struct spaceScene {
    pub title: String,
    pub title_height: i32,
    pub tint: Color,
    pub title_fill: Color,
    pub title_text: Color,
    pub item_text: Color,
    pub hover_fill: Color,
    pub selected_fill: Color,
    /// Drawn only where DWM cannot draw the border (desktop-hosted spaces).
    pub border: Color,
    pub corner_radius: i32,
    pub locked: bool,
    pub rolled_up: bool,
    pub items: Vec<ItemVisual>,
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
    pub icon: Option<IconHandle>,
    pub state: ItemState,
}
