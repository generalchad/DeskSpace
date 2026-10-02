use deskspace_core::geometry::Rect as CoreRect;
use deskspace_core::theme::Color;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap, CreateCompatibleDC,
    CreateFontW, CreatePen, CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_QUALITY, DT_CENTER,
    DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_TOP, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW,
    EndPaint, FillRect, GetStockObject, HBITMAP, HDC, HFONT, HGDIOBJ, HOLLOW_BRUSH,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, RoundRect, SRCCOPY, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DrawIconEx, HICON};
use windows::core::w;

use crate::error::PlatformError;
use crate::render::scene::{spaceVisual, Scene};

const ICON_SIZE: i32 = 32;
const CORNER: i32 = 12;

pub struct Painter {
    mem_dc: Option<HDC>,
    bitmap: Option<HBITMAP>,
    previous: HGDIOBJ,
    width: i32,
    height: i32,
    title_font: HFONT,
    item_font: HFONT,
    painted: bool,
}

impl Painter {
    pub fn new() -> Self {
        Self {
            mem_dc: None,
            bitmap: None,
            previous: HGDIOBJ::default(),
            width: 0,
            height: 0,
            title_font: make_font(15, 600),
            item_font: make_font(13, 400),
            painted: false,
        }
    }

    pub fn reset_paint(&mut self) {
        self.painted = false;
    }

    pub fn was_painted(&self) -> bool {
        self.painted
    }

    pub fn draw(&mut self, hwnd: HWND, scene: &Scene) -> Result<(), PlatformError> {
        let width = scene.width.max(1);
        let height = scene.height.max(1);
        self.painted = true;

        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let target = BeginPaint(hwnd, &mut ps);
            self.ensure_buffer(target, width, height)?;

            if let Some(mem) = self.mem_dc {
                self.paint_scene(mem, scene, width, height);
                let _ = BitBlt(target, 0, 0, width, height, Some(mem), 0, 0, SRCCOPY);
            }

            let _ = EndPaint(hwnd, &ps);
        }
        Ok(())
    }

    fn ensure_buffer(
        &mut self,
        reference: HDC,
        width: i32,
        height: i32,
    ) -> Result<(), PlatformError> {
        unsafe {
            if self.mem_dc.is_none() {
                self.mem_dc = Some(CreateCompatibleDC(Some(reference)));
            }
            if self.bitmap.is_some() && self.width == width && self.height == height {
                return Ok(());
            }
            let dc = self.mem_dc.expect("memory dc just created");
            if !self.previous.0.is_null() {
                let _ = SelectObject(dc, self.previous);
            }
            if let Some(bitmap) = self.bitmap.take() {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            let bitmap = CreateCompatibleBitmap(reference, width, height);
            self.previous = SelectObject(dc, HGDIOBJ(bitmap.0));
            self.bitmap = Some(bitmap);
            self.width = width;
            self.height = height;
        }
        Ok(())
    }

    fn paint_scene(&self, dc: HDC, scene: &Scene, width: i32, height: i32) {
        unsafe {
            let background = CreateSolidBrush(colorref(scene.color_key));
            let full = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            let _ = FillRect(dc, &full, background);
            let _ = DeleteObject(HGDIOBJ(background.0));
            let _ = SetBkMode(dc, TRANSPARENT);

            for space in &scene.spaces {
                self.paint_space(dc, space);
            }

            if let Some(status) = &scene.status {
                self.paint_status(dc, status, height);
            }
        }
    }

    fn paint_space(&self, dc: HDC, space: &spaceVisual) {
        unsafe {
            fill_round(
                dc,
                &rect(space.rect.offset(4, 4)),
                CORNER,
                space.shadow,
                space.shadow,
            );
            fill_round(dc, &rect(space.rect), CORNER, space.border, space.fill);
            fill_round(
                dc,
                &rect(space.title_rect.inset(1)),
                CORNER - 2,
                space.title_fill,
                space.title_fill,
            );

            if space.selected {
                stroke_round(dc, &rect(space.rect.inset(-1)), CORNER + 1, space.border);
            }

            let previous = SelectObject(dc, HGDIOBJ(self.title_font.0));
            let _ = SetTextColor(dc, colorref(space.title_text));
            let mut title_rect = RECT {
                left: space.title_rect.x + 12,
                top: space.title_rect.y,
                right: space.title_rect.right() - 12,
                bottom: space.title_rect.bottom(),
            };
            let mut title: Vec<u16> = space.title.encode_utf16().collect();
            let _ = DrawTextW(
                dc,
                &mut title,
                &mut title_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
            let _ = SelectObject(dc, previous);

            if space.rolled_up {
                return;
            }

            let item_font = SelectObject(dc, HGDIOBJ(self.item_font.0));
            for item in &space.items {
                self.paint_item(dc, item, space.item_text);
            }
            let _ = SelectObject(dc, item_font);
        }
    }

    fn paint_item(&self, dc: HDC, item: &crate::render::scene::ItemVisual, text: Color) {
        unsafe {
            fill_round(dc, &rect(item.rect), 8, item.border, item.fill);

            let mut label_top = item.rect.y + 6;
            if let Some(handle) = item.icon {
                let x = item.rect.x + (item.rect.width - ICON_SIZE) / 2;
                let y = item.rect.y + 6;
                let _ = DrawIconEx(
                    dc,
                    x,
                    y,
                    HICON(handle as *mut core::ffi::c_void),
                    ICON_SIZE,
                    ICON_SIZE,
                    0,
                    None,
                    DI_NORMAL,
                );
                label_top = y + ICON_SIZE + 4;
            }

            let mut text_rect = RECT {
                left: item.rect.x + 4,
                top: label_top,
                right: item.rect.right() - 4,
                bottom: item.rect.bottom() - 4,
            };
            if text_rect.top >= text_rect.bottom {
                return;
            }
            let _ = SetTextColor(dc, colorref(text));
            let mut label: Vec<u16> = item.label.encode_utf16().collect();
            let _ = DrawTextW(
                dc,
                &mut label,
                &mut text_rect,
                DT_CENTER | DT_TOP | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
        }
    }

    fn paint_status(&self, dc: HDC, status: &str, height: i32) {
        unsafe {
            let previous = SelectObject(dc, HGDIOBJ(self.item_font.0));
            let _ = SetTextColor(dc, colorref(Color::rgb(120, 128, 144)));
            let mut text_rect = RECT {
                left: 12,
                top: height - 22,
                right: 900,
                bottom: height - 4,
            };
            let mut text: Vec<u16> = status.encode_utf16().collect();
            let _ = DrawTextW(dc, &mut text, &mut text_rect, DT_LEFT | DT_SINGLELINE);
            let _ = SelectObject(dc, previous);
        }
    }
}

impl Default for Painter {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Painter {
    fn drop(&mut self) {
        unsafe {
            if let Some(dc) = self.mem_dc {
                if !self.previous.0.is_null() {
                    let _ = SelectObject(dc, self.previous);
                }
                let _ = DeleteDC(dc);
            }
            if let Some(bitmap) = self.bitmap {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            let _ = DeleteObject(HGDIOBJ(self.title_font.0));
            let _ = DeleteObject(HGDIOBJ(self.item_font.0));
        }
    }
}

fn make_font(height: i32, weight: i32) -> HFONT {
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            w!("Segoe UI"),
        )
    }
}

fn fill_round(dc: HDC, bounds: &RECT, radius: i32, border: Color, fill: Color) {
    unsafe {
        let brush = CreateSolidBrush(colorref(fill));
        let pen = CreatePen(PS_SOLID, 1, colorref(border));
        let old_brush = SelectObject(dc, HGDIOBJ(brush.0));
        let old_pen = SelectObject(dc, HGDIOBJ(pen.0));
        let _ = RoundRect(
            dc,
            bounds.left,
            bounds.top,
            bounds.right,
            bounds.bottom,
            radius,
            radius,
        );
        let _ = SelectObject(dc, old_brush);
        let _ = SelectObject(dc, old_pen);
        let _ = DeleteObject(HGDIOBJ(brush.0));
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

fn stroke_round(dc: HDC, bounds: &RECT, radius: i32, color: Color) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, colorref(color));
        let old_pen = SelectObject(dc, HGDIOBJ(pen.0));
        let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
        let _ = RoundRect(
            dc,
            bounds.left,
            bounds.top,
            bounds.right,
            bounds.bottom,
            radius,
            radius,
        );
        let _ = SelectObject(dc, old_brush);
        let _ = SelectObject(dc, old_pen);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

fn colorref(color: Color) -> COLORREF {
    COLORREF(color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16))
}

fn rect(value: CoreRect) -> RECT {
    RECT {
        left: value.x,
        top: value.y,
        right: value.right(),
        bottom: value.bottom(),
    }
}
