//! Details header layout, sorting clicks, row cell formatting, and sideways scroll thumb.

use deskspace_core::item::ItemKind;
use deskspace_core::view_layout::ViewLayout;
use deskspace_core::{Item, Point, Rect, SortKey, SortRule, Space, SpaceView, ViewMode};
use deskspace_platform::{DetailsColumnVisual, DetailsView, format_date, format_kb, type_name};

use crate::settings::{NAME_MIN_WIDTH, shown_details_columns};

use super::chrome::{SCROLL_THUMB_MARGIN, SCROLL_THUMB_WIDTH, thumb_end_margin, thumb_span};

/// The gap between a Details column's text and its divider, at 96 DPI.
const DETAILS_CELL_PAD: i32 = 6;

/// The app's Details row text: one item in, its column texts out.
pub type DetailsCellText<'a> = &'a mut dyn FnMut(&Item) -> Vec<String>;

/// The Details header for `space`: its columns and which one is sorted, or
/// `None` in any other view. Column rects are relative to the content area.
pub(crate) fn details_view(
    space: &Space,
    layout: &ViewLayout,
    content_width: i32,
    padding: i32,
    scale: f32,
    scroll_x: i32,
) -> Option<DetailsView> {
    if space.view().mode != ViewMode::Details {
        return None;
    }
    let (columns, scroll_x) = scrolled_details_columns(
        space.view(),
        content_width,
        padding,
        scroll_x,
        layout.header_height(),
        scale,
    );
    Some(DetailsView {
        header_height: layout.header_height(),
        cell_pad: (DETAILS_CELL_PAD as f32 * scale).round().max(1.0) as i32,
        columns,
        sorted: details_sorted(space.view()),
        drop_mark: None,
        scroll_x,
        visible: (padding, (content_width - padding).max(padding)),
    })
}

/// The x of the insertion mark before column `insert_at`, or after the last
/// column past it.
pub(crate) fn drop_mark_x(columns: &[DetailsColumnVisual], insert_at: usize) -> Option<i32> {
    columns
        .get(insert_at)
        .map(|column| column.rect.x)
        .or_else(|| columns.last().map(|column| column.rect.right()))
}

/// The Details columns `view` shows, laid out across `content_width`: Name
/// takes the width left over, down to `NAME_MIN_WIDTH`, unless a divider drag
/// holds it, and the rest keep their own widths. Rects are relative to the
/// content area, shifted left by `offset` for a sideways scroll, which is
/// clamped to what the columns need.
pub(crate) fn details_columns(
    view: &SpaceView,
    content_width: i32,
    padding: i32,
    offset: i32,
    header_height: i32,
    scale: f32,
) -> Vec<DetailsColumnVisual> {
    scrolled_details_columns(view, content_width, padding, offset, header_height, scale).0
}

/// `details_columns`, and the offset they were shifted by once clamped.
fn scrolled_details_columns(
    view: &SpaceView,
    content_width: i32,
    padding: i32,
    offset: i32,
    header_height: i32,
    scale: f32,
) -> (Vec<DetailsColumnVisual>, i32) {
    let scale = scale.max(0.1);
    let scaled = |value: i32| ((value as f32 * scale).round() as i32).max(1);
    let fixed: i32 = shown_details_columns(view)
        .skip(1)
        .map(|(_, width)| scaled(width))
        .sum();
    let rest = ((content_width - padding * 2 - fixed).max(scaled(NAME_MIN_WIDTH))).max(1);
    let name_width = view
        .columns
        .first()
        .and_then(|name| name.width)
        .map_or(rest, |held| scaled(i32::from(held)));
    let mut x = padding;
    let mut columns: Vec<DetailsColumnVisual> = shown_details_columns(view)
        .enumerate()
        .map(|(index, (column, width))| {
            let width = if index == 0 {
                name_width
            } else {
                scaled(width)
            };
            let visual = DetailsColumnVisual {
                rect: Rect::new(x, 0, width, header_height),
                label: column.label.to_owned(),
                align: column.align,
            };
            x += width;
            visual
        })
        .collect();
    let offset = offset.clamp(0, details_max_scroll(&columns, content_width, padding));
    for column in &mut columns {
        column.rect.x -= offset;
    }
    (columns, offset)
}

/// How far the Details columns can scroll sideways in `content_width`, leaving
/// `padding` at the right edge. Zero when the columns already fit.
pub(crate) fn details_max_scroll(
    columns: &[DetailsColumnVisual],
    content_width: i32,
    padding: i32,
) -> i32 {
    columns.last().map_or(0, |column| {
        (column.rect.right() - content_width + padding).max(0)
    })
}

/// The column whose header shows Explorer's arrow, and whether it points up.
fn details_sorted(view: &SpaceView) -> Option<(usize, bool)> {
    shown_details_columns(view)
        .position(|(column, _)| column.key == view.sort.key)
        .map(|index| (index, !view.sort.descending))
}

/// The Details column whose header contains `point`, relative to the content
/// area. `None` below the header row.
pub(crate) fn details_header_column_at(view: &DetailsView, point: Point) -> Option<usize> {
    view.columns
        .iter()
        .position(|column| column.rect.contains(point))
}

/// The sort rule after a click on `view`'s shown Details column `index`:
/// clicking the sorted column reverses it, and any other column becomes the
/// key in ascending order, as Explorer does. An index past the shown columns
/// keeps the rule.
pub(crate) fn details_rule_after(view: &SpaceView, index: usize) -> SortRule {
    let rule = view.sort;
    let Some(key) = shown_details_columns(view)
        .nth(index)
        .map(|(column, _)| column.key)
    else {
        return rule;
    };
    if rule.key == key {
        SortRule {
            descending: !rule.descending,
            ..rule
        }
    } else {
        SortRule {
            key,
            descending: false,
            ..rule
        }
    }
}

/// The text of one Details row for the columns `view` shows after Name, in
/// display order. It reads the item's facts only, never the disk.
pub(crate) fn details_cells(view: &SpaceView, item: &Item) -> Vec<String> {
    shown_details_columns(view)
        .skip(1)
        .map(|(column, _)| match column.key {
            SortKey::Modified => item.modified.map(format_date).unwrap_or_default(),
            SortKey::Created => item.created.map(format_date).unwrap_or_default(),
            SortKey::Kind => {
                type_name(&item.path, item.kind == ItemKind::Folder).unwrap_or_default()
            }
            SortKey::Size => item
                .size
                .filter(|_| item.kind != ItemKind::Folder)
                .map(format_kb)
                .unwrap_or_default(),
            SortKey::Name => String::new(),
        })
        .collect()
}

/// The bar along the bottom of a Details space whose columns pass its right
/// edge, showing how far they are scrolled sideways. `None` when the columns
/// fit, in any other view, and while folded. The rows' own bar stops short of
/// the bottom (`thumb_end_margin`), so the two never touch.
pub(super) fn details_scroll_thumb(
    space: &Space,
    content_width: i32,
    padding: i32,
    scale: f32,
    scroll_x: i32,
    corner_radius: i32,
) -> Option<Rect> {
    if space.folded() || space.view().mode != ViewMode::Details {
        return None;
    }
    let columns = details_columns(space.view(), content_width, padding, 0, 0, scale);
    let max_scroll = details_max_scroll(&columns, content_width, padding);
    let track_width = content_width - SCROLL_THUMB_MARGIN - thumb_end_margin(corner_radius);
    if max_scroll == 0 || track_width <= 0 {
        return None;
    }
    let extent = content_width + max_scroll;
    let (offset, thumb_width) =
        thumb_span(track_width, content_width, extent, scroll_x, max_scroll);
    let content_bottom = space.content_rect().bottom() - space.rect().y;
    Some(Rect::new(
        SCROLL_THUMB_MARGIN + offset,
        content_bottom - SCROLL_THUMB_MARGIN - SCROLL_THUMB_WIDTH,
        thumb_width,
        SCROLL_THUMB_WIDTH,
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use deskspace_core::view_layout::ViewMetrics;
    use deskspace_core::{Command, Desktop, GridSpec, ItemFacts, ItemId};

    use super::super::tests::details_space;
    use super::*;

    #[test]
    fn a_details_header_click_maps_to_its_column_and_reverses() {
        let space = details_space();
        let layout = ViewLayout::for_space(
            &space,
            ViewMetrics::for_view(
                &SpaceView::default().with_mode(ViewMode::Details),
                GridSpec::default(),
                1.0,
                100,
            ),
        );
        let view = details_view(
            &space,
            &layout,
            space.content_rect().width,
            GridSpec::default().padding,
            1.0,
            0,
        )
        .unwrap();
        assert_eq!(
            details_header_column_at(&view, view.columns[0].rect.center()),
            Some(0)
        );
        assert_eq!(
            details_header_column_at(&view, view.columns[1].rect.center()),
            Some(1)
        );
        assert_eq!(
            details_header_column_at(&view, Point::new(10, view.header_height + 5)),
            None,
            "below the header no column is hit"
        );

        let viewed = |sort| SpaceView {
            sort,
            ..SpaceView::default()
        };
        let rule = SortRule::default();
        let reversed = details_rule_after(&viewed(rule), 0);
        assert_eq!(
            reversed,
            SortRule {
                key: SortKey::Name,
                descending: true,
                folders_first: false,
            }
        );
        assert_eq!(details_rule_after(&viewed(reversed), 0), rule);
        let by_size = details_rule_after(&viewed(reversed), 3);
        assert_eq!(by_size.key, SortKey::Size);
        assert!(!by_size.descending);
    }

    #[test]
    fn the_date_created_column_starts_hidden() {
        let view = SpaceView {
            sort: SortRule {
                key: SortKey::Created,
                ..SortRule::default()
            },
            ..SpaceView::default()
        };
        assert_eq!(
            details_sorted(&view),
            None,
            "a hidden column shows no arrow"
        );
        assert_eq!(
            details_rule_after(&view, 4),
            view.sort,
            "past the shown columns"
        );
        let item = Item::new(
            ItemId(1),
            "C:/a.txt",
            ItemFacts {
                created: Some(1_700_000_000),
                ..ItemFacts::default()
            },
        );
        assert_eq!(details_cells(&view, &item).len(), 3, "Date, Type, and Size");
    }

    #[test]
    fn the_header_and_rows_show_the_spaces_own_columns_in_its_order() {
        use deskspace_core::{Column, ColumnSpec};

        let view = SpaceView {
            columns: ColumnSpec::sanitize(vec![
                ColumnSpec {
                    id: Column::Size,
                    width: Some(200),
                },
                ColumnSpec {
                    id: Column::Created,
                    width: None,
                },
            ]),
            sort: SortRule {
                key: SortKey::Size,
                ..SortRule::default()
            },
            ..SpaceView::default()
        };
        let columns = details_columns(&view, 1000, 0, 0, 24, 1.0);
        assert_eq!(
            columns
                .iter()
                .map(|column| column.label.as_str())
                .collect::<Vec<_>>(),
            ["Name", "Size", "Date created"]
        );
        assert_eq!(columns[1].rect.width, 200, "its own width");
        assert_eq!(columns[2].rect.width, 130, "the table's default");
        assert_eq!(columns[0].rect.width, 670, "Name takes the rest");
        assert_eq!(
            details_sorted(&view),
            Some((1, true)),
            "the arrow follows its column"
        );
        assert_eq!(
            details_rule_after(&view, 2).key,
            SortKey::Created,
            "a click sorts by the column at that place"
        );
        let item = Item::new(
            ItemId(1),
            "C:/a.txt",
            ItemFacts {
                size: Some(2048),
                ..ItemFacts::default()
            },
        );
        // The unit follows the locale and Windows build ("KB" or "KiB"), so
        // compare with the shell's own formatter rather than a literal.
        assert_eq!(
            details_cells(&view, &item),
            [format_kb(2048), String::new()]
        );
    }

    #[test]
    fn a_held_name_keeps_its_width_so_the_columns_may_pass_the_edge() {
        let view = SpaceView::default()
            .with_column_width(deskspace_core::Column::Modified, 400)
            .with_name_held(Some(300));
        let columns = details_columns(&view, 500, 0, 0, 24, 1.0);
        assert_eq!(columns[0].rect.width, 300);
        assert_eq!(columns[1].rect.x, 300, "its left edge stays put");
        assert_eq!(columns[1].rect.right(), 700, "and its divider follows");
        let released = details_columns(&view.with_name_held(None), 500, 0, 0, 24, 1.0);
        assert_eq!(released[0].rect.width, 120, "Name takes the rest again");
    }

    #[test]
    fn details_columns_shift_sideways_up_to_the_scroll_limit() {
        let view = two_column_details().with_name_held(Some(300));
        let columns = details_columns(&view, 500, 0, 0, 24, 1.0);
        assert_eq!(
            details_max_scroll(&columns, 500, 0),
            200,
            "the two columns span 700 in 500"
        );

        let shifted = details_columns(&view, 500, 0, 150, 24, 1.0);
        assert_eq!(shifted[0].rect.x, -150);
        assert_eq!(shifted[1].rect.x, 150);
        assert_eq!(shifted[1].rect.right(), 550);

        let clamped = details_columns(&view, 500, 0, 999, 24, 1.0);
        assert_eq!(
            clamped[1].rect.right(),
            500,
            "the last column reaches the edge"
        );
    }

    #[test]
    fn the_scroll_limit_leaves_the_padding_at_the_right_edge() {
        let view = two_column_details().with_name_held(Some(300));
        let columns = details_columns(&view, 500, 10, 0, 24, 1.0);
        assert_eq!(details_max_scroll(&columns, 500, 10), 220);

        let shifted = details_columns(&view, 500, 10, 220, 24, 1.0);
        assert_eq!(shifted[1].rect.right(), 490, "right padding shows");
    }

    #[test]
    fn details_scrolls_by_nothing_when_the_columns_fit() {
        let columns = details_columns(&two_column_details(), 1000, 0, 0, 24, 1.0);
        assert_eq!(details_max_scroll(&columns, 1000, 0), 0);
    }

    pub(in crate::scene) fn two_column_details() -> SpaceView {
        use deskspace_core::{Column, ColumnSpec};
        SpaceView {
            columns: ColumnSpec::sanitize(vec![
                ColumnSpec {
                    id: Column::Name,
                    width: None,
                },
                ColumnSpec {
                    id: Column::Modified,
                    width: Some(400),
                },
            ]),
            ..SpaceView::default()
        }
    }

    #[test]
    fn every_column_has_one_row_in_the_table() {
        use deskspace_core::Column;

        for id in Column::ALL {
            assert_eq!(
                crate::settings::DETAILS_COLUMNS
                    .iter()
                    .filter(|row| row.id == id)
                    .count(),
                1,
                "{id:?}"
            );
        }
    }

    #[test]
    fn the_sorted_details_column_shows_its_arrow() {
        let mut desktop = Desktop::new();
        let id = crate::fixtures::create_space(&mut desktop, "Details", Rect::new(0, 0, 360, 200));
        crate::fixtures::apply(
            &mut desktop,
            Command::SetSpaceView {
                id,
                view: deskspace_core::SpaceView {
                    mode: ViewMode::Details,
                    sort: SortRule {
                        key: SortKey::Size,
                        descending: true,
                        folders_first: false,
                    },
                    ..deskspace_core::SpaceView::default()
                },
            },
        );
        let space = desktop.space(id).unwrap();
        let layout = ViewLayout::for_space(
            space,
            ViewMetrics::for_view(
                &SpaceView::default().with_mode(ViewMode::Details),
                GridSpec::default(),
                1.0,
                100,
            ),
        );
        let view = details_view(
            space,
            &layout,
            space.content_rect().width,
            GridSpec::default().padding,
            1.0,
            0,
        )
        .unwrap();
        assert_eq!(view.sorted, Some((3, false)), "Size, pointing down");
    }
}
