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

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
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
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub color_key: Color,
    pub background_fill: Color,
    pub space_border: Color,
    pub space_shadow: Color,
    pub title_fill: Color,
    pub title_text: Color,
    pub item_fill: Color,
    pub item_border: Color,
    pub item_text: Color,
    pub selection: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            color_key: Color::rgb(255, 0, 255),
            background_fill: Color::rgb(28, 32, 40),
            space_border: Color::rgb(64, 72, 88),
            space_shadow: Color::rgb(12, 14, 18),
            title_fill: Color::rgb(40, 46, 58),
            title_text: Color::rgb(232, 236, 244),
            item_fill: Color::rgb(58, 66, 82),
            item_border: Color::rgb(86, 96, 116),
            item_text: Color::rgb(220, 226, 238),
            selection: Color::rgb(96, 156, 255),
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
}
