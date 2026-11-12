//! `App` pointer gestures: selection, drag, resize, and the marquee.

mod hover;
mod space_events;

use deskspace_core::geometry::{Point, Rect, Size};
use deskspace_core::{ItemId, ResizeEdges, Space, SpaceId, ViewLayout};

use deskspace_platform::{CheckBoxMode, DragImage, Ui, drag_threshold};

use crate::settings::STATUS_BUTTONS;

use super::drag::Drag;
use super::selection::marquee_selection;

use super::{App, RESIZE_BORDER, Surface};

/// A left press on an already-selected item without Ctrl/Shift, resolved on
/// release if it did not become a drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingClick {
    /// The item was part of a multi-selection: collapse the selection to it,
    /// so dragging one member moves the whole group but a plain click selects
    /// just this item.
    Collapse(ItemId),
    /// The item was already the only selected one: a second click starts an
    /// inline rename, as a slow double-click does in Explorer.
    Rename(ItemId),
}

/// Classifies a plain (no Ctrl/Shift) press on an already-selected item.
fn pending_click(selected: &[ItemId], item: ItemId) -> PendingClick {
    if selected == [item] {
        PendingClick::Rename(item)
    } else {
        PendingClick::Collapse(item)
    }
}

/// What releasing such a press does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingClickOutcome {
    Rename,
    Collapse,
    Nothing,
}

pub(super) fn resolve_pending_click(
    pending: Option<PendingClick>,
    item: ItemId,
    still_selected: bool,
) -> PendingClickOutcome {
    match pending {
        Some(PendingClick::Rename(pending_item)) if pending_item == item && still_selected => {
            PendingClickOutcome::Rename
        }
        Some(PendingClick::Collapse(pending_item)) if pending_item == item && still_selected => {
            PendingClickOutcome::Collapse
        }
        _ => PendingClickOutcome::Nothing,
    }
}

/// The transient pointer state: the drag in progress, what is hovered, the
/// rubber-band rectangle, and the drag image. Other modules read it through
/// these methods rather than its fields, so the hover, the drag, and the drag
/// image stay in step.
#[derive(Default)]
pub(super) struct PointerState {
    pub(super) drag: Drag,
    pub(super) pending_click: Option<PendingClick>,
    /// An item pressed with the right button, not yet dragged. Past the drag
    /// threshold it becomes a shell drag, as a right drag does in Explorer.
    pub(super) right_press: Option<(Surface, ItemId, Point)>,
    /// Whether a right drag ran since the last right press. Its release is
    /// consumed by the shell's drag loop, so DeskSpace must not also open its
    /// own menu on the next right release.
    pub(super) right_dragged: bool,
    pub(super) hovered: Option<(Surface, ItemId)>,
    /// The status-bar view button the pointer is over, and its surface.
    pub(super) status_hover: Option<(Surface, usize)>,
    /// The lens whose title bar the pointer is over, when no item or status
    /// button is: it shows the folder on display as a tip after the usual
    /// delay. A normal space's title shows nothing.
    pub(super) title_hover: Option<SpaceId>,
    /// The space whose title icon button the pointer is over, while its title
    /// is being renamed (D16).
    pub(super) title_icon_hover: Option<SpaceId>,
    pub(super) dragged: Option<(Surface, Vec<ItemId>)>,
    pub(super) marquee: Option<(Surface, Rect)>,
    pub(super) drag_image: Option<DragImage>,
    /// The space whose Back button took the last left press. The press may
    /// have gone back to the lens's own folder, where the button is gone, so
    /// the double-click that follows finds it here instead of folding the
    /// space.
    pub(super) back_press: Option<SpaceId>,
}

impl PointerState {
    /// The dragged items on `surface`, empty when another surface is dragged.
    pub(super) fn dragged_on(&self, surface: Surface) -> Vec<ItemId> {
        self.dragged
            .as_ref()
            .filter(|(owner, _)| *owner == surface)
            .map(|(_, items)| items.clone())
            .unwrap_or_default()
    }

    /// The rubber-band rectangle on `surface`, in desktop coordinates.
    pub(super) fn marquee_on(&self, surface: Surface) -> Option<Rect> {
        self.marquee
            .filter(|(owner, _)| *owner == surface)
            .map(|(_, rect)| rect)
    }
}

/// A chrome target that may claim a press on a space.
type SpacePressTarget = fn(&mut App, &mut Ui, SpaceId, Point) -> bool;

/// The chrome targets a press on a space may claim, in the order they are
/// asked. A divider is checked before the header, and the Back button before
/// the title bar can start a move.
const SPACE_PRESS_TARGETS: [SpacePressTarget; 5] = [
    // A click on a browsed lens's Back button shows the parent folder, before
    // the title bar can start a move.
    App::back_button_clicked,
    // A click on a status-bar view button switches this space's view.
    App::status_button_clicked,
    // A press on a Details divider resizes its column, so it must not also
    // sort.
    App::begin_column_resize,
    // A press on a Details header waits for its release: a click sorts, and a
    // drag moves the column.
    App::begin_header_press,
    // A press on a group's header folds or opens the group.
    App::group_header_clicked,
];

impl App {
    fn on_capture_lost(&mut self, ui: &mut Ui) {
        self.pointer.right_press = None;
        self.pointer.right_dragged = false;
        self.cancel_column_gesture(ui);
        self.end_drag(ui);
    }

    /// If `point` is on space `id`'s Back button, makes the lens current,
    /// shows the parent of the subfolder it is browsed into, and returns true
    /// (D18). A resize edge under the button wins, as the cursor shows.
    fn back_button_clicked(&mut self, ui: &mut Ui, id: SpaceId, point: Point) -> bool {
        let shown = self.is_browsed(id) || self.pointer.back_press == Some(id);
        let on_button = self.config.desktop.space(id).is_some_and(|space| {
            space.resize_edges_at(point, RESIZE_BORDER) == ResizeEdges::NONE
                && crate::scene::back_button_at(space, shown, point)
        });
        if !on_button {
            return false;
        }
        self.pointer.back_press = Some(id);
        if let Some(previous) = self.selection.enter(Surface::Space(id)) {
            self.refresh_surface(ui, previous);
        }
        self.browse_up(ui, id);
        true
    }

    fn on_back_button(&mut self, ui: &mut Ui, surface: Surface) {
        if let Surface::Space(id) = surface {
            self.browse_up(ui, id);
        }
    }

    /// If `point` is on a status-bar view button, switches the space to that
    /// view and returns true.
    fn status_button_clicked(&mut self, ui: &mut Ui, id: SpaceId, point: Point) -> bool {
        let Some(space) = self.config.desktop.space(id) else {
            return false;
        };
        let Some(index) = crate::scene::status_button_at(space, point) else {
            return false;
        };
        let Some(button) = STATUS_BUTTONS.get(index) else {
            return false;
        };
        let view = space.view().with_mode(button.mode);
        let at = space.rect().origin();
        self.set_space_view(ui, id, view, at);
        true
    }

    pub(super) fn begin_interaction(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        point: Point,
        ctrl: bool,
        shift: bool,
    ) {
        if let Surface::Space(id) = surface
            && SPACE_PRESS_TARGETS
                .iter()
                .any(|claim| claim(self, ui, id, point))
        {
            return;
        }
        if self.checkbox_pressed(ui, surface, point) {
            return;
        }
        let previous = self.selection.enter(surface);
        self.pointer.pending_click = None;
        let item = self.surface_item_at(surface, point);
        match item {
            Some(item) => {
                if shift {
                    self.extend_selection(surface, item);
                    self.selection.focus(item);
                } else if ctrl {
                    self.selection.toggle(item);
                    self.selection.focus(item);
                } else if self.selection.is_selected(item) {
                    // Keep the whole selection so a drag of one member moves
                    // the group. On release, collapse it and maybe rename.
                    self.pointer.pending_click = Some(pending_click(&self.selection.items, item));
                    self.selection.focus(item);
                } else {
                    self.selection.select_only(surface, item);
                }
            }
            None => {
                if !ctrl && !shift {
                    self.selection.deselect();
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
                    self.pointer.drag = Drag::Resize {
                        id,
                        edges,
                        origin: point,
                        active_x: false,
                        active_y: false,
                    };
                    self.selection.clear_selected();
                } else if let Some(item) = item {
                    self.pointer.drag = Drag::Item {
                        surface,
                        item,
                        origin: point,
                        active: false,
                    };
                } else if !space.locked() && space.title_bar().contains(point) {
                    self.pointer.drag = Drag::Space {
                        id,
                        grab: Point::new(point.x - space.rect().x, point.y - space.rect().y),
                    };
                } else if space.content_rect().contains(point) {
                    // Empty content: start a rubber-band selection.
                    self.pointer.drag = Drag::Marquee {
                        surface,
                        origin: point,
                        additive: ctrl || shift,
                    };
                }
            }
            Surface::Desktop => {
                self.pointer.drag = match item {
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
        if let Some(previous) = previous {
            self.refresh_surface(ui, previous);
        }
        self.refresh_surface(ui, surface);
    }

    /// A click on an item's check box either toggles that item in the
    /// selection (auto-check-select) or the independent tick set, and moves the
    /// highlight to it.
    fn checkbox_pressed(&mut self, ui: &mut Ui, surface: Surface, point: Point) -> bool {
        let Some(item) = self.checkbox_at(surface, point) else {
            return false;
        };
        let previous = self.selection.enter(surface);
        if self.surface_check_box_mode(surface) == CheckBoxMode::Independent {
            self.selection.toggle_checked(item);
            self.selection.select_only(surface, item);
        } else {
            self.selection.toggle(item);
            self.selection.focus(item);
        }
        if let Some(previous) = previous {
            self.refresh_surface(ui, previous);
        }
        self.refresh_surface(ui, surface);
        true
    }

    fn begin_right_press(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        self.pointer.right_dragged = false;
        // A row can be scrolled under the header, and a press on the
        // header is for the header's menu, so it leaves the selection.
        if self.details_header_column(surface, point).is_none() {
            self.select_at(ui, surface, point);
            self.pointer.right_press = self
                .surface_item_at(surface, point)
                .map(|item| (surface, item, point));
        }
    }

    /// Selects the item under `point` (or nothing) without starting a drag,
    /// as a right-click does before its menu opens. Right-clicking an item
    /// that is already part of a multi-selection keeps the selection.
    pub(super) fn select_at(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let previous = self.selection.enter(surface);
        let item = self.surface_item_at(surface, point);
        match item {
            Some(item) if !self.selection.is_selected(item) => {
                self.selection.select_only(surface, item);
            }
            Some(item) => self.selection.focused = Some(item),
            None => self.selection.deselect(),
        }
        if let Some(previous) = previous {
            self.refresh_surface(ui, previous);
        }
        self.refresh_surface(ui, surface);
    }

    /// Starts a shell drag when the right button has been held down and the
    /// pointer moved past the drag threshold: Explorer's right-drag, which
    /// never rearranges inside the surface, and whose release the shell's own
    /// Move / Copy / Create shortcut menu handles. Returns true once the shell
    /// has taken over the gesture.
    pub(super) fn right_drag_to(&mut self, ui: &mut Ui, point: Point) -> bool {
        let Some((surface, item, press)) = self.pointer.right_press else {
            return false;
        };
        if !right_drag_started(press, point, drag_threshold()) {
            return false;
        }
        self.pointer.right_press = None;
        self.pointer.right_dragged = true;
        // Without our image the shell substitutes its own oversized one.
        let ids = self.drag_items(item);
        self.pointer.drag_image = self.drag_image_of(surface, &ids, press);
        self.drag_item_out(ui, surface, item);
        self.pointer.drag_image = None;
        true
    }

    fn end_right_press(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        self.pointer.right_press = None;
        // A right drag ends in the shell's own Move / Copy / Create shortcut
        // menu, so only a plain right click opens DeskSpace's.
        if self.pointer.right_dragged {
            self.pointer.right_dragged = false;
        } else if let (Surface::Space(id), Some(_)) =
            (surface, self.details_header_column(surface, point))
        {
            self.show_details_header_menu(ui, id, point);
        } else {
            match self.surface_item_at(surface, point) {
                Some(item) => self.show_item_menu(ui, surface, item, point),
                None => self.show_surface_menu(ui, surface, point),
            }
        }
    }

    fn on_pointer_move(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        if !self.right_drag_to(ui, point) && !self.drag_to(ui, point) {
            if let Surface::Space(id) = surface {
                self.update_cursor(ui, id, point);
            }
            self.update_hover(ui, surface, point);
        }
    }

    /// A double-click's second press arrives as the double-click alone, so on
    /// the Back button it goes up again, as two clicks would, rather than fold
    /// the space.
    fn on_double_click(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let back = matches!(surface, Surface::Space(id) if self.back_button_clicked(ui, id, point));
        if !back {
            self.double_click(ui, surface, point);
        }
    }

    pub(super) fn double_click(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let target = match surface {
            Surface::Space(id) => match self.view_layout(id) {
                Some(layout) => {
                    self.config
                        .desktop
                        .space(id)
                        .map_or(DoubleClickTarget::Nothing, |space| {
                            double_click_target(space, &layout, self.surface_scroll(surface), point)
                        })
                }
                None => DoubleClickTarget::Nothing,
            },
            Surface::Desktop => self
                .surface_item_at(surface, point)
                .map_or(DoubleClickTarget::Nothing, DoubleClickTarget::Item),
        };
        tracing::debug!(?surface, ?point, ?target, "double-click");
        match target {
            DoubleClickTarget::TitleBar => {
                if let Surface::Space(id) = surface {
                    self.toggle_fold(ui, id);
                }
            }
            DoubleClickTarget::Item(item) => self.open_item(ui, item),
            DoubleClickTarget::Nothing => {}
        }
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
        self.pointer.marquee = Some((surface, rect));

        let items = marquee_selection(&cells, rect, additive, &self.selection.items);
        self.selection.rubber_band(surface, items);
        self.refresh_surface(ui, surface);
    }
}

/// Whether a right press at `press` has moved far enough to `now` to start a
/// drag, using the same threshold as the left-button drag.
fn right_drag_started(press: Point, now: Point, threshold: Size) -> bool {
    (now.x - press.x).abs() >= threshold.width || (now.y - press.y).abs() >= threshold.height
}

/// The movement that starts a resize on each axis: half an item's shorter
/// side, so a wide inline cell does not make the grab sluggish, and never
/// less than half the fine `snap` step. A row view has no cell width to
/// measure, so its row height sets the zone.
pub(super) fn resize_dead_zone(cell: Size, snap: i32) -> Size {
    let side = if cell.width > 1 {
        cell.width.min(cell.height)
    } else {
        cell.height
    }
    .max(1);
    let zone = (side / 2).max((snap / 2).max(1));
    Size::new(zone, zone)
}

/// The edges a resize moves right now. An axis joins only once the pointer has
/// left the dead zone around the grab point, so a click or a tiny drag on a
/// border never resizes the space; once joined, the axis stays active so its
/// edge tracks the pointer back. Returns the effective edges and the updated
/// `(active_x, active_y)` flags.
pub(super) fn resize_edges_now(
    edges: ResizeEdges,
    origin: Point,
    point: Point,
    dead_zone: Size,
    active: (bool, bool),
) -> (ResizeEdges, (bool, bool)) {
    let active_x =
        active.0 || ((edges.left || edges.right) && (point.x - origin.x).abs() > dead_zone.width);
    let active_y =
        active.1 || ((edges.top || edges.bottom) && (point.y - origin.y).abs() > dead_zone.height);
    let edges = ResizeEdges {
        left: edges.left && active_x,
        right: edges.right && active_x,
        top: edges.top && active_y,
        bottom: edges.bottom && active_y,
    };
    (edges, (active_x, active_y))
}

/// What a double-click at `point` (desktop coordinates) acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoubleClickTarget {
    /// Folds or unfolds the space.
    TitleBar,
    /// Opens the item.
    Item(ItemId),
    Nothing,
}

fn double_click_target(
    space: &Space,
    layout: &ViewLayout,
    scroll: i32,
    point: Point,
) -> DoubleClickTarget {
    if space.title_bar().contains(point) {
        DoubleClickTarget::TitleBar
    } else {
        let content = space.content_rect();
        let local =
            layout.layout_point(Point::new(point.x - content.x, point.y - content.y), scroll);
        layout
            .index_at(local)
            .and_then(|index| space.items().get(index))
            .map_or(DoubleClickTarget::Nothing, |item| {
                DoubleClickTarget::Item(item.id)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::ItemFacts;

    use deskspace_core::GridSpec;
    use deskspace_core::view_layout::ViewMetrics;

    #[test]
    fn a_second_click_on_the_only_selected_item_renames() {
        assert_eq!(
            pending_click(&[ItemId(1)], ItemId(1)),
            PendingClick::Rename(ItemId(1))
        );
        // Part of a multi-selection (or no selection) collapses instead.
        assert_eq!(
            pending_click(&[ItemId(1), ItemId(2)], ItemId(1)),
            PendingClick::Collapse(ItemId(1))
        );
        assert_eq!(
            pending_click(&[], ItemId(1)),
            PendingClick::Collapse(ItemId(1))
        );
    }

    #[test]
    fn releasing_a_second_click_starts_a_rename() {
        assert_eq!(
            resolve_pending_click(Some(PendingClick::Rename(ItemId(1))), ItemId(1), true),
            PendingClickOutcome::Rename
        );
        assert_eq!(
            resolve_pending_click(Some(PendingClick::Collapse(ItemId(1))), ItemId(1), true),
            PendingClickOutcome::Collapse
        );
        // A press on some other item, or an item that is no longer selected,
        // does nothing.
        assert_eq!(
            resolve_pending_click(Some(PendingClick::Rename(ItemId(1))), ItemId(2), true),
            PendingClickOutcome::Nothing
        );
        assert_eq!(
            resolve_pending_click(Some(PendingClick::Rename(ItemId(1))), ItemId(1), false),
            PendingClickOutcome::Nothing
        );
        assert_eq!(
            resolve_pending_click(None, ItemId(1), true),
            PendingClickOutcome::Nothing
        );
    }

    #[test]
    fn a_right_drag_starts_past_the_threshold_on_either_axis() {
        let threshold = Size::new(4, 4);
        let press = Point::new(100, 100);
        // Inside the threshold: still a click.
        assert!(!right_drag_started(press, Point::new(102, 102), threshold));
        // Reaching the threshold on one axis starts the drag.
        assert!(right_drag_started(press, Point::new(104, 100), threshold));
        assert!(right_drag_started(press, Point::new(100, 104), threshold));
        assert!(right_drag_started(press, Point::new(96, 100), threshold));
        assert!(right_drag_started(press, Point::new(100, 96), threshold));
    }

    #[test]
    fn the_dead_zone_is_half_a_cells_shorter_side_and_at_least_half_the_snap() {
        assert_eq!(resize_dead_zone(Size::new(200, 22), 16), Size::new(11, 11));
        assert_eq!(resize_dead_zone(Size::new(1, 22), 16), Size::new(11, 11));
        assert_eq!(resize_dead_zone(Size::new(96, 104), 16), Size::new(48, 48));
        assert_eq!(resize_dead_zone(Size::new(1, 4), 16), Size::new(8, 8));
    }

    #[test]
    fn resize_waits_out_the_dead_zone() {
        let zone = resize_dead_zone(Size::new(80, 100), 16);
        let grabbed = ResizeEdges {
            right: true,
            ..ResizeEdges::NONE
        };
        let origin = Point::new(200, 100);
        // Inside the dead zone (half of 80 is 40): nothing moves.
        let (edges, active) =
            resize_edges_now(grabbed, origin, Point::new(230, 100), zone, (false, false));
        assert_eq!(edges, ResizeEdges::NONE);
        assert_eq!(active, (false, false));
        // Past it, the axis joins.
        let (edges, active) =
            resize_edges_now(grabbed, origin, Point::new(241, 100), zone, (false, false));
        assert!(edges.right);
        assert_eq!(active, (true, false));
    }

    #[test]
    fn a_corner_resizes_only_the_axis_that_moved() {
        let zone = resize_dead_zone(Size::new(80, 100), 16);
        let corner = ResizeEdges {
            right: true,
            bottom: true,
            ..ResizeEdges::NONE
        };
        let origin = Point::new(200, 100);
        // Horizontal movement only: width resizes, height stays.
        let (edges, active) =
            resize_edges_now(corner, origin, Point::new(260, 110), zone, (false, false));
        assert!(edges.right && !edges.bottom);
        assert_eq!(active, (true, false));
        // A vertical move then joins the bottom too.
        let (edges, active) =
            resize_edges_now(corner, origin, Point::new(260, 200), zone, (true, false));
        assert!(edges.right && edges.bottom);
        assert_eq!(active, (true, true));
    }

    #[test]
    fn a_joined_axis_stays_active_inside_the_dead_zone() {
        let zone = resize_dead_zone(Size::new(80, 100), 16);
        let grabbed = ResizeEdges {
            right: true,
            ..ResizeEdges::NONE
        };
        let origin = Point::new(200, 100);
        // Back at the grab point, the edge still tracks rather than stopping.
        let (edges, active) =
            resize_edges_now(grabbed, origin, Point::new(200, 100), zone, (true, false));
        assert!(edges.right);
        assert_eq!(active, (true, false));
    }

    #[test]
    fn double_click_rolls_on_the_title_and_opens_items() {
        let grid = GridSpec::default();
        let mut space = Space::new(SpaceId(1), "S", Rect::new(100, 100, 240, 400));
        let mut item = deskspace_core::Item::new(ItemId(7), "C:/a.lnk", ItemFacts::default());
        item.position = Point::new(12, 12);
        space.push_item(item);
        let layout = ViewLayout::for_space(&space, ViewMetrics::for_grid(grid));
        let target = |x, y, scroll| double_click_target(&space, &layout, scroll, Point::new(x, y));
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
