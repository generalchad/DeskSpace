//! Data descriptions of native UI: popup menus and the settings window. The
//! app builds these; `win32` turns them into controls. Nothing here touches
//! the OS.

use deskspace_core::theme::Color;

/// The mouse pointer shown over a space window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CursorShape {
    #[default]
    Arrow,
    /// Left or right edge.
    SizeWestEast,
    /// Top or bottom edge.
    SizeNorthSouth,
    /// Top-left or bottom-right corner.
    SizeNorthWestSouthEast,
    /// Top-right or bottom-left corner.
    SizeNorthEastSouthWest,
}

/// What a custom shortcut opens (the "Custom shortcut" dialog, F-53).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutTarget {
    /// A file, folder, or program; written as a `.lnk`.
    Path(std::path::PathBuf),
    /// A web address or other URL (`https:`, `mailto:`, `steam:`, ...);
    /// written as a `.url` Internet shortcut.
    Url(String),
}

/// A shortcut described by hand: the target, its command-line arguments
/// (ignored for a URL), and the name it is listed under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomShortcut {
    pub target: ShortcutTarget,
    pub arguments: String,
    pub name: String,
}

impl ShortcutTarget {
    /// Reads what the user typed as a target: a URL when it starts with a
    /// scheme (two or more letters, so `C:` stays a drive) or with `www.`,
    /// otherwise a path, still unresolved. Surrounding quotes and spaces are
    /// dropped. `None` for empty text.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_matches('"').trim();
        if text.is_empty() {
            return None;
        }
        if has_url_scheme(text) {
            return Some(Self::Url(text.to_string()));
        }
        if text.len() > 4 && text[..4].eq_ignore_ascii_case("www.") {
            return Some(Self::Url(format!("https://{text}")));
        }
        Some(Self::Path(std::path::PathBuf::from(text)))
    }

    /// The name a shortcut to this target gets unless the user types one:
    /// a path's file stem (a folder's or drive's own name), or a URL's host.
    pub fn suggested_name(&self) -> String {
        match self {
            Self::Path(path) => path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().trim_end_matches('\\').to_string()),
            Self::Url(url) => {
                let rest = match url.split_once("://") {
                    Some((_, rest)) => rest,
                    None => url.split_once(':').map_or(url.as_str(), |(_, rest)| rest),
                };
                let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
                let host = host.trim_start_matches("www.");
                if host.is_empty() {
                    url.clone()
                } else {
                    host.to_string()
                }
            }
        }
    }

    /// The shortcut file's extension: `url` for a URL, else `lnk`.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Path(_) => "lnk",
            Self::Url(_) => "url",
        }
    }
}

/// Whether `text` starts with a URL scheme (RFC 3986 section 3.1: a letter,
/// then letters, digits, `+`, `-`, or `.`, then `:`). One letter is a drive.
fn has_url_scheme(text: &str) -> bool {
    let Some((scheme, _)) = text.split_once(':') else {
        return false;
    };
    scheme.len() >= 2
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Item {
        id: u32,
        label: String,
        checked: bool,
        enabled: bool,
    },
    Separator,
    /// A nested menu. Its own entries carry the ids.
    Submenu {
        label: String,
        entries: Vec<MenuEntry>,
        enabled: bool,
    },
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

    pub fn submenu(label: impl Into<String>, entries: Vec<MenuEntry>) -> Self {
        Self::Submenu {
            label: label.into(),
            entries,
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
            other => other,
        }
    }

    pub fn enabled(self, value: bool) -> Self {
        match self {
            Self::Item {
                id, label, checked, ..
            } => Self::Item {
                id,
                label,
                checked,
                enabled: value,
            },
            Self::Submenu { label, entries, .. } => Self::Submenu {
                label,
                entries,
                enabled: value,
            },
            Self::Separator => Self::Separator,
        }
    }
}

/// The settings window's navigation: a scopes pane and a categories pane. The
/// platform renders the groups and reports which entry was picked; the app owns
/// what each entry means.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsNav {
    /// What the settings apply to, in a pane left of the categories (for
    /// example "Global", then each space indented under it). Empty leaves the
    /// pane out. Fixed when the window is built: a form that starts without
    /// scopes never gains the pane.
    pub scopes: Vec<SettingsScope>,
    /// The selected scope, an index into `scopes`.
    pub selected_scope: usize,
    /// The categories pane.
    pub groups: Vec<SettingsNavGroup>,
    /// The selected entry, indexing the flattened [`SettingsNav::entries`]
    /// order. Group titles are not selectable.
    pub selected: usize,
    /// A title and an optional switch shown above the page.
    pub header: Option<SettingsHeader>,
}

/// One row of the scopes pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsScope {
    pub label: String,
    /// Indents the row under the one above it.
    pub indent: bool,
    /// Draws the row in disabled text, for a scope that only inherits.
    pub dimmed: bool,
}

/// The line above the page: a title, a switch the app can use to unlock the
/// page (reported as `SettingsEvent::HeaderSwitch`), and an optional action
/// button (reported as `SettingsEvent::HeaderButton`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsHeader {
    pub title: String,
    pub switch: Option<SettingsSwitch>,
    /// Shown to the left of the switch only when the header has one. Absent
    /// hides it, so the same header space can hold a button for one scope and
    /// not another (the page shape is unaffected).
    pub button: Option<SettingsButton>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsSwitch {
    pub label: String,
    pub on: bool,
}

/// An action button in the header. The platform reports a press; the app owns
/// what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsButton {
    pub label: String,
}

/// A titled group of rail entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsNavGroup {
    /// Empty leaves out the group's title row.
    pub title: String,
    pub entries: Vec<SettingsNavEntry>,
}

/// One selectable row of the rail. Several entries may point at the same page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsNavEntry {
    pub label: String,
    /// Index into [`SettingsForm::pages`].
    pub page: usize,
    /// Draws a marker dot (for example, a space with appearance overrides).
    pub marked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsForm {
    pub title: String,
    pub pages: Vec<SettingsPage>,
    pub apply_enabled: bool,
    /// The navigation rail: the scopes and categories panes.
    pub nav: SettingsNav,
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

/// The kind of control a `Field` is. A new kind is a variant plus its builder
/// (`settings_window/build.rs`), its `plan_field` diff (`update.rs`), and its
/// command handling (`commands.rs`); prefer composing the existing kinds.
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
        /// When true the value on the right is an editable number box instead
        /// of a static label, so a value can be typed directly.
        editable: bool,
        /// Labeled tick marks under the slider.
        marks: Vec<SliderMark>,
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
    /// A styled picture of one space whose items can be deleted (an X on
    /// each) and added to (a trailing + tile). Clicks are reported as
    /// `SettingsEvent::PreviewRemove` and `SettingsEvent::PreviewAdd`.
    SpacePreview(SpacePreview),
}

/// What a `FieldKind::SpacePreview` shows: the space's title bar and colors,
/// approximated as opaque fills, and its items in grid order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpacePreview {
    pub title: String,
    /// The space's body, opaque.
    pub fill: Color,
    /// The title bar, opaque.
    pub title_fill: Color,
    /// Title and label text.
    pub text: Color,
    pub items: Vec<PreviewItem>,
}

/// One item in a `SpacePreview`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewItem {
    pub label: String,
    pub icon: Option<crate::render::Icon>,
}

impl Default for SpacePreview {
    fn default() -> Self {
        Self {
            title: String::new(),
            fill: Color::rgb(0x20, 0x20, 0x20),
            title_fill: Color::rgb(0x18, 0x18, 0x18),
            text: Color::rgb(0xFF, 0xFF, 0xFF),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliderMark {
    pub value: i32,
    pub label: String,
}

impl SliderMark {
    pub fn new(value: i32, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
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
                editable: false,
                marks: Vec::new(),
            },
        )
    }

    /// Sets the unit shown after a slider's value (`%` by default).
    pub fn suffix(mut self, text: impl Into<String>) -> Self {
        if let FieldKind::Slider { suffix, .. } = &mut self.kind {
            *suffix = text.into();
        }
        self
    }

    /// Makes a slider's value an editable number box, so it can be typed.
    pub fn editable(mut self) -> Self {
        if let FieldKind::Slider { editable, .. } = &mut self.kind {
            *editable = true;
        }
        self
    }

    /// Adds labeled tick marks to a slider.
    pub fn marks(mut self, new_marks: Vec<SliderMark>) -> Self {
        if let FieldKind::Slider { marks, .. } = &mut self.kind {
            *marks = new_marks;
        }
        self
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

    pub fn space_preview(id: u32, preview: SpacePreview) -> Self {
        Self::new(id, "", FieldKind::SpacePreview(preview))
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl SettingsNav {
    /// Every selectable entry, flattened across groups in list order.
    pub fn entries(&self) -> impl Iterator<Item = &SettingsNavEntry> {
        self.groups.iter().flat_map(|group| group.entries.iter())
    }

    /// The page that the `entry`-th flattened entry points at.
    pub fn page_of(&self, entry: usize) -> Option<usize> {
        self.entries().nth(entry).map(|entry| entry.page)
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
    /// existing controls can be updated in place instead of rebuilt. The
    /// navigation rail is not part of the shape: its entries change as spaces
    /// are added or reset while the window is open.
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
            nav: SettingsNav {
                groups: vec![SettingsNavGroup {
                    title: "Settings".into(),
                    entries: vec![nav_entry("Page", 0, false)],
                }],
                ..Default::default()
            },
        }
    }

    fn nav_entry(label: &str, page: usize, marked: bool) -> SettingsNavEntry {
        SettingsNavEntry {
            label: label.into(),
            page,
            marked,
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
    fn nav_flattens_entries_across_groups() {
        let nav = SettingsNav {
            groups: vec![
                SettingsNavGroup {
                    title: "Settings".into(),
                    entries: vec![nav_entry("Appearance", 0, false)],
                },
                SettingsNavGroup {
                    title: "Spaces".into(),
                    entries: vec![nav_entry("Work", 7, true), nav_entry("Games", 7, false)],
                },
            ],
            selected: 2,
            ..Default::default()
        };
        let labels: Vec<&str> = nav.entries().map(|entry| entry.label.as_str()).collect();
        assert_eq!(labels, ["Appearance", "Work", "Games"]);
        // Several entries may share a page.
        assert_eq!(nav.page_of(1), Some(7));
        assert_eq!(nav.page_of(2), Some(7));
        assert_eq!(nav.page_of(3), None);
    }

    #[test]
    fn nav_is_not_part_of_the_form_shape() {
        let mut a = form(vec![Field::toggle(1, "A", true)]);
        let mut b = form(vec![Field::toggle(1, "A", true)]);
        a.nav = SettingsNav {
            groups: vec![SettingsNavGroup {
                title: "Spaces".into(),
                entries: vec![nav_entry("Work", 0, true)],
            }],
            selected: 0,
            ..Default::default()
        };
        b.nav = SettingsNav::default();
        assert!(a.same_shape(&b));
    }

    #[test]
    fn submenus_hold_their_own_entries_and_toggle_state() {
        let submenu = MenuEntry::submenu(
            "Appearance",
            vec![MenuEntry::item(1, "Opaque").checked(true)],
        );
        match &submenu {
            MenuEntry::Submenu {
                label,
                entries,
                enabled,
            } => {
                assert_eq!(label, "Appearance");
                assert_eq!(entries.len(), 1);
                assert!(*enabled);
            }
            other => panic!("expected a submenu, got {other:?}"),
        }
        // `checked` is a no-op on a submenu; `enabled` applies to it.
        assert!(matches!(
            submenu.clone().checked(false),
            MenuEntry::Submenu {
                entries,
                ..
            } if matches!(entries[0], MenuEntry::Item { checked: true, .. })
        ));
        assert!(matches!(
            submenu.enabled(false),
            MenuEntry::Submenu { enabled: false, .. }
        ));
    }

    #[test]
    fn typed_targets_are_urls_or_paths() {
        use std::path::PathBuf;
        assert_eq!(ShortcutTarget::parse("   "), None);
        assert_eq!(
            ShortcutTarget::parse("https://example.com/a"),
            Some(ShortcutTarget::Url("https://example.com/a".into()))
        );
        assert_eq!(
            ShortcutTarget::parse("www.example.com"),
            Some(ShortcutTarget::Url("https://www.example.com".into()))
        );
        assert_eq!(
            ShortcutTarget::parse("mailto:me@example.com"),
            Some(ShortcutTarget::Url("mailto:me@example.com".into()))
        );
        // A drive letter is not a scheme, and quotes are dropped.
        assert_eq!(
            ShortcutTarget::parse("\"C:\\Tools\\app.exe\""),
            Some(ShortcutTarget::Path(PathBuf::from(r"C:\Tools\app.exe")))
        );
        assert_eq!(
            ShortcutTarget::parse("notepad"),
            Some(ShortcutTarget::Path(PathBuf::from("notepad")))
        );
    }

    #[test]
    fn suggested_names_come_from_the_stem_or_the_host() {
        use std::path::PathBuf;
        let name = |target: ShortcutTarget| target.suggested_name();
        assert_eq!(
            name(ShortcutTarget::Path(PathBuf::from(r"C:\Tools\app.exe"))),
            "app"
        );
        assert_eq!(name(ShortcutTarget::Path(PathBuf::from(r"D:\"))), "D:");
        assert_eq!(
            name(ShortcutTarget::Url("https://www.example.com/x?y".into())),
            "example.com"
        );
        assert_eq!(
            name(ShortcutTarget::Url("mailto:me@example.com".into())),
            "me@example.com"
        );
        assert_eq!(ShortcutTarget::Url("a:".into()).extension(), "url");
        assert_eq!(ShortcutTarget::Path(PathBuf::new()).extension(), "lnk");
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
