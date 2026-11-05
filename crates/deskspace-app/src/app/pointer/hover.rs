//! What the pointer is over: its hover, its cursor shape, and its info tip.

use deskspace_core::geometry::Point;
use deskspace_core::{ItemId, ResizeEdges, SpaceId};
use deskspace_platform::{CursorShape, Ui};

use crate::app::Timer;

use super::{App, PointerState, RESIZE_BORDER, Surface};

/// How long the pointer must rest on an item before its info tip appears,
/// matching Explorer's tooltip delay.
const INFO_TIP_DELAY_MS: u32 = 600;

impl PointerState {
    /// The item hovered on `surface`, when that surface owns the hover.
    pub(in crate::app) fn hovered_item_on(&self, surface: Surface) -> Option<ItemId> {
        self.hovered
            .filter(|(owner, _)| *owner == surface)
            .map(|(_, item)| item)
    }

    /// The hovered item and its surface, if any.
    pub(in crate::app) fn hovered(&self) -> Option<(Surface, ItemId)> {
        self.hovered
    }

    /// Forgets the hover, as when it is dismissed outright.
    pub(in crate::app) fn clear_hover(&mut self) {
        self.hovered = None;
    }

    /// Forgets the hover when it is on `surface`.
    pub(in crate::app) fn clear_hover_on(&mut self, surface: Surface) {
        if self.hovered.is_some_and(|(owner, _)| owner == surface) {
            self.hovered = None;
        }
    }

    /// Forgets the hover when it is `item`.
    pub(in crate::app) fn clear_hover_for_item(&mut self, item: ItemId) {
        if self.hovered.is_some_and(|(_, hovered)| hovered == item) {
            self.hovered = None;
        }
    }
}

impl App {
    pub(super) fn update_cursor(&self, ui: &mut Ui, id: SpaceId, point: Point) {
        if self.title_icon_button_at(id, point) {
            ui.set_space_cursor(id.0, CursorShape::Hand);
            return;
        }
        let edges = self
            .config
            .desktop
            .space(id)
            .map_or(ResizeEdges::NONE, |space| {
                space.resize_edges_at(point, RESIZE_BORDER)
            });
        let cursor = if !edges.any() && self.details_divider_at(id, point).is_some() {
            CursorShape::SizeWestEast
        } else {
            cursor_for(edges)
        };
        ui.set_space_cursor(id.0, cursor);
    }

    pub(super) fn update_hover(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let hovered = self
            .surface_item_at(surface, point)
            .map(|item| (surface, item));
        let status_hover = match (surface, hovered) {
            (Surface::Space(id), None) => self
                .config
                .desktop
                .space(id)
                .and_then(|space| crate::scene::status_button_at(space, point))
                .map(|index| (surface, index)),
            _ => None,
        };
        // A lens's title bar tips the folder it displays (W27). While that
        // space is being renamed the tip would cover the edit, so it is not
        // armed then.
        let title_hover = match (surface, hovered, status_hover) {
            (Surface::Space(id), None, None) => self
                .config
                .desktop
                .space(id)
                .filter(|space| space.is_lens() && space.title_bar().contains(point))
                .filter(|_| self.edit.surface() != Some(surface))
                .map(|_| id),
            _ => None,
        };
        let icon_hover = match surface {
            Surface::Space(id) if self.title_icon_button_at(id, point) => Some(id),
            _ => None,
        };
        if hovered == self.pointer.hovered
            && status_hover == self.pointer.status_hover
            && title_hover == self.pointer.title_hover
            && icon_hover == self.pointer.title_icon_hover
        {
            return;
        }
        let previous = std::mem::replace(&mut self.pointer.hovered, hovered);
        let previous_status = std::mem::replace(&mut self.pointer.status_hover, status_hover);
        let previous_title = std::mem::replace(&mut self.pointer.title_hover, title_hover);
        self.pointer.title_icon_hover = icon_hover;
        // A different item (or none) cancels any shown tip and its delay.
        ui.hide_info_tip();
        Self::clear_timer(ui, Timer::InfoTip);
        if hovered.is_some() || status_hover.is_some() || title_hover.is_some() {
            Self::set_timer(ui, Timer::InfoTip, INFO_TIP_DELAY_MS);
        }
        if let Some((owner, _)) = previous.filter(|(owner, _)| *owner != surface) {
            self.refresh_surface(ui, owner);
        }
        if let Some((owner, _)) = previous_status.filter(|(owner, _)| *owner != surface) {
            self.refresh_surface(ui, owner);
        }
        if let Some(owner) = previous_title.filter(|owner| Surface::of(*owner) != surface) {
            self.refresh_surface(ui, Surface::of(owner));
        }
        self.refresh_surface(ui, surface);
    }

    pub(super) fn on_pointer_leave(&mut self, ui: &mut Ui, surface: Surface) {
        self.dismiss_info_tip(ui);
        let mut repaint = false;
        if self
            .pointer
            .hovered
            .is_some_and(|(owner, _)| owner == surface)
        {
            self.pointer.hovered = None;
            repaint = true;
        }
        if self
            .pointer
            .status_hover
            .is_some_and(|(owner, _)| owner == surface)
        {
            self.pointer.status_hover = None;
            repaint = true;
        }
        if self
            .pointer
            .title_hover
            .is_some_and(|owner| Surface::of(owner) == surface)
        {
            self.pointer.title_hover = None;
            repaint = true;
        }
        if repaint {
            self.refresh_surface(ui, surface);
        }
    }
}

fn cursor_for(edges: ResizeEdges) -> CursorShape {
    match (edges.left, edges.top, edges.right, edges.bottom) {
        (true, true, _, _) | (_, _, true, true) => CursorShape::SizeNorthWestSouthEast,
        (true, _, _, true) | (_, true, true, _) => CursorShape::SizeNorthEastSouthWest,
        (true, ..) | (_, _, true, _) => CursorShape::SizeWestEast,
        (_, true, ..) | (.., true) => CursorShape::SizeNorthSouth,
        _ => CursorShape::Arrow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_cursor_matches_grabbed_edges() {
        let edges = |left, top, right, bottom| ResizeEdges {
            left,
            top,
            right,
            bottom,
        };
        assert_eq!(cursor_for(ResizeEdges::NONE), CursorShape::Arrow);
        assert_eq!(
            cursor_for(edges(true, false, false, false)),
            CursorShape::SizeWestEast
        );
        assert_eq!(
            cursor_for(edges(false, false, false, true)),
            CursorShape::SizeNorthSouth
        );
        assert_eq!(
            cursor_for(edges(false, false, true, true)),
            CursorShape::SizeNorthWestSouthEast
        );
        assert_eq!(
            cursor_for(edges(false, true, true, false)),
            CursorShape::SizeNorthEastSouthWest
        );
    }
}
