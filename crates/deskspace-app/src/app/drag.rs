//! App dragging: the in-progress `Drag` gesture, the drag image, and
//! dropping items within, between, and out of surfaces (R1).

use std::path::PathBuf;

use deskspace_core::geometry::{Point, Rect};
use deskspace_core::{Command, ItemId, ResizeEdges, SpaceId};

use deskspace_platform::{DragImage, Ui, drag_threshold, space_dpi, virtual_desktop_rect};

use crate::scene::{SurfaceLook, desktop_layer_drag_scene, drag_scene};

use super::columns::{ColumnResize, HeaderPress};
use super::pointer::{PendingClickOutcome, resize_edges_now, resolve_pending_click};
use super::undo::move_items_label;
use super::{App, Surface};

/// The in-progress pointer gesture on a surface, if any.
#[derive(Default)]
pub(super) enum Drag {
    #[default]
    None,
    Space {
        id: SpaceId,
        grab: Point,
    },
    Resize {
        id: SpaceId,
        edges: ResizeEdges,
        /// Where the button went down. The resize ignores small movements, so
        /// grabbing a border (to move or click the space) never resizes it.
        origin: Point,
        /// Whether the horizontal/vertical axes have passed the dead zone.
        /// Set once and kept, so the edge then tracks the pointer back.
        active_x: bool,
        active_y: bool,
    },
    /// The left button went down on an item. Past the drag threshold it
    /// rearranges the item within its surface; once the pointer leaves it the
    /// drag becomes a shell drag, as dragging an icon out of a folder.
    Item {
        surface: Surface,
        item: ItemId,
        origin: Point,
        active: bool,
    },
    /// Rubber-band selection over empty content.
    Marquee {
        surface: Surface,
        origin: Point,
        /// Ctrl/Shift: add to the selection instead of replacing it.
        additive: bool,
    },
    /// A Details header divider is being dragged.
    ColumnResize(Box<ColumnResize>),
    /// The left button went down on a Details header: a click or a reorder.
    HeaderPress(HeaderPress),
}

impl App {
    pub(super) fn drag_to(&mut self, ui: &mut Ui, point: Point) -> bool {
        match self.pointer.drag {
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
                    self.pointer.drag = Drag::Item {
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
                    self.pointer.drag = Drag::None;
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
                if self.config.desktop.space(id).is_none() {
                    self.pointer.drag = Drag::None;
                    return false;
                }
                if let Some(rect) = self.drag_space_to(id, grab, point) {
                    ui.set_space_rect(id.0, rect);
                }
                true
            }
            Drag::Resize {
                id,
                edges,
                origin,
                active_x,
                active_y,
            } => {
                if self.resize_space_to(id, edges, origin, (active_x, active_y), point) {
                    self.refresh_space(ui, id);
                }
                true
            }
            Drag::ColumnResize(_) => {
                self.resize_column_to(ui, point);
                true
            }
            Drag::HeaderPress(_) => {
                self.drag_header_to(ui, point);
                true
            }
        }
    }

    pub(super) fn drag_space_to(&mut self, id: SpaceId, grab: Point, point: Point) -> Option<Rect> {
        let space = self.config.desktop.space(id)?;
        let rect = Rect::new(
            point.x - grab.x,
            point.y - grab.y,
            space.rect().width,
            space.rect().height,
        );
        self.undo.begin_if_none("Move space");
        // Not `commit`: runs on every pointer move; `end_drag` saves once.
        self.dispatch(Command::SetSpaceRect { id, rect }, point)
            .is_applied()
            .then(|| {
                self.config
                    .desktop
                    .space(id)
                    .map(|space| space.visible_rect())
            })
            .flatten()
    }

    pub(super) fn resize_space_to(
        &mut self,
        id: SpaceId,
        edges: ResizeEdges,
        origin: Point,
        active: (bool, bool),
        point: Point,
    ) -> bool {
        let layout = self.layout_for(id);
        let (active_edges, active) = resize_edges_now(edges, origin, point, layout.cell(), active);
        self.pointer.drag = Drag::Resize {
            id,
            edges,
            origin,
            active_x: active.0,
            active_y: active.1,
        };
        if !active.0 && !active.1 {
            return false;
        }
        let before = self.config.desktop.space(id).map(|space| space.rect());
        let command = Command::ResizeSpace {
            id,
            edges: active_edges,
            to: point,
        };
        self.undo.begin_if_none("Resize space");
        // Not `commit`: runs on every pointer move; `end_drag` saves once.
        self.dispatch(command, point).is_applied()
            && self.config.desktop.space(id).map(|space| space.rect()) != before
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
        // Not `commit`: runs on every pointer move; `end_drag` saves once.
        if self
            .dispatch(Command::SetDesktopItemPosition { item, position }, point)
            .is_applied()
        {
            self.refresh_desktop_layer(ui);
        }
    }

    pub(super) fn end_drag(&mut self, ui: &mut Ui) {
        let pending = self.pointer.pending_click.take();
        match std::mem::replace(&mut self.pointer.drag, Drag::None) {
            Drag::None => {}
            Drag::Item {
                surface,
                item,
                active: false,
                ..
            } => {
                // A press on an already-selected item that never became a drag
                // is a plain click: collapse the selection, or start a rename
                // when it was already the only selected item.
                match resolve_pending_click(pending, item, self.selection.is_selected(item)) {
                    PendingClickOutcome::Rename => self.begin_item_rename(ui, surface, item),
                    PendingClickOutcome::Collapse => {
                        self.selection.select_only(surface, item);
                        self.refresh_surface(ui, surface);
                    }
                    PendingClickOutcome::Nothing => {}
                }
            }
            Drag::Item { active: true, .. } => {
                self.undo.finish(&self.config.desktop);
                self.end_drag_visual(ui);
                self.schedule_save(ui);
            }
            Drag::Marquee { surface, .. } => {
                // The selection already updated live; just drop the rectangle.
                self.pointer.marquee = None;
                self.refresh_surface(ui, surface);
            }
            Drag::Space { .. } | Drag::Resize { .. } => {
                self.undo.finish(&self.config.desktop);
                self.schedule_save(ui);
            }
            Drag::ColumnResize(resize) => self.finish_column_resize(ui, *resize),
            Drag::HeaderPress(press) => self.finish_header_press(ui, press),
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
        self.pointer.drag_image = self.drag_image_of(surface, &ids, origin);
        self.pointer.dragged = Some((surface, ids));
        self.pointer.pending_click = None;
        if let Some(image) = &self.pointer.drag_image {
            ui.show_drag_ghost(image, point);
        }
        self.refresh_surface(ui, surface);
    }

    /// The group drag image of `ids` on `surface`, grabbed at `origin` (where
    /// the button went down, in screen coordinates). `None` when the surface
    /// has no window or nothing to draw.
    pub(super) fn drag_image_of(
        &mut self,
        surface: Surface,
        ids: &[ItemId],
        origin: Point,
    ) -> Option<DragImage> {
        let id_values: Vec<u64> = ids.iter().map(|id| id.0).collect();
        let view = self.view_for(surface);
        let layout = self.surface_layout(surface);
        let metrics = self.surface_metrics(surface);
        let palette = self.surface_palette(surface);
        let rect = self.surface_window_rect(surface)?;
        let grab = Point::new(origin.x - rect.x, origin.y - rect.y);
        let scene = match surface {
            Surface::Space(id) => {
                let entry = self.config.desktop.space(id)?;
                let look = SurfaceLook {
                    theme: entry.theme(&self.config.theme),
                    palette: &palette,
                    layout,
                    metrics,
                    scale: space_dpi(id.0),
                };
                drag_scene(entry, ids, look, view, &mut self.icons)
            }
            Surface::Desktop => desktop_layer_drag_scene(
                self.config.desktop.layer(),
                virtual_desktop_rect().origin(),
                ids,
                SurfaceLook {
                    theme: &self.config.theme,
                    palette: &palette,
                    layout,
                    metrics,
                    scale: 1.0,
                },
                view,
                &mut self.icons,
            ),
        };
        DragImage::of_selection(&scene, &id_values, grab)
    }

    pub(super) fn end_drag_visual(&mut self, ui: &mut Ui) {
        ui.hide_drag_ghost();
        self.pointer.drag_image = None;
        if let Some((surface, _)) = self.pointer.dragged.take() {
            self.refresh_surface(ui, surface);
        }
    }

    /// Places the dragged item in the slot under `point`, reflowing the space.
    /// Used while an item is dragged within its own space. A space that keeps
    /// sorted never rearranges (D17); a drag out still works.
    pub(super) fn reorder_item_at(
        &mut self,
        ui: &mut Ui,
        space: SpaceId,
        item: ItemId,
        point: Point,
    ) {
        if self.reorder_item_step(space, item, point) {
            self.refresh_space(ui, space);
        }
    }

    pub(super) fn reorder_item_step(&mut self, space: SpaceId, item: ItemId, point: Point) -> bool {
        if self
            .config
            .desktop
            .space(space)
            .is_some_and(|space| space.keeps_sorted())
        {
            return false;
        }
        let Some(layout) = self.view_layout(space) else {
            return false;
        };
        let moving = self.drag_items(item);
        let target = self.config.desktop.space(space).and_then(|space| {
            let content = space.content_rect();
            let point = Point::new(point.x - content.x, point.y - content.y);
            let local = layout.layout_point(point, self.scroll_of(space.id));
            let index = layout.drop_index(local)?;
            let current = space.items().iter().position(|entry| entry.id == item)?;
            Some((index, current))
        });
        let Some((index, current)) = target else {
            return false;
        };
        if index == current {
            return false;
        }
        self.undo.begin_if_none(move_items_label(moving.len()));
        let command = if moving.len() > 1 {
            Command::ReorderItems { ids: moving, index }
        } else {
            Command::ReorderItem { id: item, index }
        };
        // Not `commit`: runs on every pointer move; `end_drag` saves once.
        self.dispatch(command, point).is_applied()
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
            self.undo.cancel();
            match surface {
                Surface::Space(id) => self.prune_missing(ui, id),
                Surface::Desktop => self.resync_desktop_layer(ui),
            }
            return;
        }
        let Some(result) = ui.drag_out(&paths, &positions, self.pointer.drag_image.as_ref()) else {
            self.undo.cancel();
            return;
        };
        tracing::debug!(?result, files = paths.len(), "item drag ended");
        // A drop on a space already finished the open undo entry; a drag out
        // to Explorer is owned by Explorer's undo (ROADMAP D11).
        self.undo.cancel();
        // The drop target may have moved some files out; drop any item whose
        // file is gone, wherever it lives, and reconcile the loose layer.
        self.prune_all_missing(ui);
        self.resync_desktop_layer(ui);
    }
}
