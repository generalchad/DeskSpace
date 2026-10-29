//! An icon-set glyph as an SVG document Direct2D can draw (ROADMAP D16).
//!
//! Iconify publishes a body meant to sit inside an `<svg>`, written with
//! `currentColor`, which Direct2D's SVG support does not have. The color goes
//! in its place, and a translucent color becomes the document's opacity.

use deskspace_core::Glyph;
use deskspace_core::theme::Color;

/// `glyph`'s body as a complete SVG document at the glyph's own size, drawn
/// in `color`.
pub fn svg_document(glyph: &Glyph, color: Color) -> String {
    let fill = format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b);
    // The root's fill reaches a body that names no paint, and the replacement
    // covers every `currentColor` a body does write.
    let body = glyph.body.replace("currentColor", &fill);
    let opacity = if color.a == u8::MAX {
        String::new()
    } else {
        format!(" opacity=\"{:.3}\"", f32::from(color.a) / 255.0)
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" fill=\"{fill}\"{opacity}>{body}</svg>",
        glyph.width.max(1),
        glyph.height.max(1)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(width: u32, height: u32, body: &str) -> Glyph {
        Glyph {
            key: "fluent:thing-20-regular".to_owned(),
            body: body.to_owned(),
            width,
            height,
        }
    }

    #[test]
    fn it_wraps_the_body_at_the_glyphs_own_size() {
        let document = svg_document(&glyph(24, 20, "M0 0h24v20H0z"), Color::rgb(1, 2, 3));
        assert_eq!(
            document,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 20\" fill=\"#010203\">M0 0h24v20H0z</svg>"
        );
    }

    #[test]
    fn it_replaces_every_current_color() {
        let body = "<path fill=\"currentColor\" d=\"M0 0\"/><path stroke=\"currentColor\"/>";
        let document = svg_document(&glyph(16, 16, body), Color::rgb(0x1a, 0x2b, 0x3c));
        assert!(!document.contains("currentColor"));
        assert_eq!(document.matches("#1a2b3c").count(), 3, "{document}");
    }

    #[test]
    fn only_a_translucent_color_adds_opacity() {
        let opaque = svg_document(&glyph(16, 16, ""), Color::rgb(255, 255, 255));
        assert!(!opaque.contains("opacity"), "{opaque}");
        let half = svg_document(&glyph(16, 16, ""), Color::rgba(255, 255, 255, 128));
        assert!(half.contains(" opacity=\"0.502\""), "{half}");
        let clear = svg_document(&glyph(16, 16, ""), Color::TRANSPARENT);
        assert!(clear.contains(" opacity=\"0.000\""), "{clear}");
    }

    #[test]
    fn a_glyph_that_gives_no_size_is_one_pixel_rather_than_broken() {
        let document = svg_document(&glyph(0, 0, ""), Color::rgb(0, 0, 0));
        assert!(document.contains("viewBox=\"0 0 1 1\""), "{document}");
    }
}
