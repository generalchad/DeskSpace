use deskspace_core::geometry::Rect;
use deskspace_core::theme::Color;

use super::IconHandle;

#[derive(Debug, Clone)]
pub struct Scene {
    pub width: i32,
    pub height: i32,
    pub color_key: Color,
    pub status: Option<String>,
    pub spaces: Vec<spaceVisual>,
}

impl Scene {
    pub fn new(width: i32, height: i32, color_key: Color) -> Self {
        Self {
            width,
            height,
            color_key,
            status: None,
            spaces: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct spaceVisual {
    pub id: u64,
    pub rect: Rect,
    pub title: String,
    pub title_rect: Rect,
    pub content_rect: Rect,
    pub fill: Color,
    pub border: Color,
    pub shadow: Color,
    pub title_fill: Color,
    pub title_text: Color,
    pub item_text: Color,
    pub selected: bool,
    pub rolled_up: bool,
    pub locked: bool,
    pub items: Vec<ItemVisual>,
}

#[derive(Debug, Clone)]
pub struct ItemVisual {
    pub id: u64,
    pub rect: Rect,
    pub label: String,
    pub fill: Color,
    pub border: Color,
    pub icon: Option<IconHandle>,
}
