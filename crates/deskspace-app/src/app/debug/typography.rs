//! The debug window's typography box (F-68): a family, size, and weight per
//! text role, then Apply fonts, Reset to saved fonts, and the language choice
//! with two sample lines proving the catalog resolves.
//!
//! Picks are a draft until Apply fonts, which makes them this session's
//! override of the saved scale (`DebugTools::typography`). Nothing here is
//! saved (ROADMAP D14); the settings window's font pages save fonts.

use deskspace_core::text::{self, Key, Language};
use deskspace_core::typography::{Role, Typography};
use deskspace_platform::{DebugControl, DebugControlKind, Ui};

use super::DebugInput;
use crate::app::App;
use crate::font_choices::Part;

/// The box's title.
const GROUP: &str = "Typography";
/// The ids the box's controls start at, above the shadow box.
const BASE: u32 = 10_200;
const RESET_ID: u32 = BASE + 100;
const LANGUAGE_ID: u32 = BASE + 101;
const FONT_SAMPLE_ID: u32 = BASE + 102;
const LANGUAGE_SAMPLE_ID: u32 = BASE + 103;
const APPLY_ID: u32 = BASE + 104;
const STATUS_ID: u32 = BASE + 105;
/// A role's note is `NOTE_BASE + role`.
const NOTE_BASE: u32 = BASE + 200;

/// A role's choice `part`'s control id.
fn part_id(role: Role, part: Part) -> u32 {
    let part = Part::ALL
        .iter()
        .position(|each| *each == part)
        .expect("every part is in ALL");
    BASE + (role as usize * Part::ALL.len() + part) as u32
}

fn from_part_id(id: u32) -> Option<(Role, Part)> {
    Role::ALL
        .into_iter()
        .flat_map(|role| Part::ALL.map(|part| (role, part)))
        .find(|(role, part)| part_id(*role, *part) == id)
}

/// Which scale the box shows and what Apply would change, as its status line.
fn status(saved: &Typography, in_force: &Typography, draft: Option<&Typography>) -> String {
    let source = if in_force == saved {
        "In force: the saved fonts."
    } else {
        "In force: this session's fonts (not saved)."
    };
    match draft {
        Some(draft) if draft != in_force => {
            format!("{source}\nUnapplied changes: press Apply fonts.")
        }
        _ => source.to_owned(),
    }
}

/// The box's controls, in role order: each role's family, size, and weight,
/// then Apply, Reset, the status line, the Language choice, and the two sample
/// lines.
pub(super) fn controls(app: &App) -> Vec<DebugControl> {
    let tools = app.debug.as_ref();
    let in_force = app.typography();
    let draft = tools.and_then(|tools| tools.typography_draft.as_ref());
    let shown = draft.unwrap_or(in_force);
    let installed = app.installed_fonts();
    let mut controls = Vec::new();
    for (role, font) in shown.iter() {
        for part in Part::ALL {
            let (options, selected) = part.options(font, installed);
            controls.push(control(
                part_id(role, part),
                &format!("{}: {}", role.label(), part.label()),
                DebugControlKind::Choice {
                    options,
                    selected,
                    enabled: true,
                },
            ));
        }
        if let Some(note) = role.note() {
            controls.push(control(
                NOTE_BASE + role as u32,
                &format!("{}: Used for", role.label()),
                DebugControlKind::Readout {
                    text: note.to_owned(),
                },
            ));
        }
    }
    let unapplied = draft.is_some_and(|draft| draft != in_force);
    controls.push(button(APPLY_ID, "Apply fonts", unapplied));
    let overridden = in_force != &app.config.typography;
    controls.push(button(
        RESET_ID,
        "Reset to saved fonts",
        overridden || unapplied,
    ));
    controls.push(control(
        STATUS_ID,
        "Status",
        DebugControlKind::Readout {
            text: status(&app.config.typography, in_force, draft),
        },
    ));
    let language = tools.map_or(Language::English, |tools| tools.language);
    controls.push(control(
        LANGUAGE_ID,
        "Language",
        DebugControlKind::Choice {
            options: Language::ALL
                .iter()
                .map(|language| language.label().to_owned())
                .collect(),
            selected: Language::ALL
                .iter()
                .position(|each| *each == language)
                .unwrap_or(0),
            enabled: true,
        },
    ));
    controls.push(control(
        FONT_SAMPLE_ID,
        "Font sample",
        DebugControlKind::Readout {
            text: text::lookup(language, Key::FontSample),
        },
    ));
    controls.push(control(
        LANGUAGE_SAMPLE_ID,
        "Language sample",
        DebugControlKind::Readout {
            text: text::lookup(language, Key::LanguageSample),
        },
    ));
    controls
}

fn control(id: u32, label: &str, kind: DebugControlKind) -> DebugControl {
    DebugControl {
        id,
        label: label.to_owned(),
        kind,
        group: Some(GROUP.to_owned()),
    }
}

fn button(id: u32, label: &str, enabled: bool) -> DebugControl {
    control(id, label, DebugControlKind::Button { enabled })
}

impl App {
    /// A typography-box control: a pick goes into the draft, Apply makes the
    /// draft this session's fonts, and Reset goes back to the saved ones.
    pub(super) fn on_typography_setting(&mut self, ui: &mut Ui, id: u32, input: DebugInput) {
        match (id, input) {
            (APPLY_ID, _) => self.apply_typography_draft(ui),
            (RESET_ID, _) => {
                if let Some(tools) = self.debug.as_mut() {
                    tools.typography = None;
                    tools.typography_draft = None;
                }
                self.sync_typography(ui);
            }
            (LANGUAGE_ID, DebugInput::Choose(index)) => {
                if let Some(tools) = self.debug.as_mut() {
                    tools.language = *Language::ALL.get(index).unwrap_or(&Language::English);
                }
            }
            (id, DebugInput::Choose(index)) => {
                if let Some((role, part)) = from_part_id(id) {
                    self.pick_typography(role, part, index);
                }
            }
            _ => {}
        }
    }

    /// Records a pick in the draft, starting from what the box shows.
    fn pick_typography(&mut self, role: Role, part: Part, index: usize) {
        let mut draft = self
            .debug
            .as_ref()
            .and_then(|tools| tools.typography_draft.clone())
            .unwrap_or_else(|| self.typography().clone());
        let font = part.pick(draft.font(role), index, self.installed_fonts());
        draft.set(role, font);
        if let Some(tools) = self.debug.as_mut() {
            tools.typography_draft = Some(draft);
        }
    }

    /// Apply fonts: the draft becomes this session's fonts, or none when it
    /// matches the saved ones, so a later settings edit is followed again.
    fn apply_typography_draft(&mut self, ui: &mut Ui) {
        let saved = self.config.typography.clone();
        let Some(tools) = self.debug.as_mut() else {
            return;
        };
        let Some(draft) = tools.typography_draft.take() else {
            return;
        };
        tools.typography = (draft != saved).then_some(draft);
        self.sync_typography(ui);
        tracing::info!("debug: applied the typography box's fonts for this session");
    }
}

#[cfg(test)]
mod tests {
    use deskspace_core::typography::Font;

    use super::*;

    #[test]
    fn every_part_of_every_role_has_its_own_id() {
        let mut ids: Vec<u32> = Role::ALL
            .into_iter()
            .flat_map(|role| Part::ALL.map(|part| part_id(role, part)))
            .collect();
        for (role, part) in Role::ALL
            .into_iter()
            .flat_map(|role| Part::ALL.map(|part| (role, part)))
        {
            assert_eq!(from_part_id(part_id(role, part)), Some((role, part)));
        }
        let count = ids.len();
        ids.extend([
            RESET_ID,
            LANGUAGE_ID,
            FONT_SAMPLE_ID,
            LANGUAGE_SAMPLE_ID,
            APPLY_ID,
            STATUS_ID,
        ]);
        ids.extend(Role::ALL.map(|role| NOTE_BASE + role as u32));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count + 6 + Role::ALL.len());
    }

    #[test]
    fn the_status_says_what_is_in_force_and_what_waits() {
        let saved = Typography::new();
        let mut session = Typography::new();
        session.set(Role::Body, Font::new("Consolas", 12.0, 400));
        assert_eq!(status(&saved, &saved, None), "In force: the saved fonts.");
        assert_eq!(
            status(&saved, &saved, Some(&saved)),
            "In force: the saved fonts.",
            "a draft that changes nothing waits for nothing"
        );
        let waiting = status(&saved, &saved, Some(&session));
        assert!(waiting.ends_with("press Apply fonts."), "{waiting}");
        assert!(status(&saved, &session, None).contains("not saved"));
    }
}
