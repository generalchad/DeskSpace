//! Space chrome: title icons, the Back button, title tips, and scroll thumbs.

use std::path::{Path, PathBuf};

use deskspace_core::space::DEFAULT_TITLE_HEIGHT;
use deskspace_core::view_layout::{ScrollAxis, ViewLayout};
use deskspace_core::{Point, Rect, Space};
use deskspace_platform::{Icon, TitleIcon};

use super::subfolder_path;

pub(super) const SCROLL_THUMB_WIDTH: i32 = 4;
pub(super) const SCROLL_THUMB_MARGIN: i32 = 3;
const SCROLL_THUMB_MIN_HEIGHT: i32 = 24;
/// A title icon is a 16 px square 10 px from the left edge at 96 DPI, scaled
/// with the title bar like the title text.
const TITLE_ICON_SIZE: i32 = 16;
const TITLE_ICON_INSET: i32 = 10;
/// The Back button of a browsed lens (D18), at 96 DPI: a 24 px square, 4 px
/// from the left edge, and 2 px before the title icon. Provisional until
/// the Back button's on-screen check compares it with Explorer's.
const BACK_BUTTON_SIZE: i32 = 24;
const BACK_BUTTON_INSET: i32 = 4;
pub(super) const BACK_BUTTON_GAP: i32 = 2;

/// Where a space's title icon goes in a space `title_height` pixels tall: a
/// square `TITLE_ICON_SIZE` at 96 DPI, `TITLE_ICON_INSET` from the left edge,
/// centered vertically, both scaled by `title_height / DEFAULT_TITLE_HEIGHT`.
pub fn title_icon_rect(title_height: i32) -> Rect {
    title_icon_rect_after(title_height, None)
}

/// Where `space`'s title icon goes, moved past its Back button while the lens
/// is `browsed`.
pub fn title_icon_rect_of(space: &Space, browsed: bool) -> Rect {
    title_icon_rect_after(
        space.title_height,
        browsed.then(|| back_button_rect(space.title_height)),
    )
}

/// The title icon as a button while its space's title is renamed: the picture
/// it has, or the placeholder when `image` is `None` (D16, D26).
pub fn title_icon_button(rect: Rect, image: Option<Icon>, hovered: bool) -> TitleIcon {
    TitleIcon {
        rect,
        image,
        button: true,
        hovered,
    }
}

/// Whether `point`, in desktop coordinates, is on `space`'s title icon, which
/// is a button only while its title is renamed.
pub fn title_icon_at(space: &Space, browsed: bool, point: Point) -> bool {
    let origin = space.rect().origin();
    title_icon_rect_of(space, browsed)
        .offset(origin.x, origin.y)
        .contains(point)
}

/// `title_icon_rect`, moved past a Back button at `back` when there is one,
/// by the same inset it keeps from the edge otherwise.
pub(super) fn title_icon_rect_after(title_height: i32, back: Option<Rect>) -> Rect {
    let scale = title_height as f32 / DEFAULT_TITLE_HEIGHT as f32;
    let size = ((TITLE_ICON_SIZE as f32) * scale).round().max(1.0) as i32;
    let gap = match back {
        Some(_) => BACK_BUTTON_GAP,
        None => TITLE_ICON_INSET,
    };
    let left = back.map_or(0, Rect::right) + ((gap as f32) * scale).round() as i32;
    Rect::new(left, ((title_height - size) / 2).max(0), size, size)
}

/// Where the Back button goes in a title bar `title_height` pixels tall while
/// a lens shows a subfolder: a square `BACK_BUTTON_SIZE` at 96 DPI,
/// `BACK_BUTTON_INSET` from the left edge, centered vertically, scaled with
/// the bar like the title icon.
pub fn back_button_rect(title_height: i32) -> Rect {
    let scale = title_height as f32 / DEFAULT_TITLE_HEIGHT as f32;
    let size = ((BACK_BUTTON_SIZE as f32) * scale).round().max(1.0) as i32;
    let inset = ((BACK_BUTTON_INSET as f32) * scale).round() as i32;
    Rect::new(inset, ((title_height - size) / 2).max(0), size, size)
}

/// Whether `point`, in desktop coordinates, is on the Back button of
/// `space`, which shows one only while `browsed`.
pub fn back_button_at(space: &Space, browsed: bool, point: Point) -> bool {
    let origin = space.rect().origin();
    browsed
        && back_button_rect(space.title_height)
            .contains(Point::new(point.x - origin.x, point.y - origin.y))
}

/// What a lens's title bar asks the icon cache for: the folder it shows
/// (`sub` below its own) and whether that folder is available, so the icon
/// and the flag describe the same folder, as Explorer's does. Pure, so the
/// scene test needs no shell.
pub(super) fn title_icon_request(
    space: &Space,
    sub: &Path,
    unavailable: bool,
) -> Option<(PathBuf, bool)> {
    space
        .folder_view()
        .map(|folder| (subfolder_path(&folder.path, sub), !unavailable))
}

/// The tip a lens's title bar shows on hover: the folder it displays,
/// with `Folder not available` first when its folder is gone. `None` for a
/// normal space, whose title shows nothing, as before.
pub(crate) fn title_tip(space: &Space, shown: &Path, unavailable: bool) -> Option<String> {
    space.is_lens().then(|| {
        if unavailable {
            format!("Folder not available\n{}", shown.display())
        } else {
            shown.display().to_string()
        }
    })
}

/// Keeps the scroll indicator's far end clear of the rounded corner.
pub(super) fn thumb_end_margin(corner_radius: i32) -> i32 {
    SCROLL_THUMB_MARGIN + corner_radius / 2
}

/// A thin bar along the content's far edge, on the scroll axis: the right edge
/// for a vertical view, the bottom for a horizontal one. Its length and
/// position show which part of the content is visible. `None` when everything
/// fits.
pub(super) fn scroll_thumb(
    space: &Space,
    layout: &ViewLayout,
    scroll: i32,
    corner_radius: i32,
) -> Option<Rect> {
    if space.folded() {
        return None;
    }
    let content = space.content_rect();
    let viewport = layout.viewport(content.size());
    let max_scroll = layout.max_scroll(content.size());
    if max_scroll == 0 {
        return None;
    }
    let extent = layout.extent();
    let end_margin = thumb_end_margin(corner_radius);
    // The bar stops at the content's bottom, which the status bar may raise.
    let content_bottom = content.bottom() - space.rect().y;
    match layout.axis() {
        ScrollAxis::Vertical => {
            let track_top = space.title_height + SCROLL_THUMB_MARGIN;
            let track_height = content_bottom - end_margin - track_top;
            if track_height <= 0 {
                return None;
            }
            let (offset, thumb_height) =
                thumb_span(track_height, viewport, extent, scroll, max_scroll);
            Some(Rect::new(
                space.rect().width - SCROLL_THUMB_MARGIN - SCROLL_THUMB_WIDTH,
                track_top + offset,
                SCROLL_THUMB_WIDTH,
                thumb_height,
            ))
        }
        ScrollAxis::Horizontal => {
            let track_width = viewport - SCROLL_THUMB_MARGIN - end_margin;
            if track_width <= 0 {
                return None;
            }
            let (offset, thumb_width) =
                thumb_span(track_width, viewport, extent, scroll, max_scroll);
            Some(Rect::new(
                SCROLL_THUMB_MARGIN + offset,
                content_bottom - SCROLL_THUMB_MARGIN - SCROLL_THUMB_WIDTH,
                thumb_width,
                SCROLL_THUMB_WIDTH,
            ))
        }
    }
}

/// A thumb's offset along a `track` and its length, for a `viewport` onto
/// `extent` of content scrolled by `scroll`, at most `max_scroll`.
pub(super) fn thumb_span(
    track: i32,
    viewport: i32,
    extent: i32,
    scroll: i32,
    max_scroll: i32,
) -> (i32, i32) {
    let length = (track * viewport / extent)
        .max(SCROLL_THUMB_MIN_HEIGHT)
        .min(track);
    let offset = (track - length) * scroll.clamp(0, max_scroll) / max_scroll;
    (offset, length)
}

#[cfg(test)]
mod tests {
    use deskspace_core::Desktop;

    use super::super::tests::space;
    use super::*;

    #[test]
    fn a_lens_title_tip_names_its_folder_and_a_normal_space_shows_none() {
        let mut desktop = Desktop::new();
        let lens = crate::fixtures::create_lens(
            &mut desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            r"C:\Users\me\Downloads",
        );
        let lens = desktop.space(lens).unwrap();
        let shown = Path::new(r"C:\Users\me\Downloads\Photos");
        assert_eq!(
            title_tip(lens, shown, false).as_deref(),
            Some(r"C:\Users\me\Downloads\Photos"),
            "an available lens names the folder it displays"
        );
        assert_eq!(
            title_tip(lens, shown, true).as_deref(),
            Some(concat!(
                "Folder not available\n",
                r"C:\Users\me\Downloads\Photos"
            )),
            "an unavailable lens says so first"
        );
        assert_eq!(title_tip(&space(), shown, false), None, "a normal space");
    }

    #[test]
    fn a_title_icon_is_a_scaled_square_inside_the_bar() {
        assert_eq!(title_icon_rect(28), Rect::new(10, 6, 16, 16));
        assert_eq!(title_icon_rect(42), Rect::new(15, 9, 24, 24), "scaled");
    }

    #[test]
    fn the_title_icon_button_covers_only_the_icon() {
        let space = space();
        let rect = title_icon_rect_of(&space, false);
        let origin = space.rect().origin();
        assert!(title_icon_at(
            &space,
            false,
            Point::new(origin.x + rect.x + 1, origin.y + rect.y + 1)
        ));
        assert!(!title_icon_at(
            &space,
            false,
            Point::new(origin.x + rect.right() + 1, origin.y + rect.y)
        ));
    }
}
