//! `App` selection: the item selection, keyboard navigation, and
//! type-to-select (R1), plus the `Selection` state they share (R2).

use std::collections::HashSet;

use deskspace_core::geometry::Point;
use deskspace_core::{ItemId, Rect, SpaceId};
use deskspace_platform::{CheckBoxMode, NavDir, NavKey, Ui};

use crate::scene::item_label;

use super::{App, ScrollKey, Surface, Timer};

/// How long a type-to-select prefix lasts without another keystroke.
const TYPE_DELAY_MS: u32 = 900;

/// The item selection and the per-item marks that travel with it: the current
/// surface, the selected items, the Shift anchor, the keyboard focus, the cut
/// (ghosted) items, and the independent check-box ticks. Grouping them gives
/// one "select only this item" and one "forget this item" in place of the
/// copies the pointer, clipboard, and file paths used to repeat.
#[derive(Debug, Default)]
pub(super) struct Selection {
    /// The surface the selection lives in, if any.
    pub(super) surface: Option<Surface>,
    /// Items selected in `surface`, in the order they were selected.
    pub(super) items: Vec<ItemId>,
    /// The item a Shift+click or Shift+arrow extends the selection from.
    pub(super) anchor: Option<ItemId>,
    /// The item with keyboard focus, drawn with a focus rectangle. Follows the
    /// selection, but Ctrl+arrow moves it on its own.
    pub(super) focused: Option<ItemId>,
    /// Items on the clipboard as a cut, shown ghosted until pasted.
    pub(super) cut: HashSet<ItemId>,
    /// Ticked items in independent item-check-box mode. Session-only.
    pub(super) checked: Vec<ItemId>,
}

impl Selection {
    /// Selects exactly `item` in `surface`, moving the anchor and focus to it,
    /// as a plain click does.
    pub(super) fn select_only(&mut self, surface: Surface, item: ItemId) {
        self.surface = Some(surface);
        self.items = vec![item];
        self.anchor = Some(item);
        self.focused = Some(item);
    }

    /// Selects `items` in `surface`, anchoring and focusing the first, as
    /// Ctrl+A or a paste does.
    pub(super) fn select_items(&mut self, surface: Surface, items: Vec<ItemId>) {
        self.surface = Some(surface);
        self.anchor = items.first().copied();
        self.focused = self.anchor;
        self.items = items;
    }

    /// Moves the anchor and focus to `item`, as a Shift/Ctrl click does before
    /// it extends or toggles the selection.
    pub(super) fn focus(&mut self, item: ItemId) {
        self.anchor = Some(item);
        self.focused = Some(item);
    }

    /// Clears the selected items and focus, keeping the surface and anchor, as
    /// clicking empty content does.
    pub(super) fn deselect(&mut self) {
        self.items.clear();
        self.focused = None;
    }

    /// Clears the selected items, the anchor, and the focus, keeping the
    /// surface.
    pub(super) fn clear_items(&mut self) {
        self.items.clear();
        self.anchor = None;
        self.focused = None;
    }

    /// Clears the whole selection, surface included.
    pub(super) fn clear(&mut self) {
        let _ = self.take();
    }

    /// Clears the selection and reports the surface that showed it, when one
    /// did, so it can be repainted.
    pub(super) fn take(&mut self) -> Option<Surface> {
        let surface = self.surface.take();
        let had = !self.items.is_empty();
        self.clear_items();
        had.then_some(surface).flatten()
    }

    /// Drops `item` from every per-item mark after it is removed or its file
    /// is gone.
    pub(super) fn forget(&mut self, item: ItemId) {
        self.items.retain(|selected| *selected != item);
        if self.anchor == Some(item) {
            self.anchor = None;
        }
        if self.focused == Some(item) {
            self.focused = None;
        }
        self.cut.remove(&item);
        self.checked.retain(|checked| *checked != item);
    }

    pub(super) fn is_selected(&self, item: ItemId) -> bool {
        self.items.contains(&item)
    }

    /// Adds `item` to the selection, or removes it if it was already selected.
    pub(super) fn toggle(&mut self, item: ItemId) {
        if let Some(index) = self.items.iter().position(|entry| *entry == item) {
            self.items.remove(index);
        } else {
            self.items.push(item);
        }
    }

    /// Ticks or unticks `item` in the independent check-box set.
    pub(super) fn toggle_checked(&mut self, item: ItemId) {
        if let Some(index) = self.checked.iter().position(|entry| *entry == item) {
            self.checked.remove(index);
        } else {
            self.checked.push(item);
        }
    }

    /// Replaces the selected items, keeping the surface, anchor, and focus, as
    /// extending a range does.
    pub(super) fn replace_items(&mut self, items: Vec<ItemId>) {
        self.items = items;
    }

    /// Replaces the selection with a rubber-band result: the surface, the
    /// items, no anchor, and the last item focused.
    pub(super) fn rubber_band(&mut self, surface: Surface, items: Vec<ItemId>) {
        self.items = items;
        self.surface = Some(surface);
        self.anchor = None;
        self.focused = self.items.last().copied();
    }

    /// Clears the selected items, keeping the surface, anchor, and focus, as
    /// starting a space resize does.
    pub(super) fn clear_selected(&mut self) {
        self.items.clear();
    }
}

impl App {
    /// Clears the item selection, as clicking elsewhere does in Explorer.
    /// Returns the space that showed it, which needs a new scene.
    pub(super) fn clear_selection(&mut self, ui: &mut Ui) {
        if let Some(surface) = self.selection.take() {
            self.refresh_surface(ui, surface);
        }
    }

    /// Asks for outside clicks only while there is something for them to
    /// clear, because watching them hooks every mouse event in the session.
    pub(super) fn sync_outside_watch(&mut self, ui: &mut Ui) {
        let wanted = !self.selection.items.is_empty()
            || self.edit.is_active()
            || self.selection.surface.is_some();
        if wanted != self.watching_outside {
            ui.watch_outside_clicks(wanted);
            self.watching_outside = wanted;
        }
    }

    /// Selects every item in the current surface, as Ctrl+A does.
    pub(super) fn select_all_in_space(&mut self, ui: &mut Ui) {
        let Some(surface) = self.selection.surface else {
            return;
        };
        let ids: Vec<ItemId> = self
            .surface_items(surface)
            .iter()
            .map(|item| item.id)
            .collect();
        if ids.is_empty() {
            return;
        }
        self.selection.select_items(surface, ids);
        self.refresh_surface(ui, surface);
    }

    /// Scrolls the current space as Home, End, PageUp, or PageDown does.
    pub(super) fn scroll_current_space(&mut self, ui: &mut Ui, key: ScrollKey) {
        let Some(Surface::Space(id)) = self.selection.surface else {
            return;
        };
        let layout = self.layout_for(id);
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let max = space.max_scroll(layout.cell(), layout.padding);
        let page = space.content_rect().height.max(1);
        let current = self.scroll_of(id);
        let offset = match key {
            ScrollKey::Home => 0,
            ScrollKey::End => max,
            ScrollKey::PageUp => current - page,
            ScrollKey::PageDown => current + page,
        };
        let offset = offset.clamp(0, max);
        if offset == current {
            return;
        }
        self.scroll.insert(id, offset);
        // The items moved under a still cursor, so the hovered one changed.
        self.pointer.clear_hover_on(Surface::Space(id));
        self.refresh_space(ui, id);
    }

    /// Type-to-select: appends `ch` to the prefix buffer and selects the next
    /// item whose label starts with it, wrapping around. The buffer clears
    /// after a pause (`Timer::Type`), so a new search starts fresh.
    pub(super) fn on_typed(&mut self, ui: &mut Ui, ch: char) {
        let Some(surface) = self.selection.surface else {
            return;
        };
        self.type_buffer.push(ch);
        Self::set_timer(ui, Timer::Type, TYPE_DELAY_MS);
        let show_extensions =
            self.surface_match_explorer(surface) && self.shell_display.show_extensions;
        let labels: Vec<String> = self
            .surface_items(surface)
            .iter()
            .map(|item| item_label(item, show_extensions))
            .collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let current = self.selection.items.last().and_then(|item| {
            self.surface_items(surface)
                .iter()
                .position(|entry| entry.id == *item)
        });
        let start = current.map_or(0, |index| index + 1);
        let Some(index) = next_prefix_match(&labels, &self.type_buffer, start) else {
            return;
        };
        let Some(item) = self.surface_items(surface).get(index).map(|entry| entry.id) else {
            return;
        };
        self.selection.select_only(surface, item);
        if let Surface::Space(id) = surface {
            self.scroll_item_into_view(id, index);
        }
        self.refresh_surface(ui, surface);
    }

    /// Arrow-key navigation: move the selection the nearest item in that
    /// direction, extend it (Shift), or move the focus only (Ctrl), then bring
    /// the item into view.
    pub(super) fn on_navigate(&mut self, ui: &mut Ui, key: NavKey) {
        let Some(surface) = self.selection.surface else {
            return;
        };
        let layout = self.surface_layout(surface);
        let cells: Vec<Point> = self
            .surface_items(surface)
            .iter()
            .map(|item| {
                Point::new(
                    item.position.x + layout.cell_width / 2,
                    item.position.y + layout.cell_height / 2,
                )
            })
            .collect();
        let current = self
            .selection
            .focused
            .or_else(|| self.selection.items.last().copied())
            .and_then(|item| {
                self.surface_items(surface)
                    .iter()
                    .position(|entry| entry.id == item)
            });
        // A first arrow with nothing focused picks the first item.
        let target = match current {
            Some(index) => match next_in_direction(&cells, index, key.dir) {
                Some(index) => index,
                None => return,
            },
            None => 0,
        };
        let Some(item) = self
            .surface_items(surface)
            .get(target)
            .map(|entry| entry.id)
        else {
            return;
        };
        if key.focus {
            // Move the focus rectangle only; the selection stays.
            self.selection.focused = Some(item);
        } else if key.extend {
            self.selection.focused = Some(item);
            self.extend_selection(surface, item);
        } else {
            self.selection.select_only(surface, item);
        }
        if let Surface::Space(id) = surface {
            self.scroll_item_into_view(id, target);
        }
        self.refresh_surface(ui, surface);
    }

    /// Brings the item at `index` (in the space's order) into the content area
    /// by adjusting the scroll offset.
    pub(super) fn scroll_item_into_view(&mut self, id: SpaceId, index: usize) {
        let layout = self.layout_for(id);
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let Some(item) = space.items().get(index) else {
            return;
        };
        let content_height = space.content_rect().height;
        let max = space.max_scroll(layout.cell(), layout.padding);
        let scroll = self.scroll.get(&id).copied().unwrap_or(0);
        let top = item.position.y;
        let bottom = top + layout.cell_height;
        let offset = if top - scroll < 0 {
            top
        } else if bottom - scroll > content_height {
            bottom - content_height
        } else {
            return;
        };
        self.scroll.insert(id, offset.clamp(0, max));
    }

    /// Selects every item between the anchor and `item` in the space's order,
    /// as a Shift+click does.
    pub(super) fn extend_selection(&mut self, surface: Surface, item: ItemId) {
        let Some(anchor) = self.selection.anchor else {
            self.selection.replace_items(vec![item]);
            return;
        };
        let index = |target| {
            self.surface_items(surface)
                .iter()
                .position(|entry| entry.id == target)
        };
        let (Some(a), Some(b)) = (index(anchor), index(item)) else {
            self.selection.replace_items(vec![item]);
            return;
        };
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        let selected: Vec<ItemId> = self.surface_items(surface)[start..=end]
            .iter()
            .map(|entry| entry.id)
            .collect();
        self.selection.replace_items(selected);
    }

    /// The items an item operation acts on: the independent tick set when
    /// check boxes are separate and any are ticked, otherwise the selection.
    pub(super) fn active_items(&self) -> Vec<ItemId> {
        // Ticks can only be made in a space that shows check boxes, so any
        // left over in independent mode are the operand.
        let independent = self.shell_display.check_boxes == CheckBoxMode::Independent;
        if independent && !self.selection.checked.is_empty() {
            self.selection.checked.clone()
        } else {
            self.selection.items.clone()
        }
    }

    /// The items a drag started on `item` moves: the active set when it
    /// includes `item`, otherwise just `item`.
    pub(super) fn drag_items(&self, item: ItemId) -> Vec<ItemId> {
        let active = self.active_items();
        if active.contains(&item) {
            active
        } else {
            vec![item]
        }
    }
}

/// The selection a rubber-band rectangle produces: `base` when additive, plus
/// every item whose cell intersects `rect`.
pub(super) fn marquee_selection(
    cells: &[(ItemId, Rect)],
    rect: Rect,
    additive: bool,
    base: &[ItemId],
) -> Vec<ItemId> {
    let mut selection = if additive { base.to_vec() } else { Vec::new() };
    for (item, cell) in cells {
        if cell.intersects(rect) && !selection.contains(item) {
            selection.push(*item);
        }
    }
    selection
}

/// The index of the item nearest `current` in `dir`, from cell centers.
/// Candidates must lie strictly in that direction; the smallest primary-axis
/// distance wins, breaking ties by the cross-axis distance. `None` at the edge
/// of the grid.
fn next_in_direction(cells: &[Point], current: usize, dir: NavDir) -> Option<usize> {
    let origin = cells.get(current)?;
    let mut best: Option<(i32, i32, usize)> = None;
    for (index, cell) in cells.iter().enumerate() {
        if index == current {
            continue;
        }
        let (dx, dy) = (cell.x - origin.x, cell.y - origin.y);
        let (primary, cross) = match dir {
            NavDir::Left if dx < 0 => (-dx, dy.abs()),
            NavDir::Right if dx > 0 => (dx, dy.abs()),
            NavDir::Up if dy < 0 => (-dy, dx.abs()),
            NavDir::Down if dy > 0 => (dy, dx.abs()),
            _ => continue,
        };
        if best.is_none_or(|(best_primary, best_cross, _)| {
            (primary, cross) < (best_primary, best_cross)
        }) {
            best = Some((primary, cross, index));
        }
    }
    best.map(|(_, _, index)| index)
}

/// The index of the next label that starts with `prefix`, searching from
/// `start` and wrapping to the beginning, case-insensitively. `None` when
/// nothing matches. Drives type-to-select.
fn next_prefix_match(labels: &[&str], prefix: &str, start: usize) -> Option<usize> {
    if prefix.is_empty() || labels.is_empty() {
        return None;
    }
    let prefix = prefix.to_lowercase();
    let count = labels.len();
    (0..count)
        .map(|offset| (start + offset) % count)
        .find(|index| labels[*index].to_lowercase().starts_with(&prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::ItemFacts;

    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::Item;
    use deskspace_platform::ShellDisplay;

    fn app_with_selection(space: Option<SpaceId>, item: Option<ItemId>) -> App {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        app.selection.surface = space.map(Surface::Space);
        app.selection.items = item.into_iter().collect();
        app
    }

    #[test]
    fn rubber_band_replaces_the_selection_and_focus() {
        let mut selection = Selection {
            surface: Some(Surface::Space(SpaceId(9))),
            items: vec![ItemId(1)],
            anchor: Some(ItemId(1)),
            focused: Some(ItemId(1)),
            ..Selection::default()
        };
        selection.rubber_band(Surface::Space(SpaceId(2)), vec![ItemId(3), ItemId(4)]);
        assert_eq!(selection.surface, Some(Surface::Space(SpaceId(2))));
        assert_eq!(selection.items, vec![ItemId(3), ItemId(4)]);
        assert_eq!(selection.anchor, None);
        assert_eq!(selection.focused, Some(ItemId(4)));
    }

    #[test]
    fn replace_items_keeps_the_surface_anchor_and_focus() {
        let mut selection = Selection {
            surface: Some(Surface::Desktop),
            items: vec![ItemId(1)],
            anchor: Some(ItemId(1)),
            focused: Some(ItemId(2)),
            ..Selection::default()
        };
        selection.replace_items(vec![ItemId(5), ItemId(6)]);
        assert_eq!(selection.surface, Some(Surface::Desktop));
        assert_eq!(selection.items, vec![ItemId(5), ItemId(6)]);
        assert_eq!(selection.anchor, Some(ItemId(1)));
        assert_eq!(selection.focused, Some(ItemId(2)));
        // Clearing the selected items leaves the anchor and focus alone.
        selection.clear_selected();
        assert!(selection.items.is_empty());
        assert_eq!(selection.anchor, Some(ItemId(1)));
        assert_eq!(selection.focused, Some(ItemId(2)));
    }

    #[test]
    fn type_to_select_matches_a_prefix_and_wraps() {
        let labels = ["Apple", "Banana", "Berry", "Cherry"];
        assert_eq!(next_prefix_match(&labels, "b", 0), Some(1));
        assert_eq!(next_prefix_match(&labels, "be", 0), Some(2));
        // Searching from the current item finds it again; from after the last
        // match it wraps to the first.
        assert_eq!(next_prefix_match(&labels, "b", 2), Some(2));
        assert_eq!(next_prefix_match(&labels, "b", 3), Some(1), "wraps");
        assert_eq!(next_prefix_match(&labels, "c", 0), Some(3));
        assert_eq!(
            next_prefix_match(&labels, "C", 0),
            Some(3),
            "case-insensitive"
        );
        assert_eq!(next_prefix_match(&labels, "z", 0), None);
        assert_eq!(next_prefix_match(&labels, "", 0), None);
        assert_eq!(next_prefix_match(&[], "a", 0), None);
        // Repeated typing of the same letter cycles through the matches.
        assert_eq!(next_prefix_match(&labels, "b", 1), Some(1));
    }

    #[test]
    fn navigation_picks_the_nearest_aligned_item() {
        // A 2x2 grid of cell centers: 0 1 / 2 3.
        let cells = [
            Point::new(0, 0),
            Point::new(100, 0),
            Point::new(0, 100),
            Point::new(100, 100),
        ];
        assert_eq!(next_in_direction(&cells, 0, NavDir::Right), Some(1));
        assert_eq!(next_in_direction(&cells, 1, NavDir::Left), Some(0));
        assert_eq!(next_in_direction(&cells, 0, NavDir::Down), Some(2));
        assert_eq!(next_in_direction(&cells, 3, NavDir::Up), Some(1));
        assert_eq!(next_in_direction(&cells, 2, NavDir::Right), Some(3));
        // The edges of the grid go nowhere.
        assert_eq!(next_in_direction(&cells, 0, NavDir::Left), None);
        assert_eq!(next_in_direction(&cells, 1, NavDir::Right), None);
        assert_eq!(next_in_direction(&cells, 0, NavDir::Up), None);
        assert_eq!(next_in_direction(&cells, 3, NavDir::Down), None);
        assert_eq!(next_in_direction(&[], 0, NavDir::Right), None);
    }

    #[test]
    fn navigation_prefers_the_aligned_neighbor() {
        let cells = [
            Point::new(0, 0),    // 0
            Point::new(100, 40), // 1: diagonal to the right
            Point::new(100, 0),  // 2: aligned to the right
        ];
        assert_eq!(next_in_direction(&cells, 0, NavDir::Right), Some(2));
        // Both are below, but 1 is nearer vertically than 2.
        assert_eq!(next_in_direction(&cells, 0, NavDir::Down), Some(1));
    }

    #[test]
    fn independent_checks_are_the_operand() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        app.shell_display = ShellDisplay {
            check_boxes: CheckBoxMode::Independent,
            ..ShellDisplay::default()
        };
        app.selection.items = vec![ItemId(1)];
        app.selection.checked = vec![ItemId(2), ItemId(3)];
        assert_eq!(app.active_items(), vec![ItemId(2), ItemId(3)]);
        // With nothing ticked the selection is used.
        app.selection.checked.clear();
        assert_eq!(app.active_items(), vec![ItemId(1)]);
    }

    #[test]
    fn ctrl_click_toggles_and_shift_click_selects_a_range() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = app.config.desktop.add_space("S", Rect::new(0, 0, 400, 400));
        for n in 1..=4u64 {
            app.config.desktop.add_item(
                id,
                Item::new(
                    ItemId(n),
                    format!("C:/missing/{n}.txt"),
                    ItemFacts::default(),
                ),
            );
        }
        app.selection.surface = Some(Surface::Space(id));
        // Ctrl+click builds a selection and toggles items off again.
        app.selection.toggle(ItemId(1));
        app.selection.toggle(ItemId(3));
        assert_eq!(app.selection.items, vec![ItemId(1), ItemId(3)]);
        app.selection.toggle(ItemId(1));
        assert_eq!(app.selection.items, vec![ItemId(3)]);
        // Shift+click extends from the anchor across the space's order.
        app.selection.anchor = Some(ItemId(1));
        app.extend_selection(Surface::Space(id), ItemId(4));
        assert_eq!(
            app.selection.items,
            vec![ItemId(1), ItemId(2), ItemId(3), ItemId(4)]
        );
    }

    #[test]
    fn taking_a_selection_reports_the_space_to_repaint() {
        let mut app = app_with_selection(Some(SpaceId(2)), Some(ItemId(5)));
        assert_eq!(app.selection.take(), Some(Surface::Space(SpaceId(2))));
        assert_eq!(
            (app.selection.surface, app.selection.items.len()),
            (None, 0)
        );
        // Nothing is left to clear, so nothing needs repainting.
        assert_eq!(app.selection.take(), None);
    }

    #[test]
    fn a_space_without_a_selected_item_needs_no_repaint() {
        let mut app = app_with_selection(Some(SpaceId(2)), None);
        assert_eq!(app.selection.take(), None);
        assert_eq!(app.selection.surface, None);
    }

    #[test]
    fn marquee_selects_intersecting_cells() {
        let cells = [
            (ItemId(1), Rect::new(0, 0, 10, 10)),
            (ItemId(2), Rect::new(20, 0, 10, 10)),
            (ItemId(3), Rect::new(0, 20, 10, 10)),
        ];
        assert_eq!(
            marquee_selection(&cells, Rect::new(5, 5, 20, 20), false, &[]),
            vec![ItemId(1), ItemId(2), ItemId(3)]
        );
        // A rectangle away from every cell selects nothing.
        assert!(marquee_selection(&cells, Rect::new(100, 100, 5, 5), false, &[]).is_empty());
        // Additive keeps the base selection and appends new hits.
        assert_eq!(
            marquee_selection(&cells, Rect::new(0, 0, 6, 6), true, &[ItemId(2)]),
            vec![ItemId(2), ItemId(1)]
        );
    }
}
