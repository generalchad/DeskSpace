//! The Space fonts and Window fonts pages, Global only: a card per text role
//! with its family, size, and weight, and a reset per page. An edit writes
//! `Config::typography`, which the spaces preview at once and Apply saves.

use deskspace_config::Config;
use deskspace_core::typography::{Font, Role};
use deskspace_platform::{Field, FieldValue, SettingsPage};

use super::FieldEffect;
use super::ids::field;
use crate::font_choices::Part;

/// One font page.
struct FontPage {
    title: &'static str,
    note_id: u32,
    note: &'static str,
    roles: &'static [Role],
    reset: u32,
}

/// The font pages in build order; every role is on exactly one.
const FONT_PAGES: [FontPage; 2] = [
    FontPage {
        title: "Space fonts",
        note_id: field::SPACE_FONTS_NOTE,
        note: "The text inside every space. The spaces show a change at once.",
        roles: &[
            Role::SpaceTitle,
            Role::ItemLabel,
            Role::Details,
            Role::Placeholder,
            Role::Zoom,
        ],
        reset: field::RESET_SPACE_FONTS,
    },
    FontPage {
        title: "Window fonts",
        note_id: field::WINDOW_FONTS_NOTE,
        note: "The text of DeskSpace's own windows and dialogs. The settings \
               window shows a change the next time it opens.",
        roles: &[
            Role::Body,
            Role::Heading1,
            Role::Heading2,
            Role::Heading3,
            Role::Caption,
        ],
        reset: field::RESET_WINDOW_FONTS,
    },
];

/// The field id of `role`'s heading (`None`) or one of its choices.
fn font_field(role: Role, part: Option<Part>) -> u32 {
    let slot = part.map_or(0, |part| {
        1 + Part::ALL
            .iter()
            .position(|each| *each == part)
            .expect("every part is in ALL") as u32
    });
    field::FONTS + role as u32 * field::FONT_SLOTS + slot
}

/// The role and choice a font field id names.
fn font_choice(id: u32) -> Option<(Role, Part)> {
    Role::ALL
        .into_iter()
        .flat_map(|role| Part::ALL.map(|part| (role, part)))
        .find(|(role, part)| font_field(*role, Some(*part)) == id)
}

/// The font pages, showing `config`'s fonts. `installed` is every installed
/// family, sorted.
pub(super) fn font_pages(config: &Config, installed: &[String]) -> Vec<SettingsPage> {
    FONT_PAGES
        .iter()
        .map(|page| font_page(page, config, installed))
        .collect()
}

fn font_page(page: &FontPage, config: &Config, installed: &[String]) -> SettingsPage {
    let mut fields = vec![Field::note(page.note_id, page.note)];
    for role in page.roles {
        let font = config.typography.font(*role);
        fields.push(Field::heading(font_field(*role, None), role.label()));
        for part in Part::ALL {
            let (options, selected) = part.options(font, installed);
            let options: Vec<&str> = options.iter().map(String::as_str).collect();
            fields.push(Field::choice(
                font_field(*role, Some(part)),
                part.label(),
                &options,
                selected,
            ));
            if part == Part::Family
                && let Some(note) = role.note()
            {
                fields.push(Field::note(field::FONT_NOTES + *role as u32, note));
            }
        }
    }
    fields.push(Field::button(page.reset, "", "Reset to defaults"));
    SettingsPage {
        title: page.title.into(),
        fields,
    }
}

/// Applies a font choice's edit to `config`; `None` when `id` is not one.
pub(super) fn apply_font_field(
    config: &mut Config,
    id: u32,
    value: &FieldValue,
    installed: &[String],
) -> Option<FieldEffect> {
    let (role, part) = font_choice(id)?;
    let FieldValue::Choice(index) = value else {
        return None;
    };
    let font = part.pick(config.typography.font(role), *index, installed);
    config.typography.set(role, font);
    Some(FieldEffect::Fonts)
}

/// Whether `id` is a font page's reset button.
pub(super) fn is_font_reset(id: u32) -> bool {
    FONT_PAGES.iter().any(|page| page.reset == id)
}

/// A font page's "Reset to defaults", the button `reset`: puts that page's
/// roles back to their defaults, previewed like any edit.
pub fn reset_fonts(config: &mut Config, reset: u32) {
    if let Some(page) = FONT_PAGES.iter().find(|page| page.reset == reset) {
        for role in page.roles {
            config.typography.set(*role, Font::default_for(*role));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed() -> Vec<String> {
        ["Arial", "Consolas"].map(String::from).to_vec()
    }

    #[test]
    fn every_role_is_on_exactly_one_page() {
        for role in Role::ALL {
            let pages = FONT_PAGES
                .iter()
                .filter(|page| page.roles.contains(&role))
                .count();
            assert_eq!(pages, 1, "{role}");
        }
    }

    #[test]
    fn the_font_ids_stay_inside_their_block() {
        let last = field::FONTS + Role::ALL.len() as u32 * field::FONT_SLOTS - 1;
        assert!(last < field::FONT_NOTES, "the choices end before the notes");
        let last_note = field::FONT_NOTES + Role::ALL.len() as u32 - 1;
        assert!(last_note < field::SPACE_FONTS_NOTE);
        for role in Role::ALL {
            for part in Part::ALL {
                assert_eq!(
                    font_choice(font_field(role, Some(part))),
                    Some((role, part))
                );
            }
            assert_eq!(font_choice(font_field(role, None)), None, "a heading");
        }
    }

    #[test]
    fn a_pick_writes_the_config_and_previews_the_fonts() {
        let mut config = Config::new();
        let id = font_field(Role::SpaceTitle, Some(Part::Family));
        let effect = apply_font_field(&mut config, id, &FieldValue::Choice(2), &installed());
        assert_eq!(effect, Some(FieldEffect::Fonts));
        assert_eq!(config.typography.font(Role::SpaceTitle).family, "Consolas");
        let id = font_field(Role::SpaceTitle, Some(Part::Weight));
        apply_font_field(&mut config, id, &FieldValue::Choice(2), &installed());
        assert_eq!(config.typography.font(Role::SpaceTitle).weight, 700);
    }

    #[test]
    fn a_reset_puts_back_only_its_pages_roles() {
        let mut config = Config::new();
        config
            .typography
            .set(Role::SpaceTitle, Font::new("Arial", 20.0, 700));
        config
            .typography
            .set(Role::Body, Font::new("Arial", 14.0, 400));
        reset_fonts(&mut config, field::RESET_SPACE_FONTS);
        assert_eq!(
            config.typography.font(Role::SpaceTitle),
            &Font::default_for(Role::SpaceTitle)
        );
        assert_eq!(config.typography.font(Role::Body).family, "Arial");
        assert!(is_font_reset(field::RESET_WINDOW_FONTS));
    }

    #[test]
    fn a_page_shows_each_role_as_a_card_of_three_choices() {
        let pages = font_pages(&Config::new(), &installed());
        assert_eq!(pages.len(), FONT_PAGES.len());
        let window = &pages[1];
        let headings = window
            .fields
            .iter()
            .filter(|field| field.label == "Heading 1")
            .count();
        assert_eq!(headings, 1);
        let family = window
            .fields
            .iter()
            .find(|field| field.id == font_field(Role::Body, Some(Part::Family)))
            .expect("Body's family");
        assert!(matches!(
            &family.kind,
            deskspace_platform::FieldKind::Choice { options, selected: 0 }
                if options[0] == "Windows default" && options.len() == 3
        ));
    }
}
