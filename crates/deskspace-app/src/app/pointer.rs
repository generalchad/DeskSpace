//! `App` pointer: space events, selection gestures, and drags (R1).

use std::path::PathBuf;

use deskspace_core::geometry::{Point, Rect, Size};
use deskspace_core::theme::ICON_SIZE_RANGE;
use deskspace_core::{Command, ItemId, ResizeEdges, Space, SpaceId};

use deskspace_platform::{
    CheckBoxMode, CursorShape, DragImage, MouseButton, SpaceEvent, Ui, drag_threshold,
    virtual_desktop_rect,
};

use crate::scene::{desktop_layer_drag_scene, drag_scene};

use super::selection::marquee_selection;

use super::{
    App, Drag, ICON_SIZE_WHEEL_STEP, INFO_TIP_DELAY_MS, INFO_TIP_TIMER, PendingClickOutcome,
    RESIZE_BORDER, Surface, WHEEL_DELTA, pending_click, resolve_pending_click,
};

impl App {
    pub(super) fn on_space_event(&mut self, ui: &mut Ui, id: SpaceId, event: SpaceEvent) {
        // The chrome-less desktop layer reuses the space window: id 0 is it,
        // any other id a real space. A space window can outlive its space by
        // one queued event, so ignore anything they report.
        let surface = Surface::of(id);
        let exists = match surface {
            Surface::Space(id) => self.config.desktop.space(id).is_some(),
            Surface::Desktop => self.layer_active(),
        };
        if !exists {
            return;
        }
        // Clicking anywhere commits a rename in progress before the click is
        // handled, as clicking away from a title does elsewhere.
        if self.edit.is_some()
            && matches!(
                &event,
                SpaceEvent::MouseDown { .. } | SpaceEvent::DoubleClick { .. }
            )
        {
            self.finish_rename(ui, true);
        }
        // A space is part of the desktop, and never takes activation, so a
        // click here is made to behave like one on the desktop: focus it and
        // clear its icon selection.
        if matches!(&event, SpaceEvent::MouseDown { .. }) {
            self.dismiss_info_tip(ui);
            self.refresh_shell_display(ui);
            ui.on_desktop_click();
        }
        match event {
            SpaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ctrl,
                shift,
            } => self.begin_interaction(ui, surface, Point::new(x, y), ctrl, shift),
            SpaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Right,
                ..
            } => self.select_at(ui, surface, Point::new(x, y)),
            SpaceEvent::MouseMove { x, y } => {
                let point = Point::new(x, y);
                if !self.drag_to(ui, point) {
                    if let Surface::Space(id) = surface {
                        self.update_cursor(ui, id, point);
                    }
                    self.update_hover(ui, surface, point);
                }
            }
            SpaceEvent::Wheel { x, y, delta, ctrl } => {
                if ctrl {
                    self.adjust_icon_size(ui, delta);
                } else if let Surface::Space(id) = surface {
                    self.scroll_by(ui, id, Point::new(x, y), delta);
                }
            }
            SpaceEvent::MouseUp {
                button: MouseButton::Left,
                ..
            }
            | SpaceEvent::CaptureLost => self.end_drag(ui),
            SpaceEvent::MouseUp {
                x,
                y,
                button: MouseButton::Right,
            } => {
                let point = Point::new(x, y);
                match self.surface_item_at(surface, point) {
                    Some(item) => self.show_item_menu(ui, surface, item, point),
                    None => {
                        if let Surface::Space(id) = surface {
                            self.show_space_menu(ui, id, point);
                        }
                    }
                }
            }
            SpaceEvent::DoubleClick { x, y } => self.double_click(ui, surface, Point::new(x, y)),
            SpaceEvent::MouseLeave => {
                self.dismiss_info_tip(ui);
                if self.hovered.is_some_and(|(owner, _)| owner == surface) {
                    self.hovered = None;
                    self.refresh_surface(ui, surface);
                }
            }
            SpaceEvent::FilesDropped {
                x,
                y,
                paths,
                keep_in_place,
            } => {
                // Only a space registers an OLE drop target, so `id` is a
                // space here.
                if let Surface::Space(space) = surface {
                    self.add_dropped(ui, space, Point::new(x, y), paths, keep_in_place);
                }
            }
            SpaceEvent::DpiChanged { .. } => {
                if let Surface::Space(id) = surface {
                    self.on_dpi_changed(ui, id);
                }
            }
            _ => {}
        }
    }

    pub(super) fn begin_interaction(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        point: Point,
        ctrl: bool,
        shift: bool,
    ) {
        // A click on an item's check box either toggles that item in the
        // selection (auto-check-select) or the independent tick set, and moves
        // the highlight to it.
        if let Some(item) = self.checkbox_at(surface, point) {
            let previous = self.selected_surface.replace(surface);
            if self.surface_check_box_mode(surface) == CheckBoxMode::Independent {
                if let Some(index) = self.checked_items.iter().position(|entry| *entry == item) {
                    self.checked_items.remove(index);
                } else {
                    self.checked_items.push(item);
                }
                self.selected_items = vec![item];
            } else {
                self.toggle_selected(item);
            }
            self.selection_anchor = Some(item);
            self.focused = Some(item);
            if let Some(previous) = previous.filter(|previous| *previous != surface) {
                self.refresh_surface(ui, previous);
            }
            self.refresh_surface(ui, surface);
            return;
        }
        let previous = self.selected_surface.replace(surface);
        self.pending_click = None;
        let item = self.surface_item_at(surface, point);
        match item {
            Some(item) => {
                if shift {
                    self.extend_selection(surface, item);
                } else if ctrl {
                    self.toggle_selected(item);
                } else if self.is_selected(item) {
                    // Keep the whole selection so a drag of one member moves
                    // the group. On release, collapse it and maybe rename.
                    self.pending_click = Some(pending_click(&self.selected_items, item));
                } else {
                    self.selected_items = vec![item];
                }
                self.selection_anchor = Some(item);
                self.focused = Some(item);
            }
            None => {
                if !ctrl && !shift {
                    self.selected_items.clear();
                    self.focused = None;
                }
            }
        }
        match surface {
            Surface::Space(id) => {
                let Some(space) = self.config.desktop.space(id) else {
                    return;
                };
                let edges = space.resize_edges_at(point, RESIZE_BORDER);
                if edges.any() {
                    self.drag = Drag::Resize { id, edges };
                    self.selected_items.clear();
                } else if let Some(item) = item {
                    self.drag = Drag::Item {
                        surface,
                        item,
                        origin: point,
                        active: false,
                    };
                } else if !space.locked && space.title_bar().contains(point) {
                    self.drag = Drag::Space {
                        id,
                        grab: Point::new(point.x - space.rect.x, point.y - space.rect.y),
                    };
                } else if space.content_rect().contains(point) {
                    // Empty content: start a rubber-band selection.
                    self.drag = Drag::Marquee {
                        surface,
                        origin: point,
                        additive: ctrl || shift,
                    };
                }
            }
            Surface::Desktop => {
                self.drag = match item {
                    Some(item) => Drag::Item {
                        surface,
                        item,
                        origin: point,
                        active: false,
                    },
                    None => Drag::Marquee {
                        surface,
                        origin: point,
                        additive: ctrl || shift,
                    },
                };
            }
        }
        if let Some(previous) = previous.filter(|previous| *previous != surface) {
            self.refresh_surface(ui, previous);
        }
        self.refresh_surface(ui, surface);
    }

    /// Selects the item under `point` (or nothing) without starting a drag,
    /// as a right-click does before its menu opens. Right-clicking an item
    /// that is already part of a multi-selection keeps the selection.
    pub(super) fn select_at(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let previous = self.selected_surface.replace(surface);
        let item = self.surface_item_at(surface, point);
        match item {
            Some(item) if !self.is_selected(item) => {
                self.selected_items = vec![item];
                self.selection_anchor = Some(item);
                self.focused = Some(item);
            }
            Some(item) => self.focused = Some(item),
            None => {
                self.selected_items.clear();
                self.focused = None;
            }
        }
        if let Some(previous) = previous.filter(|previous| *previous != surface) {
            self.refresh_surface(ui, previous);
        }
        self.refresh_surface(ui, surface);
    }

    pub(super) fn drag_to(&mut self, ui: &mut Ui, point: Point) -> bool {
        match self.drag {
            Drag::None => false,
            Drag::Item {
                surface,
                item,
                origin,
                active,
            } => {
                if !active {
                    let threshold = drag_threshold();
                    if (point.x - origin.x).abs() < threshold.width
                        && (point.y - origin.y).abs() < threshold.height
                    {
                        return true;
                    }
                    self.drag = Drag::Item {
                        surface,
                        item,
                        origin,
                        active: true,
                    };
                    self.begin_drag_visual(ui, surface, item, origin, point);
                }
                // Still over the surface: rearrange (a space) or reposition (a
                // loose item). Left it: hand off to the shell, which moves the
                // file to another space or app.
                if self.surface_contains(surface, point) {
                    ui.move_drag_ghost(point);
                    match surface {
                        Surface::Space(id) => self.reorder_item_at(ui, id, item, point),
                        Surface::Desktop => self.move_desktop_item(ui, item, origin, point),
                    }
                } else {
                    self.drag = Drag::None;
                    // The shell draws the same image from here on.
                    ui.hide_drag_ghost();
                    self.drag_item_out(ui, surface, item);
                    self.end_drag_visual(ui);
                }
                true
            }
            Drag::Marquee {
                surface,
                origin,
                additive,
            } => {
                self.update_marquee(ui, surface, origin, additive, point);
                true
            }
            Drag::Space { id, grab } => {
                let Some(space) = self.config.desktop.space(id) else {
                    self.drag = Drag::None;
                    return false;
                };
                let rect = Rect::new(
                    point.x - grab.x,
                    point.y - grab.y,
                    space.rect.width,
                    space.rect.height,
                );
                if self.dispatch(Command::SetSpaceRect { id, rect }, point) {
                    if let Some(space) = self.config.desktop.space(id) {
                        ui.set_space_rect(id.0, space.visible_rect());
                    }
                }
                true
            }
            Drag::Resize { id, edges } => {
                let before = self.config.desktop.space(id).map(|space| space.rect);
                let command = Command::ResizeSpace {
                    id,
                    edges,
                    to: point,
                };
                if self.dispatch(command, point)
                    && self.config.desktop.space(id).map(|space| space.rect) != before
                {
                    self.refresh_space(ui, id);
                }
                true
            }
        }
    }

    /// Repositions a loose desktop item as it is dragged, so it follows the
    /// pointer at its absolute virtual-desktop position.
    pub(super) fn move_desktop_item(
        &mut self,
        ui: &mut Ui,
        item: ItemId,
        origin: Point,
        point: Point,
    ) {
        let Some(entry) = self.config.desktop.layer().item(item) else {
            return;
        };
        let start = entry.position;
        let position = Point::new(start.x + point.x - origin.x, start.y + point.y - origin.y);
        if self.dispatch(Command::SetDesktopItemPosition { item, position }, point) {
            self.refresh_desktop_layer(ui);
        }
    }

    pub(super) fn update_cursor(&self, ui: &mut Ui, id: SpaceId, point: Point) {
        let edges = self
            .config
            .desktop
            .space(id)
            .map_or(ResizeEdges::NONE, |space| {
                space.resize_edges_at(point, RESIZE_BORDER)
            });
        ui.set_space_cursor(id.0, cursor_for(edges));
    }

    /// Ctrl+wheel changes the global icon size, as it does on the desktop:
    /// spaces that follow Global resize together, while a customized space
    /// keeps its own size. Persisted through the settings, not the config's
    /// base grid.
    pub(super) fn adjust_icon_size(&mut self, ui: &mut Ui, delta: i32) {
        let step = delta * ICON_SIZE_WHEEL_STEP / WHEEL_DELTA;
        if step == 0 {
            return;
        }
        let current = self
            .config
            .theme
            .icon_size
            .clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1) as i32;
        let next = (current + step).clamp(ICON_SIZE_RANGE.0.into(), ICON_SIZE_RANGE.1.into());
        if next as u8 == self.config.theme.icon_size {
            return;
        }
        self.config.theme.icon_size = next as u8;
        self.relayout_all(ui);
        self.schedule_save(ui);
        self.refresh_settings(ui);
    }

    pub(super) fn scroll_by(&mut self, ui: &mut Ui, id: SpaceId, point: Point, delta: i32) {
        let layout = self.layout_for(id);
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let max = space.max_scroll(layout.cell(), layout.padding);
        let current = self.scroll_of(id);
        let next = (current - delta * layout.scroll_step() / WHEEL_DELTA).clamp(0, max);
        if next == current {
            return;
        }
        self.scroll.insert(id, next);
        // Items moved under a still cursor, so the hovered one changed too.
        let surface = Surface::Space(id);
        self.hovered = self
            .surface_item_at(surface, point)
            .map(|item| (surface, item));
        self.refresh_space(ui, id);
    }

    pub(super) fn end_drag(&mut self, ui: &mut Ui) {
        let pending = self.pending_click.take();
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::None => {}
            Drag::Item {
                surface,
                item,
                active: false,
                ..
            } => {
                // A press on an already-selected item that never became a drag
                // is a plain click: collapse the selection, or start a rename
                // when it was already the only selected item. Only a space
                // renames inline.
                match resolve_pending_click(pending, item, self.is_selected(item)) {
                    PendingClickOutcome::Rename => {
                        if let Surface::Space(id) = surface {
                            self.begin_item_rename(ui, id, item);
                        }
                    }
                    PendingClickOutcome::Collapse => {
                        self.selected_items = vec![item];
                        self.selection_anchor = Some(item);
                        self.focused = Some(item);
                        self.refresh_surface(ui, surface);
                    }
                    PendingClickOutcome::Nothing => {}
                }
            }
            Drag::Item { active: true, .. } => {
                self.end_drag_visual(ui);
                self.schedule_save(ui);
            }
            Drag::Marquee { surface, .. } => {
                // The selection already updated live; just drop the rectangle.
                self.marquee = None;
                self.refresh_surface(ui, surface);
            }
            Drag::Space { .. } | Drag::Resize { .. } => self.schedule_save(ui),
        }
    }

    /// Fades the dragged items' slots and floats their group drag image under
    /// the pointer, as Explorer does once an icon drag starts. `origin` is
    /// where the button went down, so the image keeps the spot that was
    /// grabbed.
    pub(super) fn begin_drag_visual(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        item: ItemId,
        origin: Point,
        point: Point,
    ) {
        // The whole active set drags, as in Explorer, so the image is that set
        // rendered as it is now, before the slots fade.
        let ids = self.drag_items(item);
        let id_values: Vec<u64> = ids.iter().map(|id| id.0).collect();
        let view = self.view_for(surface);
        let layout = self.surface_layout(surface);
        let metrics = self.surface_metrics(surface);
        let palette = self.surface_palette(surface);
        let grab = self
            .surface_window_rect(surface)
            .map(|rect| Point::new(origin.x - rect.x, origin.y - rect.y));
        self.drag_image = match surface {
            Surface::Space(id) => self.config.desktop.space(id).and_then(|entry| {
                let scene = drag_scene(
                    entry,
                    &ids,
                    entry.theme(&self.config.theme),
                    &palette,
                    layout,
                    metrics,
                    view,
                    &mut self.icons,
                );
                DragImage::of_selection(&scene, &id_values, grab?)
            }),
            Surface::Desktop => {
                let scene = desktop_layer_drag_scene(
                    self.config.desktop.layer(),
                    virtual_desktop_rect().origin(),
                    &ids,
                    &self.config.theme,
                    &palette,
                    layout,
                    metrics,
                    view,
                    &mut self.icons,
                );
                grab.and_then(|grab| DragImage::of_selection(&scene, &id_values, grab))
            }
        };
        self.dragged = Some((surface, ids));
        self.pending_click = None;
        if let Some(image) = &self.drag_image {
            ui.show_drag_ghost(image, point);
        }
        self.refresh_surface(ui, surface);
    }

    pub(super) fn end_drag_visual(&mut self, ui: &mut Ui) {
        ui.hide_drag_ghost();
        self.drag_image = None;
        if let Some((surface, _)) = self.dragged.take() {
            self.refresh_surface(ui, surface);
        }
    }

    pub(super) fn update_hover(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let hovered = self
            .surface_item_at(surface, point)
            .map(|item| (surface, item));
        if hovered == self.hovered {
            return;
        }
        let previous = std::mem::replace(&mut self.hovered, hovered);
        // A different item (or none) cancels any shown tip and its delay.
        ui.hide_info_tip();
        ui.clear_timer(INFO_TIP_TIMER);
        if hovered.is_some() {
            ui.set_timer(INFO_TIP_TIMER, INFO_TIP_DELAY_MS);
        }
        if let Some((owner, _)) = previous.filter(|(owner, _)| *owner != surface) {
            self.refresh_surface(ui, owner);
        }
        self.refresh_surface(ui, surface);
    }

    pub(super) fn double_click(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let layout = self.surface_layout(surface);
        let target = match surface {
            Surface::Space(id) => {
                self.config
                    .desktop
                    .space(id)
                    .map_or(DoubleClickTarget::Nothing, |space| {
                        double_click_target(
                            space,
                            layout.cell(),
                            self.surface_scroll(surface),
                            point,
                        )
                    })
            }
            Surface::Desktop => self
                .surface_item_at(surface, point)
                .map_or(DoubleClickTarget::Nothing, DoubleClickTarget::Item),
        };
        tracing::debug!(?surface, ?point, ?target, "double-click");
        match target {
            DoubleClickTarget::TitleBar => {
                if let Surface::Space(id) = surface {
                    self.toggle_rollup(ui, id);
                }
            }
            DoubleClickTarget::Item(item) => {
                if let Some(path) = self.config.desktop.item(item).map(|item| item.path.clone()) {
                    ui.open_path(&path);
                }
            }
            DoubleClickTarget::Nothing => {}
        }
    }

    /// Places the dragged item in the grid slot under `point`, reflowing the
    /// space. Used while an item is dragged within its own space.
    pub(super) fn reorder_item_at(
        &mut self,
        ui: &mut Ui,
        space: SpaceId,
        item: ItemId,
        point: Point,
    ) {
        let layout = self.layout_for(space);
        let moving = self.drag_items(item);
        let target = self.config.desktop.space(space).and_then(|space| {
            let content = space.content_rect();
            let columns = if space.user_sized {
                layout.columns_for(space.rect.width)
            } else {
                layout.columns
            }
            .max(1);
            let step_x = layout.cell_width + layout.gap;
            let step_y = layout.cell_height + layout.gap;
            let column = ((point.x - content.x - layout.padding) / step_x).clamp(0, columns - 1);
            let row = ((point.y - content.y - layout.padding) / step_y).max(0);
            let count = space.items.len();
            if count == 0 {
                return None;
            }
            let index = ((row * columns + column) as usize).min(count - 1);
            let current = space.items.iter().position(|entry| entry.id == item)?;
            Some((index, current))
        });
        let Some((index, current)) = target else {
            return;
        };
        if index == current {
            return;
        }
        let command = if moving.len() > 1 {
            Command::ReorderItems { ids: moving, index }
        } else {
            Command::ReorderItem { id: item, index }
        };
        if self.dispatch(command, point) {
            self.refresh_space(ui, space);
        }
    }

    /// Drags an item out like Explorer. Wherever it lands, the drop target
    /// alone decides what happens to the file, and has finished by the time
    /// the drag returns. The item leaves the space only if its file is gone
    /// (moved elsewhere); after a copy, link, or cancel it stays, as in
    /// Explorer. Drops on spaces arrive as `FilesDropped` after this returns
    /// and move the item between spaces.
    ///
    /// DeskSpace must never move the file itself here. It used to move a
    /// file dropped on the desktop back out of storage even when the desktop
    /// had copied it, which left `- Copy` and ` (2)` duplicates behind.
    pub(super) fn drag_item_out(&mut self, ui: &mut Ui, surface: Surface, item: ItemId) {
        // Dragging an item that is part of the active set drags the whole set,
        // as Explorer does; otherwise just that one item.
        let ids = self.drag_items(item);
        let (paths, positions): (Vec<PathBuf>, Vec<Point>) = self
            .item_cells(surface, &ids)
            .into_iter()
            .filter(|(path, _)| path.exists())
            .unzip();
        // A stale item (its file moved or deleted since it was drawn) is
        // dropped instead of repeatedly failing to start a drag.
        if paths.is_empty() {
            match surface {
                Surface::Space(id) => self.prune_missing(ui, id),
                Surface::Desktop => self.resync_desktop_layer(ui),
            }
            return;
        }
        let Some(result) = ui.drag_out(&paths, &positions, self.drag_image.as_ref()) else {
            return;
        };
        tracing::debug!(?result, files = paths.len(), "item drag ended");
        // The drop target may have moved some files out; drop any item whose
        // file is gone, wherever it lives, and reconcile the loose layer.
        self.prune_all_missing(ui);
        self.resync_desktop_layer(ui);
    }

    /// Updates the rubber-band rectangle and the live selection as the pointer
    /// moves over empty content.
    pub(super) fn update_marquee(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        origin: Point,
        additive: bool,
        point: Point,
    ) {
        let Some(content) = self.surface_content(surface) else {
            return;
        };
        let cells = self.surface_cells(surface);
        let left = origin.x.min(point.x).max(content.x);
        let top = origin.y.min(point.y).max(content.y);
        let right = origin.x.max(point.x).min(content.right());
        let bottom = origin.y.max(point.y).min(content.bottom());
        let rect = Rect::new(left, top, (right - left).max(0), (bottom - top).max(0));
        self.marquee = Some((surface, rect));

        self.selected_surface = Some(surface);
        self.selected_items = marquee_selection(&cells, rect, additive, &self.selected_items);
        self.selection_anchor = None;
        self.focused = self.selected_items.last().copied();
        self.refresh_surface(ui, surface);
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

/// What a double-click at `point` (desktop coordinates) acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoubleClickTarget {
    /// Rolls the space up or down.
    TitleBar,
    /// Opens the item.
    Item(ItemId),
    Nothing,
}

fn double_click_target(space: &Space, cell: Size, scroll: i32, point: Point) -> DoubleClickTarget {
    if space.title_bar().contains(point) {
        DoubleClickTarget::TitleBar
    } else if let Some(item) = space.item_at(point, cell, scroll) {
        DoubleClickTarget::Item(item)
    } else {
        DoubleClickTarget::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use deskspace_core::GridSpec;

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

    #[test]
    fn double_click_rolls_on_the_title_and_opens_items() {
        let layout = GridSpec::default();
        let mut space = Space::new(SpaceId(1), "S", Rect::new(100, 100, 240, 400));
        let mut item = deskspace_core::Item::new(ItemId(7), "C:/a.lnk");
        item.position = Point::new(12, 12);
        space.push_item(item);
        let target =
            |x, y, scroll| double_click_target(&space, layout.cell(), scroll, Point::new(x, y));
        let in_item = (100 + 12 + 10, 100 + space.title_height + 12 + 10);

        assert_eq!(target(150, 110, 0), DoubleClickTarget::TitleBar);
        assert_eq!(
            target(in_item.0, in_item.1, 0),
            DoubleClickTarget::Item(ItemId(7))
        );
        assert_eq!(target(300, 450, 0), DoubleClickTarget::Nothing);
        // Scrolled past the item, the same point is empty content.
        assert_eq!(
            target(in_item.0, in_item.1, 200),
            DoubleClickTarget::Nothing
        );
    }
}
