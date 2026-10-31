//! `App` Details columns: dragging a header's divider to resize a column, and
//! the header itself to reorder it. A click that never becomes a drag sorts,
//! so sorting waits for the release.
//!
//! Name takes the width the other columns leave, so a divider cannot simply
//! move: widening a column would shrink Name and leave the divider behind the
//! pointer. So Name is held at its width for the drag and takes the rest again
//! on release (D18). The divider right of Name resizes the next column
//! instead, since Name has no width of its own to set.

use deskspace_core::geometry::Point;
use deskspace_core::{Column, Command, SpaceId, SpaceView, ViewMode};
use deskspace_platform::{DetailsColumnVisual, DetailsView, Ui, drag_threshold, space_dpi};

use super::drag::Drag;
use super::{App, RESIZE_BORDER, Surface};

/// How far from a divider a press still grabs it, each way, at 96 DPI.
/// Provisional until W31's on-screen check compares it with Explorer's.
const DIVIDER_SLOP: i32 = 4;

/// The shown column whose right edge is the divider at `x`, relative to the
/// content area. A space showing only Name has no divider, since nothing can
/// give Name more or less room than the rest.
pub(crate) fn divider_at(x: i32, columns: &[DetailsColumnVisual], scale: f32) -> Option<usize> {
    if columns.len() < 2 {
        return None;
    }
    let slop = (DIVIDER_SLOP as f32 * scale.max(0.1)).round() as i32;
    columns
        .iter()
        .position(|column| (x - column.rect.right()).abs() <= slop)
}

/// The index to insert a dragged column at when the pointer is at `x`, relative
/// to the content area: before the first column whose middle is past `x`, and
/// never before Name (D18).
pub(crate) fn column_drop_index(x: i32, columns: &[DetailsColumnVisual]) -> usize {
    (1..columns.len())
        .find(|index| x < columns[*index].rect.center().x)
        .unwrap_or_else(|| columns.len().max(1))
}

/// A left press on a header. Sorting or moving is decided on release.
#[derive(Clone, Copy)]
pub(super) struct HeaderPress {
    id: SpaceId,
    column: Column,
    origin: Point,
    /// Whether the pointer has passed the drag threshold, so the release is no
    /// longer a click.
    dragged: bool,
    /// Where the column would land. Name never has one, since it cannot move.
    drop: Option<usize>,
}

/// What releasing a header press does.
#[derive(Debug, PartialEq, Eq)]
enum HeaderRelease {
    /// A click: sort by the column at this place.
    Sort(usize),
    /// A drag: show the view with the column moved.
    Reorder(SpaceView),
    Nothing,
}

/// Decides a release against the view as it is now, so a column that has gone
/// since the press does nothing.
fn header_release(press: &HeaderPress, view: &SpaceView) -> HeaderRelease {
    let Some(index) = view.columns.iter().position(|spec| spec.id == press.column) else {
        return HeaderRelease::Nothing;
    };
    if !press.dragged {
        return HeaderRelease::Sort(index);
    }
    let Some(insert_at) = press.drop else {
        return HeaderRelease::Nothing;
    };
    let moved = view.with_column_moved(press.column, insert_at);
    if moved == *view {
        HeaderRelease::Nothing
    } else {
        HeaderRelease::Reorder(moved)
    }
}

/// A divider drag in progress. It holds the column by id, and notices when
/// anything else has changed the view, so it cancels rather than overwrite.
pub(super) struct ColumnResize {
    id: SpaceId,
    /// The column whose width the drag sets.
    column: Column,
    /// That column's width in pixels when the button went down.
    start: i32,
    /// Whether moving right widens it. The divider right of Name narrows it.
    grows_rightward: bool,
    /// Name's width at 96 DPI for the drag, when Name has to hold still.
    held_name: Option<u16>,
    /// Where the button went down, in the same coordinates as the drag.
    origin: i32,
    scale: f32,
    /// The view to put back on Esc.
    before: SpaceView,
    /// The view this drag last applied.
    applied: SpaceView,
}

impl ColumnResize {
    /// The drag that `divider` starts in `view`, drawn as `columns`. `None`
    /// when the view and the drawn columns no longer agree.
    fn new(
        id: SpaceId,
        view: &SpaceView,
        columns: &[DetailsColumnVisual],
        divider: usize,
        origin: i32,
        scale: f32,
    ) -> Option<Self> {
        let scale = scale.max(0.1);
        let beside_name = divider == 0;
        let target = if beside_name { 1 } else { divider };
        Some(Self {
            id,
            column: view.columns.get(target)?.id,
            start: columns.get(target)?.rect.width,
            grows_rightward: !beside_name,
            held_name: (!beside_name).then(|| logical(columns[0].rect.width, scale)),
            origin,
            scale,
            before: view.clone(),
            applied: view.clone(),
        })
    }

    /// The view with the pointer at `x`.
    fn view_at(&self, x: i32) -> SpaceView {
        let moved = x - self.origin;
        // A press that has not moved, or a drag back to where it began,
        // changes nothing, so it records no undo entry.
        if moved == 0 {
            return self.before.clone();
        }
        let width = self.start + if self.grows_rightward { moved } else { -moved };
        self.before
            .with_column_width(self.column, logical(width, self.scale))
            .with_name_held(self.held_name)
    }
}

/// `pixels` at 96 DPI, in the range a column's width can hold.
fn logical(pixels: i32, scale: f32) -> u16 {
    (pixels as f32 / scale)
        .round()
        .clamp(0.0, f32::from(u16::MAX)) as u16
}

impl App {
    /// The Details header drawn in space `id`, when it shows one.
    fn details_header(&self, id: SpaceId) -> Option<DetailsView> {
        let space = self.config.desktop.space(id)?;
        let layout = self.view_layout(id)?;
        crate::scene::details_view(
            space,
            &layout,
            space.content_rect().width,
            self.layout_for(id).padding,
            space_dpi(id.0),
        )
    }

    /// The Details header drawn in space `id`, and `point` relative to the
    /// content area, when `point` is on the header row.
    fn details_header_at(&self, id: SpaceId, point: Point) -> Option<(DetailsView, Point)> {
        let space = self.config.desktop.space(id)?;
        let content = space.content_rect();
        if space.view().mode != ViewMode::Details || !content.contains(point) {
            return None;
        }
        let local = Point::new(point.x - content.x, point.y - content.y);
        // Checked before building the header, which runs on every pointer move.
        if local.y >= self.view_layout(id)?.header_height() {
            return None;
        }
        Some((self.details_header(id)?, local))
    }

    /// The shown Details column whose header holds `point` on `surface`, if
    /// any. The desktop layer has no header.
    pub(super) fn details_header_column(&self, surface: Surface, point: Point) -> Option<usize> {
        let Surface::Space(id) = surface else {
            return None;
        };
        let (details, local) = self.details_header_at(id, point)?;
        crate::scene::details_header_column_at(&details, local)
    }

    /// The divider `point` is on, leaving the space's own resize border to the
    /// space.
    pub(super) fn details_divider_at(&self, id: SpaceId, point: Point) -> Option<usize> {
        let space = self.config.desktop.space(id)?;
        if space.resize_edges_at(point, RESIZE_BORDER).any() {
            return None;
        }
        let (details, local) = self.details_header_at(id, point)?;
        divider_at(local.x, &details.columns, space_dpi(id.0))
    }

    /// Makes space `id` current so the keyboard hook reports Esc during a
    /// column gesture. The selection stays unless it was on another surface.
    fn make_current_for_column_gesture(&mut self, ui: &mut Ui, id: SpaceId) {
        let surface = Surface::Space(id);
        let previous = self.selection.surface.replace(surface);
        if let Some(previous) = previous.filter(|previous| *previous != surface) {
            self.selection.clear_items();
            self.refresh_surface(ui, previous);
        }
    }

    /// Starts a header press if `point` is on a header, leaving the space's own
    /// resize border to the space.
    pub(super) fn begin_header_press(&mut self, ui: &mut Ui, id: SpaceId, point: Point) -> bool {
        let Some(index) = self.details_header_column(Surface::Space(id), point) else {
            return false;
        };
        let Some(space) = self.config.desktop.space(id) else {
            return false;
        };
        if space.resize_edges_at(point, RESIZE_BORDER).any() {
            return false;
        }
        let Some(column) = space.view().columns.get(index).map(|spec| spec.id) else {
            return false;
        };
        self.pointer.drag = Drag::HeaderPress(HeaderPress {
            id,
            column,
            origin: point,
            dragged: false,
            drop: None,
        });
        self.make_current_for_column_gesture(ui, id);
        true
    }

    /// Follows a header press: past the drag threshold it stops being a click,
    /// and a column other than Name shows where it would land. The columns do
    /// not change until the release.
    pub(super) fn drag_header_to(&mut self, ui: &mut Ui, point: Point) {
        let Drag::HeaderPress(mut press) = self.pointer.drag else {
            return;
        };
        let threshold = drag_threshold();
        if !press.dragged
            && (point.x - press.origin.x).abs() < threshold.width
            && (point.y - press.origin.y).abs() < threshold.height
        {
            return;
        }
        press.dragged = true;
        let Some(details) = self.details_header(press.id) else {
            self.cancel_column_gesture(ui);
            return;
        };
        let Some(space) = self.config.desktop.space(press.id) else {
            self.cancel_column_gesture(ui);
            return;
        };
        let local = point.x - space.content_rect().x;
        let drop =
            (press.column != Column::Name).then(|| column_drop_index(local, &details.columns));
        let changed = drop != press.drop;
        press.drop = drop;
        self.pointer.drag = Drag::HeaderPress(press);
        if changed {
            self.refresh_space(ui, press.id);
        }
    }

    /// Ends a header press: a click sorts, as Explorer does, reversing the
    /// column already sorted, and a drag moves the column as one undo entry.
    pub(super) fn finish_header_press(&mut self, ui: &mut Ui, press: HeaderPress) {
        let id = press.id;
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let view = space.view();
        let at = space.rect().origin();
        match header_release(&press, view) {
            HeaderRelease::Sort(index) => {
                let rule = crate::scene::details_rule_after(view, index);
                self.sort_space(ui, id, rule, at);
            }
            HeaderRelease::Reorder(view) => {
                self.set_space_view_labelled(ui, id, view, at, "Reorder columns");
            }
            HeaderRelease::Nothing => {
                if press.drop.is_some() {
                    self.refresh_space(ui, id);
                }
            }
        }
    }

    /// Where a dragged header on `surface` would land, for the scene.
    pub(super) fn column_drop_on(&self, surface: Surface) -> Option<usize> {
        match self.pointer.drag {
            Drag::HeaderPress(press) if surface == Surface::Space(press.id) => press.drop,
            _ => None,
        }
    }

    /// Starts resizing a column if `point` is on a divider.
    pub(super) fn begin_column_resize(&mut self, ui: &mut Ui, id: SpaceId, point: Point) -> bool {
        let Some(divider) = self.details_divider_at(id, point) else {
            return false;
        };
        let Some((details, _)) = self.details_header_at(id, point) else {
            return false;
        };
        let Some(space) = self.config.desktop.space(id) else {
            return false;
        };
        let Some(resize) = ColumnResize::new(
            id,
            space.view(),
            &details.columns,
            divider,
            point.x,
            space_dpi(id.0),
        ) else {
            return false;
        };
        self.pointer.drag = Drag::ColumnResize(Box::new(resize));
        self.make_current_for_column_gesture(ui, id);
        true
    }

    /// Follows the pointer: the rows are laid out again, without saving, and
    /// the release records one undo entry and saves once.
    pub(super) fn resize_column_to(&mut self, ui: &mut Ui, point: Point) {
        let Drag::ColumnResize(resize) = &self.pointer.drag else {
            return;
        };
        let id = resize.id;
        let untouched = self
            .config
            .desktop
            .space(id)
            .is_some_and(|space| *space.view() == resize.applied);
        if !untouched {
            self.cancel_column_gesture(ui);
            return;
        }
        let next = resize.view_at(point.x);
        if next == resize.applied {
            return;
        }
        self.undo.begin_if_none("Resize column");
        // Not `commit`: runs on every pointer move; `end_drag` saves once.
        let command = Command::SetSpaceView {
            id,
            view: next.clone(),
        };
        if self.dispatch(command, point).is_applied() {
            if let Drag::ColumnResize(resize) = &mut self.pointer.drag {
                resize.applied = next;
            }
            self.refresh_space(ui, id);
        }
    }

    /// Ends a drag on release: Name takes the rest again, as one undo entry.
    pub(super) fn finish_column_resize(&mut self, ui: &mut Ui, resize: ColumnResize) {
        let id = resize.id;
        if resize.applied == resize.before {
            self.undo.cancel();
            return;
        }
        let untouched = self
            .config
            .desktop
            .space(id)
            .is_some_and(|space| *space.view() == resize.applied);
        if !untouched {
            self.undo.cancel();
            return;
        }
        let view = resize.applied.with_name_held(None);
        let at = self.space_center(id);
        self.dispatch(Command::SetSpaceView { id, view }, at);
        self.undo.finish(&self.config.desktop);
        self.refresh_space(ui, id);
        self.resnapshot_settings(ui, id);
        self.schedule_save(ui);
    }

    /// Cancels a column gesture, as Esc does and as losing the mouse does: a
    /// header press does nothing, and a resize puts the view back unless
    /// something else has changed it since. Returns whether there was one.
    pub(super) fn cancel_column_gesture(&mut self, ui: &mut Ui) -> bool {
        if let Drag::HeaderPress(press) = self.pointer.drag {
            self.pointer.drag = Drag::None;
            if press.drop.is_some() {
                self.refresh_space(ui, press.id);
            }
            return true;
        }
        if !matches!(self.pointer.drag, Drag::ColumnResize(_)) {
            return false;
        }
        let Drag::ColumnResize(resize) = std::mem::take(&mut self.pointer.drag) else {
            return false;
        };
        let id = resize.id;
        let current = self.config.desktop.space(id).map(|space| space.view());
        if resize.applied != resize.before && current == Some(&resize.applied) {
            let at = self.space_center(id);
            let view = resize.before.clone();
            self.dispatch(Command::SetSpaceView { id, view }, at);
            self.refresh_space(ui, id);
            self.schedule_save(ui);
        }
        self.undo.cancel();
        true
    }
}

#[cfg(test)]
mod tests {
    use deskspace_core::{ColumnSpec, Rect};
    use deskspace_platform::ColumnAlign;

    use super::*;

    /// Name 300 wide from x = 12, then Date 130, Type 100, Size 80.
    fn columns() -> Vec<DetailsColumnVisual> {
        let mut x = 12;
        [300, 130, 100, 80]
            .into_iter()
            .map(|width| {
                let rect = Rect::new(x, 0, width, 25);
                x += width;
                DetailsColumnVisual {
                    rect,
                    label: String::new(),
                    align: ColumnAlign::Left,
                }
            })
            .collect()
    }

    fn resize(divider: usize, scale: f32) -> ColumnResize {
        ColumnResize::new(
            SpaceId(1),
            &SpaceView::default(),
            &columns(),
            divider,
            500,
            scale,
        )
        .unwrap()
    }

    fn width_of(view: &SpaceView, column: Column) -> Option<u16> {
        view.columns
            .iter()
            .find(|spec| spec.id == column)
            .and_then(|spec| spec.width)
    }

    fn press(column: Column, dragged: bool, drop: Option<usize>) -> HeaderPress {
        HeaderPress {
            id: SpaceId(1),
            column,
            origin: Point::new(0, 0),
            dragged,
            drop,
        }
    }

    #[test]
    fn the_drop_index_is_the_nearest_gap_and_never_before_name() {
        let columns = columns();
        // Name 12-312, Date 312-442, Type 442-542, Size 542-622.
        assert_eq!(column_drop_index(0, &columns), 1, "left of everything");
        assert_eq!(column_drop_index(100, &columns), 1, "over Name");
        assert_eq!(column_drop_index(376, &columns), 1, "Date's left half");
        assert_eq!(column_drop_index(377, &columns), 2, "Date's right half");
        assert_eq!(column_drop_index(500, &columns), 3, "Type's right half");
        assert_eq!(column_drop_index(590, &columns), 4, "Size's right half");
        assert_eq!(column_drop_index(5000, &columns), 4, "past the last column");
    }

    #[test]
    fn showing_only_name_drops_after_it() {
        let mut columns = columns();
        columns.truncate(1);
        assert_eq!(column_drop_index(5000, &columns), 1);
    }

    #[test]
    fn a_click_sorts_by_the_column_at_its_place_now() {
        let view = SpaceView::default();
        assert_eq!(
            header_release(&press(Column::Kind, false, None), &view),
            HeaderRelease::Sort(2)
        );
        assert_eq!(
            header_release(&press(Column::Name, false, None), &view),
            HeaderRelease::Sort(0),
            "Name still sorts"
        );
    }

    #[test]
    fn a_drag_moves_the_column_and_does_not_sort() {
        let view = SpaceView::default();
        assert_eq!(
            header_release(&press(Column::Size, true, Some(1)), &view),
            HeaderRelease::Reorder(view.with_column_moved(Column::Size, 1))
        );
    }

    #[test]
    fn name_never_moves_and_a_drag_of_it_does_not_sort() {
        assert_eq!(
            header_release(&press(Column::Name, true, None), &SpaceView::default()),
            HeaderRelease::Nothing
        );
    }

    #[test]
    fn a_drag_back_to_its_place_does_nothing() {
        let view = SpaceView::default();
        for drop in [2, 3] {
            assert_eq!(
                header_release(&press(Column::Kind, true, Some(drop)), &view),
                HeaderRelease::Nothing,
                "{drop}"
            );
        }
    }

    #[test]
    fn a_column_gone_since_the_press_does_nothing() {
        let view = SpaceView::default().with_column_toggled(Column::Kind);
        for gesture in [
            press(Column::Kind, false, None),
            press(Column::Kind, true, Some(1)),
        ] {
            assert_eq!(header_release(&gesture, &view), HeaderRelease::Nothing);
        }
    }

    #[test]
    fn a_divider_is_hit_at_each_edge_of_its_slop_and_not_in_between() {
        let columns = columns();
        let name_edge = columns[0].rect.right();
        assert_eq!(divider_at(name_edge, &columns, 1.0), Some(0));
        assert_eq!(divider_at(name_edge - 4, &columns, 1.0), Some(0));
        assert_eq!(divider_at(name_edge + 4, &columns, 1.0), Some(0));
        assert_eq!(divider_at(name_edge - 5, &columns, 1.0), None);
        assert_eq!(divider_at(name_edge + 5, &columns, 1.0), None);
        let last = columns[3].rect.right();
        assert_eq!(divider_at(last, &columns, 1.0), Some(3), "the last column");
        assert_eq!(divider_at(columns[0].rect.x, &columns, 1.0), None);
    }

    #[test]
    fn the_slop_scales_with_the_display() {
        let columns = columns();
        let edge = columns[1].rect.right();
        assert_eq!(divider_at(edge + 6, &columns, 1.0), None);
        assert_eq!(divider_at(edge + 6, &columns, 1.5), Some(1));
    }

    #[test]
    fn showing_only_name_has_no_divider() {
        let mut columns = columns();
        columns.truncate(1);
        assert_eq!(divider_at(columns[0].rect.right(), &columns, 1.0), None);
    }

    #[test]
    fn a_column_widens_with_the_pointer_while_name_holds_still() {
        let view = resize(1, 1.0).view_at(540);
        assert_eq!(width_of(&view, Column::Modified), Some(170));
        assert_eq!(width_of(&view, Column::Name), Some(300), "Name is held");
        assert_eq!(
            width_of(&view, Column::Kind),
            None,
            "the others keep theirs"
        );
    }

    #[test]
    fn the_divider_beside_name_resizes_the_next_column_against_the_pointer() {
        let view = resize(0, 1.0).view_at(540);
        assert_eq!(width_of(&view, Column::Modified), Some(90));
        assert_eq!(
            width_of(&view, Column::Name),
            None,
            "Name takes the rest, so the divider follows the pointer"
        );
    }

    #[test]
    fn widths_clamp_at_both_ends() {
        let (low, high) = deskspace_core::COLUMN_WIDTH_RANGE;
        assert_eq!(
            width_of(&resize(1, 1.0).view_at(-5000), Column::Modified),
            Some(low)
        );
        assert_eq!(
            width_of(&resize(1, 1.0).view_at(50_000), Column::Modified),
            Some(high)
        );
    }

    #[test]
    fn a_stored_width_is_at_96_dpi() {
        // 130 px on a 150 % display is 87 at 96 DPI; 30 px more is 107.
        let view = resize(1, 1.5).view_at(530);
        assert_eq!(width_of(&view, Column::Modified), Some(107));
    }

    #[test]
    fn a_release_lets_name_take_the_rest_again() {
        let held = resize(1, 1.0).view_at(540);
        let released = held.with_name_held(None);
        assert_eq!(width_of(&released, Column::Name), None);
        assert_eq!(width_of(&released, Column::Modified), Some(170));
    }

    #[test]
    fn a_drag_back_to_the_start_keeps_every_other_view_setting() {
        let view = SpaceView {
            status_bar: true,
            columns: ColumnSpec::sanitize(vec![ColumnSpec {
                id: Column::Size,
                width: Some(90),
            }]),
            ..SpaceView::default()
        };
        let columns: Vec<DetailsColumnVisual> = columns().into_iter().take(2).collect();
        let resize = ColumnResize::new(SpaceId(1), &view, &columns, 1, 500, 1.0).unwrap();
        let moved = resize.view_at(510);
        assert!(moved.status_bar);
        assert_eq!(width_of(&moved, Column::Size), Some(140));
    }

    #[test]
    fn a_press_that_has_not_moved_changes_nothing() {
        assert_eq!(resize(1, 1.0).view_at(500), SpaceView::default());
        assert_eq!(resize(0, 1.5).view_at(500), SpaceView::default());
    }

    #[test]
    fn a_stale_divider_starts_no_drag() {
        assert!(
            ColumnResize::new(SpaceId(1), &SpaceView::default(), &columns(), 9, 500, 1.0).is_none()
        );
    }
}
