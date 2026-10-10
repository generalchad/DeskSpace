use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Scales the alpha channel by `factor / 255`.
    pub const fn fade(self, factor: u8) -> Self {
        self.with_alpha(((self.a as u16 * factor as u16 + 127) / 255) as u8)
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }

    pub fn to_rgb_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        let parse = |s: &str| u8::from_str_radix(s, 16).ok();
        match hex.len() {
            6 => Some(Self::rgb(
                parse(&hex[0..2])?,
                parse(&hex[2..4])?,
                parse(&hex[4..6])?,
            )),
            8 => Some(Self::rgba(
                parse(&hex[0..2])?,
                parse(&hex[2..4])?,
                parse(&hex[4..6])?,
                parse(&hex[6..8])?,
            )),
            _ => None,
        }
    }

    /// Relative luminance per WCAG 2.x, in `0.0..=1.0`.
    /// https://www.w3.org/TR/WCAG21/#dfn-relative-luminance
    pub fn luminance(self) -> f32 {
        fn channel(value: u8) -> f32 {
            let c = value as f32 / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

/// Converts a percentage (clamped to 100) into an alpha byte.
pub const fn percent_to_alpha(percent: u8) -> u8 {
    let percent = if percent > 100 { 100 } else { percent };
    ((percent as u16 * 255 + 50) / 100) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

/// The material behind a space body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backdrop {
    /// Frosted (blur plus noise) view of whatever is behind the space.
    #[default]
    Acrylic,
    /// Plain blur of whatever is behind the space. Accepts "mica" from early
    /// v2 configs; Mica only renders on active windows, and spaces never are.
    #[serde(alias = "mica")]
    Blur,
    /// No backdrop material; the tint is drawn fully opaque.
    Solid,
    /// Plain glass: no blur, only the tint at its opacity. At 0 % tint, no
    /// title shading, and no border, only the icons and labels are visible.
    Clear,
}

/// Whether item labels get a shadow to stay legible over a transparent space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelShadow {
    /// Only when the effective tint is below the legibility threshold.
    #[default]
    Auto,
    On,
    Off,
}

/// Corner radii in pixels at 96 DPI. `SMALL` and `WINDOWS` match the Windows
/// 11 design guidance (4 px for small surfaces, 8 px for windows), and DWM
/// draws them natively; other radii are drawn by DeskSpace.
/// https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/apply-rounded-corners
pub mod corner {
    pub const SQUARE: u8 = 0;
    pub const SMALL: u8 = 4;
    pub const WINDOWS: u8 = 8;
    pub const ROUND: u8 = 16;
    pub const MAX: u8 = 24;
}

/// The shell draws its icon overlay (the shortcut arrow) a touch larger than
/// most people want, so DeskSpace's default is smaller than Explorer's 100%.
pub const OVERLAY_SIZE_DEFAULT: u8 = 80;
/// The bounds an overlay size is clamped to. 100% matches Explorer exactly.
pub const OVERLAY_SIZE_RANGE: (u8, u8) = (40, 100);
/// The bounds an item icon size is clamped to, as a percent of the default.
pub const ICON_SIZE_RANGE: (u8, u8) = (50, 200);
/// The bounds an item label size is clamped to, as a percent of the default.
pub const LABEL_SIZE_RANGE: (u8, u8) = (60, 160);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub mode: ThemeMode,
    pub backdrop: Backdrop,
    /// Corner radius in pixels, `0..=corner::MAX`. Also reads the older
    /// `corners = "round" | "small" | "square"` setting.
    #[serde(alias = "corners", deserialize_with = "radius_or_style")]
    pub corner_radius: u8,
    /// How strongly the tint covers the backdrop, in percent.
    pub tint_opacity: u8,
    /// How much darker (dark mode) or lighter (light mode) the title bar is
    /// than the body, in percent.
    pub title_shading: u8,
    pub show_border: bool,
    /// Item label lines, 1 or 2.
    pub label_lines: u8,
    /// Item label font size as a percent of the default (100).
    pub label_size: u8,
    /// Item icon size as a percent of the default (100). Scales the icon and
    /// the grid cell it sits in, so a larger icon gets more room.
    pub icon_size: u8,
    /// The shell's icon overlay (the shortcut arrow, a sync badge) as a percent
    /// of the icon size (100 is Explorer's own proportion).
    pub overlay_size: u8,
    pub label_shadow: LabelShadow,
    /// When false, `tint` and `text` are ignored and the palette follows the
    /// light or dark mode.
    pub custom_colors: bool,
    pub tint: Color,
    pub text: Color,
    /// Mirror Explorer's display settings (item check boxes, filename
    /// extensions) instead of hiding them, so spaces match the desktop.
    pub match_explorer: bool,
    /// Replace the selection highlight and border with Windows' accent color.
    pub use_accent: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            mode: ThemeMode::System,
            backdrop: Backdrop::Acrylic,
            corner_radius: corner::WINDOWS,
            tint_opacity: 40,
            title_shading: 30,
            show_border: true,
            label_lines: 2,
            label_size: 100,
            icon_size: 100,
            overlay_size: OVERLAY_SIZE_DEFAULT,
            label_shadow: LabelShadow::Auto,
            custom_colors: false,
            tint: DARK_TINT,
            text: DARK_TEXT,
            match_explorer: true,
            use_accent: false,
        }
    }
}

fn radius_or_style<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RadiusOrStyle {
        Radius(i64),
        Style(String),
    }
    Ok(match RadiusOrStyle::deserialize(deserializer)? {
        RadiusOrStyle::Radius(radius) => radius.clamp(0, corner::MAX as i64) as u8,
        RadiusOrStyle::Style(style) => match style.as_str() {
            "square" => corner::SQUARE,
            "small" => corner::SMALL,
            _ => corner::WINDOWS,
        },
    })
}

/// Windows 11 dark and light base colors (Fluent "solid background" and
/// "text primary" tokens).
const DARK_TINT: Color = Color::rgb(32, 32, 32);
const DARK_TEXT: Color = Color::rgb(255, 255, 255);
const LIGHT_TINT: Color = Color::rgb(243, 243, 243);
const LIGHT_TEXT: Color = Color::rgb(26, 26, 26);

/// Colors of an inline rename field, matching the box Explorer puts over the
/// name it is editing: an opaque system-window fill, a border, and the system
/// highlight for the selected text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditColors {
    pub fill: Color,
    pub text: Color,
    pub selection: Color,
    pub selection_text: Color,
    pub border: Color,
}

/// Windows' default text-selection highlight (`COLOR_HIGHLIGHT`).
const HIGHLIGHT: Color = Color::rgb(0x00, 0x78, 0xD4);

impl EditColors {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                fill: Color::rgb(0x2B, 0x2B, 0x2B),
                text: Color::rgb(0xFF, 0xFF, 0xFF),
                selection: HIGHLIGHT,
                selection_text: Color::rgb(0xFF, 0xFF, 0xFF),
                border: Color::rgb(0x5A, 0x5A, 0x5A),
            }
        } else {
            Self {
                fill: Color::rgb(0xFF, 0xFF, 0xFF),
                text: Color::rgb(0x00, 0x00, 0x00),
                selection: HIGHLIGHT,
                selection_text: Color::rgb(0xFF, 0xFF, 0xFF),
                border: Color::rgb(0x00, 0x00, 0x00),
            }
        }
    }
}

/// Concrete colors for drawing a space, resolved from a `Theme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    pub tint: Color,
    pub title_fill: Color,
    pub title_text: Color,
    pub item_text: Color,
    pub hover_fill: Color,
    pub selected_fill: Color,
    pub border: Color,
    pub edit: EditColors,
}

impl Palette {
    /// Replaces the selection highlight and border with `accent` at the alpha
    /// each one already has, leaving every other color alone.
    pub fn with_accent(mut self, accent: Color) -> Self {
        self.selected_fill = accent.with_alpha(self.selected_fill.a);
        self.border = accent.with_alpha(self.border.a);
        self
    }
}

impl Backdrop {
    /// Whether the backdrop is a blur material drawn by DWM.
    pub const fn is_blurred(self) -> bool {
        matches!(self, Self::Acrylic | Self::Blur)
    }
}

impl Theme {
    /// The corner radius spaces are drawn with. Blur materials fill the whole
    /// window rectangle and ignore window and blur regions, so only DWM's own
    /// corners (square, small, or Windows) can clip them; other radii showed
    /// square patches of blur outside the corners. Blurred spaces therefore
    /// use the nearest DWM radius at or below the chosen one, and the chosen
    /// radius applies again with Clear or Solid.
    pub fn effective_corner_radius(&self) -> u8 {
        if !self.backdrop.is_blurred() {
            return self.corner_radius;
        }
        match self.corner_radius {
            0..corner::SMALL => corner::SQUARE,
            corner::SMALL..corner::WINDOWS => corner::SMALL,
            _ => corner::WINDOWS,
        }
    }

    pub fn is_dark(&self, system_dark: bool) -> bool {
        match self.mode {
            ThemeMode::System => system_dark,
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
        }
    }

    pub fn palette(&self, system_dark: bool) -> Palette {
        let dark = self.is_dark(system_dark);
        let (tint, text) = match (self.custom_colors, dark) {
            (true, _) => (self.tint, self.text),
            (false, true) => (DARK_TINT, DARK_TEXT),
            (false, false) => (LIGHT_TINT, LIGHT_TEXT),
        };
        let tint_alpha = match self.backdrop {
            Backdrop::Solid => 255,
            Backdrop::Acrylic | Backdrop::Blur | Backdrop::Clear => {
                percent_to_alpha(self.tint_opacity)
            }
        };
        let shade = if dark {
            Color::rgb(0, 0, 0)
        } else {
            Color::rgb(255, 255, 255)
        };
        // Highlights are the text color at low alpha, the way Fluent's subtle
        // fills are defined, so they stay visible on any custom tint.
        let highlight = if text.luminance() > 0.5 {
            Color::rgb(255, 255, 255)
        } else {
            Color::rgb(0, 0, 0)
        };
        Palette {
            dark,
            tint: tint.with_alpha(tint_alpha),
            title_fill: shade.with_alpha(percent_to_alpha(self.title_shading) / 2),
            title_text: text,
            item_text: text,
            hover_fill: highlight.with_alpha(0x18),
            selected_fill: highlight.with_alpha(0x2C),
            border: highlight.with_alpha(0x30),
            // The rename field follows Windows' app theme, as Explorer's does,
            // rather than the space's own Color mode override.
            edit: EditColors::new(system_dark),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_rgb() {
        let color = Color::rgb(0x12, 0x34, 0x56);
        assert_eq!(
            Color::from_hex(&color.to_hex()),
            Some(color.with_alpha(255))
        );
        assert_eq!(color.to_rgb_hex(), "#123456");
    }

    #[test]
    fn parses_short_and_long_hex() {
        assert_eq!(Color::from_hex("#FF0000"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(
            Color::from_hex("#FF000080"),
            Some(Color::rgba(255, 0, 0, 128))
        );
        assert_eq!(Color::from_hex("nope"), None);
    }

    #[test]
    fn percent_maps_to_full_alpha_range() {
        assert_eq!(percent_to_alpha(0), 0);
        assert_eq!(percent_to_alpha(100), 255);
        assert_eq!(percent_to_alpha(250), 255);
        assert_eq!(percent_to_alpha(50), 128);
    }

    #[test]
    fn fade_scales_alpha() {
        assert_eq!(Color::rgb(1, 2, 3).fade(255).a, 255);
        assert_eq!(Color::rgb(1, 2, 3).fade(0).a, 0);
        assert_eq!(Color::rgba(1, 2, 3, 200).fade(128).a, 100);
    }

    #[test]
    fn system_mode_follows_the_os() {
        let theme = Theme::default();
        assert!(theme.palette(true).dark);
        assert!(!theme.palette(false).dark);
        let light = Theme {
            mode: ThemeMode::Light,
            ..Theme::default()
        };
        assert!(!light.palette(true).dark);
    }

    #[test]
    fn solid_backdrop_is_opaque() {
        let theme = Theme {
            backdrop: Backdrop::Solid,
            tint_opacity: 10,
            ..Theme::default()
        };
        assert_eq!(theme.palette(true).tint.a, 255);
        assert_eq!(Theme::default().palette(true).tint.a, percent_to_alpha(40));
    }

    #[test]
    fn clear_backdrop_can_be_fully_transparent() {
        let theme = Theme {
            backdrop: Backdrop::Clear,
            tint_opacity: 0,
            ..Theme::default()
        };
        assert_eq!(theme.palette(true).tint.a, 0);
    }

    #[test]
    fn blurred_spaces_use_the_nearest_dwm_radius_below() {
        let radius = |backdrop, corner_radius| {
            Theme {
                backdrop,
                corner_radius,
                ..Theme::default()
            }
            .effective_corner_radius()
        };
        assert_eq!(radius(Backdrop::Acrylic, 24), corner::WINDOWS);
        assert_eq!(radius(Backdrop::Blur, 8), corner::WINDOWS);
        assert_eq!(radius(Backdrop::Acrylic, 6), corner::SMALL);
        assert_eq!(radius(Backdrop::Blur, 3), corner::SQUARE);
        assert_eq!(radius(Backdrop::Clear, 24), 24);
        assert_eq!(radius(Backdrop::Solid, 13), 13);
    }

    #[test]
    fn edit_field_contrasts_like_explorer_in_both_modes() {
        for dark in [true, false] {
            let edit = Theme::default().palette(dark).edit;
            assert_eq!(edit.fill.a, 255);
            assert_eq!(edit.selection_text.a, 255);
            // The fill is light in light mode and dark in dark mode, with the
            // text always contrasting against it.
            assert_eq!(edit.fill.luminance() < 0.5, dark);
            assert!((edit.fill.luminance() - edit.text.luminance()).abs() > 0.5);
            // The highlight matches Windows' default selection blue.
            assert_eq!(
                (edit.selection.r, edit.selection.g, edit.selection.b),
                (0, 120, 212)
            );
        }
    }

    #[test]
    fn custom_colors_override_the_mode_palette() {
        let theme = Theme {
            custom_colors: true,
            tint: Color::rgb(10, 60, 120),
            text: Color::rgb(250, 240, 200),
            ..Theme::default()
        };
        let palette = theme.palette(false);
        assert_eq!(palette.tint.r, 10);
        assert_eq!(palette.title_text, Color::rgb(250, 240, 200));
        assert_eq!(palette.hover_fill.r, 255);
    }

    #[test]
    fn accent_replaces_selection_and_border_keeping_alpha() {
        let palette = Theme::default().palette(true);
        let accent = Color::rgb(0x00, 0x78, 0xD4);
        let accented = palette.with_accent(accent);
        assert_eq!(
            accented.selected_fill,
            accent.with_alpha(palette.selected_fill.a)
        );
        assert_eq!(accented.border, accent.with_alpha(palette.border.a));
        assert_eq!(accented.tint, palette.tint);
        assert_eq!(accented.hover_fill, palette.hover_fill);
    }
}
