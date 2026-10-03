//! Data descriptions of native UI: popup menus and the settings window. The
//! app builds these; `win32` turns them into controls. Nothing here touches
//! the OS.

use deskspace_core::theme::Color;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Item {
        id: u32,
        label: String,
        checked: bool,
        enabled: bool,
    },
    Separator,
}

impl MenuEntry {
    pub fn item(id: u32, label: impl Into<String>) -> Self {
        Self::Item {
            id,
            label: label.into(),
            checked: false,
            enabled: true,
        }
    }

    pub fn checked(self, value: bool) -> Self {
        match self {
            Self::Item {
                id, label, enabled, ..
            } => Self::Item {
                id,
                label,
                checked: value,
                enabled,
            },
            Self::Separator => Self::Separator,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsForm {
    pub title: String,
    pub pages: Vec<SettingsPage>,
    pub apply_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsPage {
    pub title: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Stable id reported back in `SettingsEvent`s. Unique across all pages.
    pub id: u32,
    pub label: String,
    pub kind: FieldKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    /// Bold section title; `label` is the text.
    Heading,
    /// Read-only paragraph; `label` is the text.
    Note,
    Choice {
        options: Vec<String>,
        selected: usize,
    },
    Toggle {
        on: bool,
    },
    Slider {
        min: i32,
        max: i32,
        value: i32,
        suffix: String,
    },
    Color {
        color: Color,
    },
    Text {
        value: String,
        read_only: bool,
    },
    Button {
        caption: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    Choice(usize),
    Toggle(bool),
    Slider(i32),
    Color(Color),
    Text(String),
}

impl Field {
    fn new(id: u32, label: impl Into<String>, kind: FieldKind) -> Self {
        Self {
            id,
            label: label.into(),
            kind,
            enabled: true,
        }
    }

    pub fn heading(id: u32, text: impl Into<String>) -> Self {
        Self::new(id, text, FieldKind::Heading)
    }

    pub fn note(id: u32, text: impl Into<String>) -> Self {
        Self::new(id, text, FieldKind::Note)
    }

    pub fn choice(id: u32, label: impl Into<String>, options: &[&str], selected: usize) -> Self {
        Self::new(
            id,
            label,
            FieldKind::Choice {
                options: options.iter().map(|option| option.to_string()).collect(),
                selected,
            },
        )
    }

    pub fn toggle(id: u32, label: impl Into<String>, on: bool) -> Self {
        Self::new(id, label, FieldKind::Toggle { on })
    }

    pub fn slider(id: u32, label: impl Into<String>, range: (i32, i32), value: i32) -> Self {
        Self::new(
            id,
            label,
            FieldKind::Slider {
                min: range.0,
                max: range.1,
                value,
                suffix: "%".to_string(),
            },
        )
    }

    pub fn color(id: u32, label: impl Into<String>, color: Color) -> Self {
        Self::new(id, label, FieldKind::Color { color })
    }

    pub fn text(id: u32, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(
            id,
            label,
            FieldKind::Text {
                value: value.into(),
                read_only: false,
            },
        )
    }

    pub fn read_only_text(id: u32, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(
            id,
            label,
            FieldKind::Text {
                value: value.into(),
                read_only: true,
            },
        )
    }

    pub fn button(id: u32, label: impl Into<String>, caption: impl Into<String>) -> Self {
        Self::new(
            id,
            label,
            FieldKind::Button {
                caption: caption.into(),
            },
        )
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl SettingsForm {
    pub fn field(&self, id: u32) -> Option<&Field> {
        self.pages
            .iter()
            .flat_map(|page| page.fields.iter())
            .find(|field| field.id == id)
    }

    /// Whether `other` has the same pages and fields in the same order, so the
    /// existing controls can be updated in place instead of rebuilt.
    pub fn same_shape(&self, other: &Self) -> bool {
        self.pages.len() == other.pages.len()
            && self.pages.iter().zip(&other.pages).all(|(a, b)| {
                a.fields.len() == b.fields.len()
                    && a.fields.iter().zip(&b.fields).all(|(x, y)| {
                        x.id == y.id
                            && std::mem::discriminant(&x.kind) == std::mem::discriminant(&y.kind)
                    })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(fields: Vec<Field>) -> SettingsForm {
        SettingsForm {
            title: "Settings".into(),
            pages: vec![SettingsPage {
                title: "Page".into(),
                fields,
            }],
            apply_enabled: false,
        }
    }

    #[test]
    fn same_shape_ignores_values_but_not_kinds() {
        let a = form(vec![Field::toggle(1, "A", true)]);
        let b = form(vec![Field::toggle(1, "A", false).enabled(false)]);
        let c = form(vec![Field::text(1, "A", "x")]);
        assert!(a.same_shape(&b));
        assert!(!a.same_shape(&c));
    }

    #[test]
    fn field_lookup_spans_pages() {
        let mut settings = form(vec![Field::toggle(1, "A", true)]);
        settings.pages.push(SettingsPage {
            title: "Second".into(),
            fields: vec![Field::note(2, "hello")],
        });
        assert_eq!(settings.field(2).map(|f| f.label.as_str()), Some("hello"));
        assert!(settings.field(3).is_none());
    }
}
