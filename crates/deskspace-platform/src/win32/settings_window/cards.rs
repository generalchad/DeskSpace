//! A page's cards: the rounded box behind a group of fields, and the hairlines
//! between its rows (ROADMAP D26).

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CreatePen, CreateSolidBrush, DeleteObject, FillRect, HDC, HGDIOBJ, PS_SOLID, RoundRect,
    SelectObject,
};

use crate::settings_cards::CARD_RADIUS;
use crate::win32::dark_mode::UiColors;

use super::build::scaled;
use super::commands::colorref;
use super::paint::erase_body;
use super::with_state;

/// One page's cards and their row dividers, in page coordinates.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct PageCards {
    pub(super) cards: Vec<Rect>,
    pub(super) hairlines: Vec<Rect>,
}

/// Fills `page` with the body color and then draws its cards, so a card's
/// fill meets the erasing background rather than showing the body through a
/// seam.
///
/// # Safety
/// `settings` is the live settings window, `page` is one of its live page
/// containers, and `dc` is `page`'s device context for this erase.
pub(super) unsafe fn erase_page(settings: HWND, page: HWND, dc: HDC) -> bool {
    // SAFETY: the caller's contract; the borrow ends before the drawing.
    unsafe {
        if !erase_body(settings, page, dc) {
            return false;
        }
        let Some((colors, cards, dpi)) = with_state(settings, |state| {
            let index = state.pages.iter().position(|each| *each == page)?;
            Some((state.colors, state.cards.get(index)?.clone(), state.dpi))
        })
        .flatten() else {
            return true;
        };
        draw(&cards, dc, colors, dpi);
        true
    }
}

/// Draws each card as a rounded box in the card color, edged in its stroke,
/// and each hairline as a 1 px line in the stroke color.
fn draw(cards: &PageCards, dc: HDC, colors: UiColors, dpi: u32) {
    // SAFETY: `dc` is a live device context; every object made here is
    // selected out again and deleted, and the previous ones are restored.
    unsafe {
        let fill = CreateSolidBrush(colorref(colors.card));
        let stroke = CreateSolidBrush(colorref(colors.card_stroke));
        let pen = CreatePen(PS_SOLID, 1, colorref(colors.card_stroke));
        let previous_brush = SelectObject(dc, HGDIOBJ(fill.0));
        let previous_pen = SelectObject(dc, HGDIOBJ(pen.0));
        let radius = scaled(CARD_RADIUS, dpi);
        for card in &cards.cards {
            let _ = RoundRect(
                dc,
                card.x,
                card.y,
                card.x + card.width,
                card.y + card.height,
                radius,
                radius,
            );
        }
        for line in &cards.hairlines {
            let rect = RECT {
                left: line.x,
                top: line.y,
                right: line.x + line.width,
                bottom: line.y + line.height,
            };
            FillRect(dc, &rect, stroke);
        }
        SelectObject(dc, previous_pen);
        SelectObject(dc, previous_brush);
        let _ = DeleteObject(HGDIOBJ(fill.0));
        let _ = DeleteObject(HGDIOBJ(stroke.0));
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}
