//! The debug window's typography box (F-68): one group per text role with a
//! family, size, and weight, a reset, and the language choice with two sample
//! lines proving the catalog resolves. Session only (ROADMAP D14).

use deskspace_core::text::{self, Key, Language};
use deskspace_core::typography::{FONT_WEIGHTS, Font, Role};
use deskspace_platform::{DebugControl, DebugControlKind, Ui};

use super::DebugInput;
use crate::app::App;

/// The box's title.
const GROUP: &str = "Typography";
/// The ids the box's controls start at, above the shadow box.
const BASE: u32 = 10_200;
/// The sizes the Debug window offers, within `FONT_SIZE_RANGE`.
const SIZE_PRESETS: &[f32] = &[
    8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 40.0, 48.0,
];

/// One part of a role's row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Family,
    Size,
    Weight,
}

impl Part {
    const ALL: [Self; 3] = [Self::Family, Self::Size, Self::Weight];

    fn label(self) -> &'static str {
        match self {
            Self::Family => "Family",
            Self::Size => "Size",
            Self::Weight => "Weight",
        }
    }

    fn id(self, role: Role) -> u32 {
        let role = Role::ALL
            .iter()
            .position(|each| *each == role)
            .expect("every role is in ALL");
        let part = Self::ALL
            .iter()
            .position(|each| *each == self)
            .expect("every part is in ALL");
        BASE + (role * Self::ALL.len() + part) as u32
    }

    fn from_id(id: u32) -> Option<(Role, Self)> {
        for role in Role::ALL {
            for part in Self::ALL {
                if part.id(role) == id {
                    return Some((role, part));
                }
            }
        }
        None
    }
}

const RESET_ID: u32 = BASE + 100;
const LANGUAGE_ID: u32 = BASE + 101;
const FONT_SAMPLE_ID: u32 = BASE + 102;
const LANGUAGE_SAMPLE_ID: u32 = BASE + 103;

/// Every family the box offers, Windows' default first.
fn families(app: &App) -> Vec<String> {
    let mut names = vec![String::from("Windows default")];
    if let Some(list) = app.debug.as_ref().and_then(|tools| tools.fonts.as_ref()) {
        names.extend(list.iter().cloned());
    }
    names
}

/// The sizes the box offers, with `current` in place when it is between them.
fn sizes(current: f32) -> Vec<f32> {
    let mut values = SIZE_PRESETS.to_vec();
    if !values.iter().any(|size| (size - current).abs() < 0.01) {
        values.push(current);
        values.sort_by(|a, b| a.total_cmp(b));
    }
    values
}

/// The box's controls, in role order: each role's family, size, and weight,
/// then Reset, the Language choice, and the two sample lines.
pub(super) fn controls(app: &App, ui: &Ui) -> Vec<DebugControl> {
    let typography = app.typography();
    let names = families(app);
    let language = app
        .debug
        .as_ref()
        .map_or(Language::English, |tools| tools.language);
    let mut controls = Vec::new();
    for role in Role::ALL {
        let font = typography.font(role);
        // The family row.
        let selected = names
            .iter()
            .position(|name| {
                if font.family.is_empty() {
                    name == "Windows default"
                } else {
                    name == &font.family
                }
            })
            .unwrap_or(0);
        controls.push(control(
            Part::Family.id(role),
            &format!("{}: {}", role.label(), Part::Family.label()),
        ));
        let family_options: Vec<String> = names.clone();
        controls.last_mut().expect("just pushed").kind = DebugControlKind::Choice {
            options: family_options,
            selected,
            enabled: true,
        };

        let values = sizes(font.size);
        let current = values
            .iter()
            .position(|size| (size - font.size).abs() < 0.01)
            .unwrap_or(0);
        let mut size_control = control(
            Part::Size.id(role),
            &format!("{}: {}", role.label(), Part::Size.label()),
        );
        size_control.kind = DebugControlKind::Choice {
            options: values.iter().map(|size| format!("{size} px")).collect(),
            selected: current,
            enabled: true,
        };
        controls.push(size_control);

        let weights: Vec<u16> = FONT_WEIGHTS.iter().map(|(value, _)| *value).collect();
        let weight_names: Vec<String> = FONT_WEIGHTS
            .iter()
            .map(|(_, name)| (*name).to_owned())
            .collect();
        let current = FONT_WEIGHTS
            .iter()
            .position(|(value, _)| *value == font.weight)
            .unwrap_or(0);
        let mut weight_control = control(
            Part::Weight.id(role),
            &format!("{}: {}", role.label(), Part::Weight.label()),
        );
        weight_control.kind = DebugControlKind::Choice {
            options: weight_names,
            selected: current,
            enabled: true,
        };
        let _ = weights;
        controls.push(weight_control);
        if let Some(note) = role.note() {
            controls.push(control(BASE + 200 + role as u32, note));
        }
    }
    controls.push(button(RESET_ID, "Reset fonts"));
    let language_names: Vec<String> = Language::ALL
        .iter()
        .map(|language| language.label().to_owned())
        .collect();
    let mut language_control = control(LANGUAGE_ID, "Language");
    language_control.kind = DebugControlKind::Choice {
        options: language_names,
        selected: Language::ALL
            .iter()
            .position(|each| *each == language)
            .unwrap_or(0),
        enabled: true,
    };
    controls.push(language_control);
    let mut font_sample = control(FONT_SAMPLE_ID, "Font sample");
    font_sample.kind = DebugControlKind::Readout {
        text: text::lookup(language, Key::FontSample),
    };
    controls.push(font_sample);
    let mut language_sample = control(LANGUAGE_SAMPLE_ID, "Language sample");
    language_sample.kind = DebugControlKind::Readout {
        text: text::lookup(language, Key::LanguageSample),
    };
    controls.push(language_sample);
    let _ = ui;
    controls
}

fn control(id: u32, label: &str) -> DebugControl {
    DebugControl {
        id,
        label: label.to_owned(),
        kind: DebugControlKind::Button { enabled: true },
        group: Some(GROUP.to_owned()),
    }
}

fn button(id: u32, label: &str) -> DebugControl {
    let mut control = control(id, label);
    control.kind = DebugControlKind::Button { enabled: true };
    control
}

impl App {
    /// Applies a typography-box control and asks for the Debug window to be
    /// rebuilt on the next refresh tick, so its own text follows the fonts.
    pub(super) fn on_typography_setting(&mut self, ui: &mut Ui, id: u32, input: DebugInput) {
        if id == RESET_ID {
            if let Some(tools) = self.debug.as_mut() {
                tools.typography = None;
                tools.rebuild_debug = true;
            }
            self.sync_typography(ui);
            return;
        }
        if id == LANGUAGE_ID {
            if let (DebugInput::Choose(index), Some(tools)) = (input, self.debug.as_mut()) {
                tools.language = *Language::ALL.get(index).unwrap_or(&Language::English);
            }
            return;
        }
        let Some((role, part)) = Part::from_id(id) else {
            return;
        };
        let DebugInput::Choose(index) = input else {
            return;
        };
        let names = families(self);
        let mut typography = self.typography().clone();
        let Some(tools) = self.debug.as_mut() else {
            return;
        };
        let font = typography.font(role).clone();
        let updated = match part {
            Part::Family => {
                let family = names
                    .get(index)
                    .map(|name| {
                        if name == "Windows default" {
                            String::new()
                        } else {
                            name.clone()
                        }
                    })
                    .unwrap_or_default();
                Font { family, ..font }
            }
            Part::Size => {
                let values = sizes(font.size);
                Font {
                    size: values.get(index).copied().unwrap_or(font.size),
                    ..font
                }
            }
            Part::Weight => Font {
                weight: FONT_WEIGHTS
                    .get(index)
                    .map_or(font.weight, |(value, _)| *value),
                ..font
            },
        };
        typography.set(role, updated);
        tools.typography = Some(typography);
        tools.rebuild_debug = true;
        self.sync_typography(ui);
    }
}
