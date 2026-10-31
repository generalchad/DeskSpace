//! The view and sort controls the View page, the Lenses defaults page, and
//! the New spaces page share (W20, W23, W40).

use deskspace_core::SpaceView;
use deskspace_platform::Field;

use super::ids::field;
use super::tables::{SORT_KEYS, VIEW_MODES, VIEW_ORDERS, group_choice, group_labels};
use super::{index_of, labels};

/// The field ids one page uses for the shared view and sort controls. The
/// pages differ in which fields they list: `keep_sorted` is `None` on the
/// Lenses page, where a lens always keeps its order.
pub(super) struct ViewIds {
    pub(super) heading: u32,
    pub(super) mode: u32,
    pub(super) status_bar: u32,
    pub(super) sort_heading: u32,
    pub(super) sort: u32,
    pub(super) order: u32,
    pub(super) folders_first: u32,
    pub(super) keep_sorted: Option<u32>,
    /// The Group by choice and its direction. `None` on the New spaces page,
    /// which W38 leaves out.
    pub(super) group: Option<(u32, u32)>,
}

pub(super) const VIEW_PAGE_IDS: ViewIds = ViewIds {
    heading: field::VIEW_HEADING,
    mode: field::VIEW_MODE,
    status_bar: field::VIEW_STATUS_BAR,
    sort_heading: field::VIEW_SORT_HEADING,
    sort: field::VIEW_SORT_KEY,
    order: field::VIEW_ORDER,
    folders_first: field::VIEW_FOLDERS_FIRST,
    keep_sorted: Some(field::VIEW_KEEP_SORTED),
    group: Some((field::VIEW_GROUP, field::VIEW_GROUP_ORDER)),
};

pub(super) const LENSES_PAGE_IDS: ViewIds = ViewIds {
    heading: field::LENSES_VIEW_HEADING,
    mode: field::LENSES_VIEW_MODE,
    status_bar: field::LENSES_STATUS_BAR,
    sort_heading: field::LENSES_SORT_HEADING,
    sort: field::LENSES_SORT_KEY,
    order: field::LENSES_ORDER,
    folders_first: field::LENSES_FOLDERS_FIRST,
    keep_sorted: None,
    group: Some((field::LENSES_GROUP, field::LENSES_GROUP_ORDER)),
};

pub(super) const NEW_SPACES_PAGE_IDS: ViewIds = ViewIds {
    heading: field::NEW_SPACES_VIEW_HEADING,
    mode: field::NEW_SPACE_VIEW_MODE,
    status_bar: field::NEW_SPACE_STATUS_BAR,
    sort_heading: field::NEW_SPACES_SORT_HEADING,
    sort: field::NEW_SPACE_SORT_KEY,
    order: field::NEW_SPACE_ORDER,
    folders_first: field::NEW_SPACE_FOLDERS_FIRST,
    keep_sorted: Some(field::NEW_SPACE_KEEP_SORTED),
    group: None,
};

/// The view and sort controls shared by the View page, the Lenses defaults
/// page, and the New spaces page. `lens` greys and ticks Keep sorted, as a
/// lens always holds its order.
pub(super) fn view_fields(view: &SpaceView, ids: &ViewIds, lens: bool) -> Vec<Field> {
    let modes: Vec<&str> = VIEW_MODES.iter().map(|(_, label, _)| *label).collect();
    let mode = VIEW_MODES
        .iter()
        .position(|(candidate, ..)| *candidate == view.mode)
        .unwrap_or(0);
    let mut fields = vec![
        Field::heading(ids.heading, "View"),
        Field::choice(ids.mode, "Show as", &modes, mode),
        Field::toggle(ids.status_bar, "Status bar", view.status_bar),
        Field::heading(ids.sort_heading, "Sort"),
        Field::choice(
            ids.sort,
            "Sort by",
            &labels(&SORT_KEYS),
            index_of(&SORT_KEYS, view.sort.key),
        ),
        Field::choice(
            ids.order,
            "Order",
            &VIEW_ORDERS,
            usize::from(view.sort.descending),
        ),
        Field::toggle(ids.folders_first, "Folders first", view.sort.folders_first),
    ];
    if let Some(id) = ids.keep_sorted {
        fields.push(Field::toggle(id, "Keep sorted", view.keep_sorted || lens).enabled(!lens));
    }
    if let Some((group, order)) = ids.group {
        fields.push(Field::choice(
            group,
            "Group by",
            &group_labels(),
            group_choice(view.group),
        ));
        fields.push(
            Field::choice(
                order,
                "Group order",
                &VIEW_ORDERS,
                usize::from(view.group.is_some_and(|group| group.descending)),
            )
            .enabled(view.group.is_some()),
        );
    }
    fields
}
