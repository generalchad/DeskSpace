//! What each kind of text in DeskSpace is drawn with (the type scale): a font
//! family, a size at 96 DPI, and a weight per role. The platform turns a role
//! into an `HFONT` for GDI and an `IDWriteTextFormat` for Direct2D, so the
//! settings window, the debug window, dialogs, and the spaces' own text all
//! read one table.
//!
//! The values are 96-DPI pixels, not points: GDI's `lfHeight` and
//! DirectWrite's text-format size are the same number then, and a role's size
//! means the same thing whichever painter draws it. A family of `""` means
//! Windows' message font (Segoe UI Variable Text on Windows 11, else Segoe
//! UI).

use std::fmt;

/// One kind of text, from the smallest to the largest role.
///
/// `ALL` is the picker's and the debug box's order; a role is never removed or
/// renumbered, so a saved name keeps meaning what it meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Body text: labels, check boxes, buttons, values, and the debug console.
    Body,
    /// A page's largest heading.
    Heading1,
    /// A section or dialog heading.
    Heading2,
    /// A group's heading, and the settings window's semibold rows.
    Heading3,
    /// A note or description under a field.
    Caption,
    /// A space's title bar.
    SpaceTitle,
    /// An item's label.
    ItemLabel,
    /// A Details, Tiles, or Content cell.
    Details,
    /// A space's placeholder text (an unavailable lens, or "No items match").
    Placeholder,
    /// The zoom pop-up's percentage.
    Zoom,
}

impl Role {
    pub const ALL: [Self; 10] = [
        Self::Body,
        Self::Heading1,
        Self::Heading2,
        Self::Heading3,
        Self::Caption,
        Self::SpaceTitle,
        Self::ItemLabel,
        Self::Details,
        Self::Placeholder,
        Self::Zoom,
    ];

    /// The role's name in the Debug box.
    pub fn label(self) -> &'static str {
        match self {
            Self::Body => "Body",
            Self::Heading1 => "Heading 1",
            Self::Heading2 => "Heading 2",
            Self::Heading3 => "Heading 3",
            Self::Caption => "Caption",
            Self::SpaceTitle => "Space title",
            Self::ItemLabel => "Item label",
            Self::Details => "Details",
            Self::Placeholder => "Placeholder",
            Self::Zoom => "Zoom",
        }
    }

    /// What this role is drawn for, when the settings and debug windows do not
    /// show it directly.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::Body => None,
            Self::SpaceTitle => Some("The title bar of every space."),
            Self::ItemLabel => {
                Some("Item labels; the space's label-size setting scales this size.")
            }
            Self::Details => Some("Details, Tiles, and Content cells."),
            Self::Placeholder => Some("A space's message when it has nothing to show."),
            Self::Zoom => Some("The zoom pop-up's percentage."),
            _ => None,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// The allowed size of a role, at 96 DPI, and the sizes the Debug box offers.
pub const FONT_SIZE_RANGE: (f32, f32) = (6.0, 48.0);
pub const FONT_SIZE_STEP: f32 = 0.5;
/// The weights the Debug box offers, and their Windows names.
pub const FONT_WEIGHTS: &[(u16, &str)] = &[(400, "Regular"), (600, "Semibold"), (700, "Bold")];

/// One role's font.
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    /// A family name, or `""` for Windows' message font.
    pub family: String,
    /// The size at 96 DPI, in pixels.
    pub size: f32,
    /// 400, 600, or 700, as `FONT_WEIGHTS` lists them.
    pub weight: u16,
}

impl Font {
    pub fn new(family: impl Into<String>, size: f32, weight: u16) -> Self {
        Self {
            family: family.into(),
            size,
            weight,
        }
    }

    /// The same font within the allowed ranges: an out-of-range size clamps,
    /// and a weight no preset names becomes the nearest preset.
    pub fn clamped(&self) -> Self {
        Self {
            family: self.family.trim().to_owned(),
            size: if self.size.is_finite() {
                self.size.clamp(FONT_SIZE_RANGE.0, FONT_SIZE_RANGE.1)
            } else {
                Self::default_for(Role::Body).size
            },
            weight: nearest_weight(self.weight),
        }
    }

    /// The default font for `role`, which is what a fresh run starts with.
    pub fn default_for(role: Role) -> Self {
        let (size, weight) = match role {
            Role::Body | Role::Caption => (12.0, 400),
            Role::Heading1 => (16.0, 600),
            Role::Heading2 => (14.0, 600),
            Role::Heading3 => (12.0, 600),
            Role::SpaceTitle => (13.0, 600),
            Role::ItemLabel | Role::Details | Role::Placeholder => (12.0, 400),
            Role::Zoom => (14.0, 600),
        };
        Self::new("", size, weight)
    }
}

impl Default for Font {
    fn default() -> Self {
        Self::default_for(Role::Body)
    }
}

fn nearest_weight(weight: u16) -> u16 {
    FONT_WEIGHTS
        .iter()
        .map(|(value, _)| *value)
        .min_by_key(|value| value.abs_diff(weight))
        .unwrap_or(400)
}

/// Every role's font. `Default` is the type scale every run starts with.
#[derive(Debug, Clone, PartialEq)]
pub struct Typography {
    fonts: [Font; Role::ALL.len()],
}

impl Typography {
    pub fn new() -> Self {
        Self {
            fonts: Role::ALL.map(Font::default_for),
        }
    }

    pub fn font(&self, role: Role) -> &Font {
        &self.fonts[role as usize]
    }

    /// Sets `role`'s font, clamped to the allowed ranges.
    pub fn set(&mut self, role: Role, font: Font) {
        self.fonts[role as usize] = font.clamped();
    }

    /// Every role's font in `Role::ALL` order.
    pub fn iter(&self) -> impl Iterator<Item = (Role, &Font)> {
        Role::ALL.into_iter().map(|role| (role, self.font(role)))
    }

    /// Every role within the allowed ranges.
    pub fn clamped(&self) -> Self {
        Role::ALL
            .into_iter()
            .fold(Self::new(), |mut typography, role| {
                typography.set(role, self.font(role).clone());
                typography
            })
    }

    /// The weight GDI and DirectWrite draw a role at, as a number both take.
    pub fn weight_of(&self, role: Role) -> u16 {
        self.font(role).weight
    }
}

impl Default for Typography {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_has_a_default_within_the_range() {
        let typography = Typography::new();
        for (role, font) in typography.iter() {
            assert!((FONT_SIZE_RANGE.0..=FONT_SIZE_RANGE.1).contains(&font.size));
            assert!(
                FONT_WEIGHTS
                    .iter()
                    .any(|(weight, _)| *weight == font.weight),
                "{role}"
            );
        }
    }

    #[test]
    fn a_role_keeps_its_font_apart_from_the_others() {
        let mut typography = Typography::new();
        let heading = typography.font(Role::Heading1).clone();
        typography.set(Role::Body, Font::new("Consolas", 10.0, 400));
        assert_eq!(typography.font(Role::Body).family, "Consolas");
        assert_eq!(typography.font(Role::Heading1), &heading);
    }

    #[test]
    fn an_out_of_range_font_clamps() {
        let mut typography = Typography::new();
        typography.set(Role::Body, Font::new("  Georgia  ", 100.0, 500));
        let font = typography.font(Role::Body);
        assert_eq!(font.family, "Georgia", "trimmed");
        assert_eq!(font.size, FONT_SIZE_RANGE.1);
        assert_eq!(font.weight, 400, "500 is nearest Regular");
        typography.set(Role::Body, Font::new("", f32::NAN, 600));
        assert_eq!(typography.font(Role::Body).size, 12.0);
    }

    #[test]
    fn the_role_names_are_unique() {
        let mut labels: Vec<&str> = Role::ALL.iter().map(|role| role.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), Role::ALL.len());
    }
}
