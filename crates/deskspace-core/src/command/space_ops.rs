//! Commands that create, remove, move, size, and restyle spaces.

use std::collections::HashSet;

use super::CommandOutcome;
use crate::desktop::Desktop;
use crate::geometry::{Point, Rect};
use crate::icon::SpaceIcon;
use crate::id::SpaceId;
use crate::item::Item;
use crate::layout::{
    GridSpec, arrange_space, clamp_to_bounds, position_items, resize_rect, resolve_collisions,
    snap_rect,
};
use crate::sort::SortRule;
use crate::source::{LensTitle, SpaceSource};
use crate::space::{ResizeEdges, Space, SpaceLook};
use crate::view::SpaceView;
use crate::view_layout::{ViewMetrics, fit_size};

impl Desktop {
    pub(super) fn apply_create_space(
        &mut self,
        title: String,
        rect: Rect,
        locked: bool,
        icon: SpaceIcon,
        view: SpaceView,
        bounds: Rect,
    ) -> CommandOutcome {
        let rect = clamp_to_bounds(rect, bounds);
        let id = self.add_space(title, rect);
        if let Some(space) = self.space_mut(id) {
            space.locked = locked;
            space.icon = icon;
            space.view = view;
        }
        CommandOutcome::Applied
    }

    pub(super) fn apply_delete_space(&mut self, id: SpaceId) -> CommandOutcome {
        match self.remove_space(id) {
            Some(_) => CommandOutcome::Applied,
            None => CommandOutcome::NotFound,
        }
    }

    /// Removes every listed space first, then inserts the restored ones in
    /// index order, so each lands at the index it was recorded at while the
    /// spaces around it are as they were.
    pub(super) fn apply_restore_spaces(
        &mut self,
        mut spaces: Vec<(SpaceId, Option<(usize, Space)>)>,
        layout: GridSpec,
    ) -> CommandOutcome {
        if !self.can_restore(&spaces) {
            return CommandOutcome::Rejected;
        }
        for (id, state) in &mut spaces {
            let Some(removed) = self.remove_space(*id) else {
                continue;
            };
            let restored = state.as_mut().map(|(_, space)| space);
            let items = keep_lens_items(removed, restored, layout);
            self.retire_items(items);
        }
        spaces.sort_by_key(|(_, state)| state.as_ref().map(|(index, _)| *index));
        for (index, space) in spaces.into_iter().filter_map(|(_, state)| state) {
            self.note_ids(&space);
            let index = index.min(self.spaces.len());
            self.spaces.insert(index, space);
        }
        CommandOutcome::Applied
    }

    /// Whether `apply_restore_spaces` may apply `spaces`; see
    /// `Command::RestoreSpaces` for the rules.
    fn can_restore(&self, spaces: &[(SpaceId, Option<(usize, Space)>)]) -> bool {
        let listed: Vec<SpaceId> = spaces.iter().map(|(id, _)| *id).collect();
        let unique = listed
            .iter()
            .enumerate()
            .all(|(index, id)| !listed[..index].contains(id));
        let ids_match = spaces
            .iter()
            .all(|(id, state)| state.as_ref().is_none_or(|(_, space)| space.id == *id));
        if !unique || !ids_match {
            return false;
        }
        let mut restored = HashSet::new();
        spaces
            .iter()
            .filter_map(|(_, state)| state.as_ref())
            .flat_map(|(_, space)| space.items())
            .all(|item| {
                let held_elsewhere = self
                    .space_containing(item.id)
                    .is_some_and(|holder| !listed.contains(&holder));
                restored.insert(item.id) && !held_elsewhere && self.layer.item(item.id).is_none()
            })
    }

    pub(super) fn apply_rename_space(&mut self, id: SpaceId, title: String) -> CommandOutcome {
        self.with_space(id, |space| {
            space.title = title;
            if let SpaceSource::Folder(view) = &mut space.source {
                view.title = LensTitle::Custom;
            }
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_set_space_view(
        &mut self,
        id: SpaceId,
        view: SpaceView,
        layout: GridSpec,
    ) -> CommandOutcome {
        self.with_space(id, |space| {
            let kept = space.keeps_sorted();
            space.view = view;
            // Turning the choice on sorts at once, so a space never claims an
            // order it does not hold.
            if space.keeps_sorted() && !kept {
                space.view.sort.sort_items(&mut space.items);
                position_items(space, layout);
            }
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_set_space_icon(&mut self, id: SpaceId, icon: SpaceIcon) -> CommandOutcome {
        self.with_space(id, |space| {
            space.icon = icon;
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_move_space(
        &mut self,
        id: SpaceId,
        delta: Point,
        layout: GridSpec,
        bounds: Rect,
    ) -> CommandOutcome {
        self.with_space(id, |space| {
            if space.locked {
                return CommandOutcome::Rejected;
            }
            let moved = space.rect.offset(delta.x, delta.y);
            space.rect = clamp_to_bounds(snap_rect(moved, layout.snap), bounds);
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_set_space_rect(
        &mut self,
        id: SpaceId,
        rect: Rect,
        layout: GridSpec,
        bounds: Rect,
    ) -> CommandOutcome {
        self.with_space(id, |space| {
            if space.locked {
                return CommandOutcome::Rejected;
            }
            space.rect = clamp_to_bounds(snap_rect(rect, layout.snap), bounds);
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_resize_space(
        &mut self,
        id: SpaceId,
        edges: ResizeEdges,
        to: Point,
        metrics: ViewMetrics,
        layout: GridSpec,
        bounds: Rect,
    ) -> CommandOutcome {
        self.with_space(id, |space| {
            if space.locked || space.folded || !edges.any() {
                return CommandOutcome::Rejected;
            }
            let min = layout.min_space_size();
            let resized = resize_rect(space.rect, edges, to, min, layout.snap, bounds);
            space.rect = if layout.fit_to_grid {
                let chrome =
                    space.title_height + space.unfolded_status_bar_height() + metrics.header;
                let fit_columns = !space.view().mode.rows_fill_width();
                clamp_to_bounds(
                    metrics.fit_rect(resized, edges, chrome, fit_columns),
                    bounds,
                )
            } else {
                resized
            };
            space.user_sized = true;
            position_items(space, layout);
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_fit_to_contents(
        &mut self,
        id: SpaceId,
        metrics: ViewMetrics,
        layout: GridSpec,
        bounds: Rect,
    ) -> CommandOutcome {
        self.with_space(id, |space| {
            if space.locked {
                return CommandOutcome::Rejected;
            }
            space.user_sized = false;
            arrange_space(space, layout);
            if space.view.places_by_order() {
                let size = fit_size(space, metrics);
                space.rect.width = size.width;
                space.rect.height = size.height;
            } else {
                // Fit an icon grid into a narrower work area with whole
                // columns, then let it scroll down (D23).
                let columns = layout.columns_for(bounds.width);
                if columns < layout.columns {
                    arrange_space(space, GridSpec { columns, ..layout });
                }
            }
            // No fitted space is larger than its work area; items beyond it
            // scroll (D23).
            space.rect.width = space.rect.width.min(bounds.width);
            space.rect.height = space.rect.height.min(bounds.height);
            space.rect = clamp_to_bounds(space.rect, bounds);
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_keep_size(&mut self, id: SpaceId) -> CommandOutcome {
        self.with_space(id, |space| {
            space.user_sized = true;
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_reflow(&mut self, id: SpaceId, layout: GridSpec) -> CommandOutcome {
        self.with_space(id, |space| {
            if space.locked {
                // A locked space keeps its size; the scale still reaches its
                // title height and item positions.
                space.title_height = layout.title_height;
                position_items(space, layout);
            } else {
                // `arrange_space` resizes auto spaces, reflows user-sized
                // ones, and applies the title height either way.
                arrange_space(space, layout);
            }
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_sort_space(
        &mut self,
        id: SpaceId,
        rule: SortRule,
        layout: GridSpec,
    ) -> CommandOutcome {
        self.with_space(id, |space| {
            space.view.sort = rule;
            rule.sort_items(&mut space.items);
            position_items(space, layout);
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_customize_space(
        &mut self,
        id: SpaceId,
        on: bool,
        seed: SpaceLook,
    ) -> CommandOutcome {
        self.with_space(id, |space| {
            if on && space.look.is_none() {
                space.look = Some(seed.clamped());
            }
            space.custom = on;
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_set_space_look(
        &mut self,
        id: SpaceId,
        custom: bool,
        look: Option<SpaceLook>,
    ) -> CommandOutcome {
        self.with_space(id, |space| {
            space.look = look.map(SpaceLook::clamped);
            space.custom = custom;
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_toggle_lock(&mut self, id: SpaceId) -> CommandOutcome {
        self.with_space(id, |space| {
            space.locked = !space.locked;
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_toggle_fold(&mut self, id: SpaceId) -> CommandOutcome {
        self.with_space(id, |space| {
            space.folded = !space.folded;
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_auto_arrange(&mut self, id: SpaceId, layout: GridSpec) -> CommandOutcome {
        self.with_space(id, |space| {
            arrange_space(space, layout);
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_snap_all(&mut self, layout: GridSpec, bounds: Rect) -> CommandOutcome {
        for space in self.spaces.iter_mut().filter(|space| !space.locked) {
            space.rect = clamp_to_bounds(snap_rect(space.rect, layout.snap), bounds);
        }
        resolve_collisions(&mut self.spaces, layout.gap, bounds);
        CommandOutcome::Applied
    }
}

/// Gives `restored`, a lens undo brought back without items, the ones `removed`
/// holds if it shows the same folder, and returns the items nothing keeps: the
/// folder has not changed, so they are still right, and the lens's worker
/// builds a new list if its rules did. They are placed again if the size
/// changed, since a resized space flows them into other columns.
fn keep_lens_items(
    mut removed: Space,
    restored: Option<&mut Space>,
    layout: GridSpec,
) -> Vec<Item> {
    let items = std::mem::take(&mut removed.items);
    let Some(restored) = restored else {
        return if removed.is_lens() { items } else { Vec::new() };
    };
    if !restored.is_lens() {
        return Vec::new();
    }
    if removed.folder_view().map(|view| &view.path) != restored.folder_view().map(|view| &view.path)
    {
        return items;
    }
    restored.items = items;
    if removed.rect.size() != restored.rect.size() {
        position_items(restored, layout);
    }
    Vec::new()
}
