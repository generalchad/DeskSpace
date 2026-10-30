//! `App` zoom: Ctrl+wheel, the zoom pop-up's buttons, and the zoom keys
//! change the icon size (ROADMAP D9), and the pop-up shows the result
//! (ROADMAP F-57). On a space set to one of Explorer's views they step its
//! view instead, with no pop-up (W39).

use deskspace_core::theme::{ICON_SIZE_DEFAULT, ICON_SIZE_RANGE, Palette, Theme};
use deskspace_core::{Backdrop, SpaceId, SpaceView, ViewMode};
use deskspace_platform::{
    DESKTOP_ICON_RANGE, ITEM_ICON_SIZE, Ui, ZoomAction, ZoomBar, next_desktop_icon_size,
};

use super::effect::Effect;
use super::{App, ICON_SIZE_WHEEL_STEP, Surface, Timer, WHEEL_DELTA};
use crate::scene::space_style;
use crate::settings::step_view;

/// How long the zoom pop-up stays after the last zoom, unless the pointer is
/// over it.
const ZOOM_BAR_MS: u32 = 2_000;

/// The least tint opacity the pop-up is drawn with, so its text stays readable
/// over any wallpaper even when the spaces are nearly see-through.
const ZOOM_BAR_MIN_TINT_ALPHA: u8 = 160;

/// The icon size that zooming changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ZoomLevel {
    /// Under "Match Explorer's display": the desktop's own icon size, in
    /// pixels, which the matching spaces follow.
    Desktop(i32),
    /// Otherwise: DeskSpace's global icon size, in percent.
    Custom(u8),
}

impl ZoomLevel {
    /// The size as a percentage of the default. The desktop's 48 px ("Medium
    /// icons") is 100 %, as DeskSpace's own icon size is.
    fn percent(self) -> u32 {
        match self {
            Self::Desktop(pixels) => {
                ((pixels.max(0) * 100 + ITEM_ICON_SIZE / 2) / ITEM_ICON_SIZE) as u32
            }
            Self::Custom(percent) => percent.into(),
        }
    }

    fn can_zoom_out(self) -> bool {
        match self {
            Self::Desktop(pixels) => pixels > DESKTOP_ICON_RANGE.0,
            Self::Custom(percent) => percent > ICON_SIZE_RANGE.0,
        }
    }

    fn can_zoom_in(self) -> bool {
        match self {
            Self::Desktop(pixels) => pixels < DESKTOP_ICON_RANGE.1,
            Self::Custom(percent) => percent < ICON_SIZE_RANGE.1,
        }
    }

    fn is_default(self) -> bool {
        match self {
            Self::Desktop(pixels) => pixels == ITEM_ICON_SIZE,
            Self::Custom(percent) => percent == ICON_SIZE_DEFAULT,
        }
    }
}

/// The pop-up for `level`, in the global look. A clear backdrop would leave
/// nothing behind the text, so the pop-up uses acrylic instead.
fn zoom_bar(level: ZoomLevel, theme: &Theme, palette: &Palette, system_dark: bool) -> ZoomBar {
    let mut look = theme.clone();
    if look.backdrop == Backdrop::Clear {
        look.backdrop = Backdrop::Acrylic;
    }
    ZoomBar {
        percent: level.percent(),
        can_zoom_out: level.can_zoom_out(),
        can_zoom_in: level.can_zoom_in(),
        can_reset: !level.is_default(),
        style: space_style(&look, system_dark),
        tint: palette
            .tint
            .with_alpha(palette.tint.a.max(ZOOM_BAR_MIN_TINT_ALPHA)),
        text: palette.title_text,
        hover: palette.hover_fill,
        border: palette.border,
    }
}

impl App {
    pub(super) fn zoom_level(&self) -> ZoomLevel {
        match self.system.shell().icon_size {
            Some(pixels) if self.config.theme.match_explorer() => ZoomLevel::Desktop(pixels),
            _ => ZoomLevel::Custom(
                self.config
                    .theme
                    .icon_size
                    .clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1),
            ),
        }
    }

    /// A zoom pop-up button or zoom key: one notch either way, or back to
    /// 100 %.
    pub(super) fn zoom(&mut self, ui: &mut Ui, action: ZoomAction) {
        let effects = self.zoom_effects(action);
        self.run(ui, effects);
    }

    /// Ctrl+wheel: `delta` is the wheel's, in `WHEEL_DELTA` units. A space
    /// set to one of Explorer's views steps through them instead (W39).
    pub(super) fn adjust_icon_size(&mut self, ui: &mut Ui, surface: Surface, delta: i32) {
        // Zooming in walks toward the bigger views, against the menu's
        // order, as Explorer's Ctrl+wheel does.
        if self.step_view_of(ui, self.view_step(surface, -(delta / WHEEL_DELTA))) {
            return;
        }
        let effects = self.wheel_zoom_effects(delta);
        self.run(ui, effects);
    }

    /// Ctrl+Plus, Ctrl+Minus, and Ctrl+0: on a space set to one of
    /// Explorer's views they step it, and Ctrl+0 returns it to "Settings
    /// icon size" (W39); anything else zooms as the pop-up's buttons do.
    pub(super) fn zoom_key(&mut self, ui: &mut Ui, action: ZoomAction) {
        if self.step_view_of(ui, self.zoom_step(self.selection.surface, action)) {
            return;
        }
        self.zoom(ui, action);
    }

    /// The space on `surface` and the view `notches` wheel steps give it
    /// (W39), when it is set to one of Explorer's views; `None` for
    /// "Settings icon size", the desktop layer, or a gone space, which zoom
    /// as D9 says. Pure, so tests check it without a `Ui`.
    fn view_step(&self, surface: Surface, notches: i32) -> Option<(SpaceId, SpaceView)> {
        let (id, view) = self.fixed_view(surface)?;
        Some((id, view.with_mode(step_view(view.mode, notches))))
    }

    /// The view a zoom key steps the current space to (W39): Ctrl+Plus and
    /// Ctrl+Minus walk Explorer's views, zooming in toward the bigger ones,
    /// and Ctrl+0 returns to "Settings icon size". No current space, or one
    /// on "Settings icon size", zooms as D9 says.
    fn zoom_step(
        &self,
        surface: Option<Surface>,
        action: ZoomAction,
    ) -> Option<(SpaceId, SpaceView)> {
        let (id, view) = self.fixed_view(surface?)?;
        let mode = match action {
            ZoomAction::In => step_view(view.mode, -1),
            ZoomAction::Out => step_view(view.mode, 1),
            ZoomAction::Reset => ViewMode::Icons,
        };
        Some((id, view.with_mode(mode)))
    }

    /// The space on `surface` when it is set to one of Explorer's views
    /// (W39), with its view; `None` otherwise.
    fn fixed_view(&self, surface: Surface) -> Option<(SpaceId, SpaceView)> {
        let Surface::Space(id) = surface else {
            return None;
        };
        let view = self.config.desktop.space(id)?.view();
        (view.mode != ViewMode::Icons).then_some((id, view))
    }

    /// Applies a view step `view_step` or `zoom_step` decided (W39): one
    /// "Change view" undo entry and no zoom pop-up. Returns whether the
    /// space was set to one of Explorer's views, so a step already at an
    /// end — or a partial wheel notch — does nothing rather than zooming.
    fn step_view_of(&mut self, ui: &mut Ui, step: Option<(SpaceId, SpaceView)>) -> bool {
        let Some((id, view)) = step else {
            return false;
        };
        if self
            .config
            .desktop
            .space(id)
            .is_some_and(|space| space.view() != view)
        {
            let at = self.space_center(id);
            self.set_space_view(ui, id, view, at);
        }
        true
    }

    /// Hides the pop-up once the pointer is off it. The timer repeats, so a
    /// hovered pop-up is checked again until the pointer leaves.
    pub(super) fn on_zoom_bar_timer(&mut self, ui: &mut Ui) {
        let hovered = ui.is_zoom_bar_hovered();
        self.run(ui, zoom_bar_timer_effects(hovered));
    }

    fn zoom_effects(&mut self, action: ZoomAction) -> Vec<Effect> {
        match action {
            ZoomAction::Out => self.wheel_zoom_effects(-WHEEL_DELTA),
            ZoomAction::In => self.wheel_zoom_effects(WHEEL_DELTA),
            ZoomAction::Reset => self.reset_zoom_effects(),
        }
    }

    /// Ctrl+wheel changes the icon size, as it does on the desktop, and shows
    /// the zoom pop-up. A space with its own `Custom` look keeps its size
    /// either way.
    ///
    /// Under "Match Explorer's display" (ROADMAP D9) it changes the desktop's
    /// own icon size, which the spaces that match Explorer then follow, so
    /// they stay 1:1 with the desktop. Otherwise, or when the desktop's size
    /// cannot be read, it changes DeskSpace's global icon size, persisted
    /// through the settings rather than the config's base grid.
    ///
    /// A fixed icon view (D17) reads the look's size nowhere: its layout uses
    /// `ViewMode::icon_px`, so the re-flow a zoom asks for leaves it alone.
    fn wheel_zoom_effects(&mut self, delta: i32) -> Vec<Effect> {
        let mut effects = match self.zoom_level() {
            ZoomLevel::Desktop(current) => desktop_zoom_effects(current, delta),
            ZoomLevel::Custom(current) => {
                let step = delta * ICON_SIZE_WHEEL_STEP / WHEEL_DELTA;
                let next = (i32::from(current) + step)
                    .clamp(ICON_SIZE_RANGE.0.into(), ICON_SIZE_RANGE.1.into());
                self.set_global_icon_size(next as u8)
            }
        };
        effects.extend(show_zoom_bar_effects());
        effects
    }

    fn reset_zoom_effects(&mut self) -> Vec<Effect> {
        let mut effects = match self.zoom_level() {
            ZoomLevel::Desktop(_) => vec![
                Effect::SetDesktopIconSize(ITEM_ICON_SIZE),
                Effect::RefreshShellDisplay,
            ],
            ZoomLevel::Custom(_) => self.set_global_icon_size(ICON_SIZE_DEFAULT),
        };
        effects.extend(show_zoom_bar_effects());
        effects
    }

    fn set_global_icon_size(&mut self, size: u8) -> Vec<Effect> {
        if size == self.config.theme.icon_size {
            return Vec::new();
        }
        self.config.theme.icon_size = size;
        vec![
            Effect::RelayoutAll,
            Effect::ScheduleSave,
            Effect::RefreshSettings,
        ]
    }

    /// Shows the pop-up with the current size.
    pub(super) fn show_zoom_bar(&self, ui: &mut Ui) {
        let palette = self.surface_palette(Surface::Desktop);
        ui.show_zoom_bar(zoom_bar(
            self.zoom_level(),
            &self.config.theme,
            &palette,
            self.system.dark(),
        ));
    }
}

/// Ctrl+wheel under "Match Explorer's display": the desktop's own size moves
/// by whole notches, so a partial one (a precision wheel) changes nothing.
fn desktop_zoom_effects(current: i32, delta: i32) -> Vec<Effect> {
    let notches = delta / WHEEL_DELTA;
    if notches == 0 {
        return Vec::new();
    }
    vec![
        Effect::SetDesktopIconSize(next_desktop_icon_size(current, notches)),
        Effect::RefreshShellDisplay,
    ]
}

/// Shows the pop-up and (re)starts its hide timer.
fn show_zoom_bar_effects() -> [Effect; 2] {
    [
        Effect::ShowZoomBar,
        Effect::SetTimer(Timer::ZoomBar, ZOOM_BAR_MS),
    ]
}

fn zoom_bar_timer_effects(hovered: bool) -> Vec<Effect> {
    if hovered {
        return Vec::new();
    }
    vec![Effect::HideZoomBar, Effect::ClearTimer(Timer::ZoomBar)]
}

#[cfg(test)]
mod tests {
    use super::*;

    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::theme::Preset;
    use deskspace_platform::ShellDisplay;

    /// An app whose zoom changes `preset`'s size: DeskSpace's own
    /// `icon_size`, or under Match Explorer the desktop's `desktop_size`
    /// (`None` when it cannot be read).
    fn zoom_app(preset: Preset, icon_size: u8, desktop_size: Option<i32>) -> App {
        let mut config = Config::new();
        config.theme.preset = preset;
        config.theme.icon_size = icon_size;
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(config, store, false, false);
        app.system.set_shell(ShellDisplay {
            icon_size: desktop_size,
            ..ShellDisplay::default()
        });
        app
    }

    fn then_show_bar(mut effects: Vec<Effect>) -> Vec<Effect> {
        effects.extend(show_zoom_bar_effects());
        effects
    }

    const SIZE_CHANGED: [Effect; 3] = [
        Effect::RelayoutAll,
        Effect::ScheduleSave,
        Effect::RefreshSettings,
    ];

    #[test]
    fn a_custom_zoom_changes_the_global_size_and_shows_the_bar() {
        let mut app = zoom_app(Preset::Custom, ICON_SIZE_DEFAULT, Some(ITEM_ICON_SIZE));
        let effects = app.zoom_effects(ZoomAction::In);
        assert_eq!(effects, then_show_bar(SIZE_CHANGED.to_vec()));
        let step = ICON_SIZE_WHEEL_STEP as u8;
        assert_eq!(app.config.theme.icon_size, ICON_SIZE_DEFAULT + step);

        let effects = app.zoom_effects(ZoomAction::Out);
        assert_eq!(effects, then_show_bar(SIZE_CHANGED.to_vec()));
        assert_eq!(app.config.theme.icon_size, ICON_SIZE_DEFAULT);
    }

    #[test]
    fn a_custom_zoom_at_either_end_only_shows_the_bar() {
        let (small, large) = ICON_SIZE_RANGE;
        let mut app = zoom_app(Preset::Custom, large, None);
        assert_eq!(app.zoom_effects(ZoomAction::In), then_show_bar(vec![]));
        assert_eq!(app.config.theme.icon_size, large);

        let mut app = zoom_app(Preset::Custom, small, None);
        assert_eq!(app.zoom_effects(ZoomAction::Out), then_show_bar(vec![]));
        assert_eq!(app.config.theme.icon_size, small);
    }

    #[test]
    fn a_custom_reset_changes_only_a_size_away_from_the_default() {
        let mut app = zoom_app(Preset::Custom, ICON_SIZE_DEFAULT, None);
        assert_eq!(app.zoom_effects(ZoomAction::Reset), then_show_bar(vec![]));

        let mut app = zoom_app(Preset::Custom, ICON_SIZE_RANGE.1, None);
        let effects = app.zoom_effects(ZoomAction::Reset);
        assert_eq!(effects, then_show_bar(SIZE_CHANGED.to_vec()));
        assert_eq!(app.config.theme.icon_size, ICON_SIZE_DEFAULT);
    }

    #[test]
    fn a_match_explorer_zoom_asks_explorer_and_leaves_the_global_size() {
        let mut app = zoom_app(
            Preset::MatchExplorer,
            ICON_SIZE_DEFAULT,
            Some(ITEM_ICON_SIZE),
        );
        let effects = app.zoom_effects(ZoomAction::In);
        let next = next_desktop_icon_size(ITEM_ICON_SIZE, 1);
        assert!(next > ITEM_ICON_SIZE);
        assert_eq!(
            effects,
            then_show_bar(vec![
                Effect::SetDesktopIconSize(next),
                Effect::RefreshShellDisplay,
            ])
        );
        assert_eq!(app.config.theme.icon_size, ICON_SIZE_DEFAULT);
    }

    #[test]
    fn a_match_explorer_zoom_at_either_end_stays_in_range() {
        let (small, large) = DESKTOP_ICON_RANGE;
        for (size, action) in [(large, ZoomAction::In), (small, ZoomAction::Out)] {
            let mut app = zoom_app(Preset::MatchExplorer, ICON_SIZE_DEFAULT, Some(size));
            assert_eq!(
                app.zoom_effects(action),
                then_show_bar(vec![
                    Effect::SetDesktopIconSize(size),
                    Effect::RefreshShellDisplay,
                ])
            );
        }
    }

    #[test]
    fn a_match_explorer_reset_asks_for_medium_icons() {
        let mut app = zoom_app(Preset::MatchExplorer, ICON_SIZE_DEFAULT, Some(96));
        assert_eq!(
            app.zoom_effects(ZoomAction::Reset),
            then_show_bar(vec![
                Effect::SetDesktopIconSize(ITEM_ICON_SIZE),
                Effect::RefreshShellDisplay,
            ])
        );
    }

    #[test]
    fn a_partial_wheel_notch_leaves_the_desktop_size_alone() {
        let mut app = zoom_app(
            Preset::MatchExplorer,
            ICON_SIZE_DEFAULT,
            Some(ITEM_ICON_SIZE),
        );
        let effects = app.wheel_zoom_effects(WHEEL_DELTA / 2);
        assert_eq!(effects, then_show_bar(vec![]));
    }

    #[test]
    fn an_unreadable_desktop_size_zooms_the_global_size() {
        let mut app = zoom_app(Preset::MatchExplorer, ICON_SIZE_DEFAULT, None);
        let effects = app.zoom_effects(ZoomAction::In);
        assert_eq!(effects, then_show_bar(SIZE_CHANGED.to_vec()));
    }

    #[test]
    fn the_bar_hides_only_once_the_pointer_is_off_it() {
        assert_eq!(zoom_bar_timer_effects(true), vec![]);
        assert_eq!(
            zoom_bar_timer_effects(false),
            vec![Effect::HideZoomBar, Effect::ClearTimer(Timer::ZoomBar)]
        );
    }

    #[test]
    fn the_desktop_size_reads_as_a_percent_of_medium_icons() {
        assert_eq!(ZoomLevel::Desktop(48).percent(), 100);
        assert_eq!(ZoomLevel::Desktop(96).percent(), 200);
        assert_eq!(ZoomLevel::Desktop(16).percent(), 33);
        assert_eq!(ZoomLevel::Desktop(56).percent(), 117);
        assert_eq!(ZoomLevel::Custom(110).percent(), 110);
    }

    #[test]
    fn zooming_stops_at_both_ends_of_its_range() {
        let (small, large) = DESKTOP_ICON_RANGE;
        assert!(!ZoomLevel::Desktop(small).can_zoom_out());
        assert!(ZoomLevel::Desktop(small).can_zoom_in());
        assert!(!ZoomLevel::Desktop(large).can_zoom_in());
        let (small, large) = ICON_SIZE_RANGE;
        assert!(!ZoomLevel::Custom(small).can_zoom_out());
        assert!(!ZoomLevel::Custom(large).can_zoom_in());
        assert!(ZoomLevel::Custom(ICON_SIZE_DEFAULT).can_zoom_in());
    }

    #[test]
    fn reset_applies_only_away_from_the_default() {
        assert!(ZoomLevel::Desktop(ITEM_ICON_SIZE).is_default());
        assert!(!ZoomLevel::Desktop(64).is_default());
        assert!(ZoomLevel::Custom(ICON_SIZE_DEFAULT).is_default());
        let theme = Theme::default();
        let bar = zoom_bar(ZoomLevel::Custom(120), &theme, &theme.palette(false), false);
        assert!(bar.can_reset);
        assert_eq!(bar.percent, 120);
    }

    #[test]
    fn a_clear_theme_still_gets_a_material_and_a_readable_tint() {
        let theme = Theme {
            backdrop: Backdrop::Clear,
            ..Theme::default()
        };
        let mut palette = theme.palette(false);
        palette.tint = palette.tint.with_alpha(20);
        let bar = zoom_bar(ZoomLevel::Custom(100), &theme, &palette, false);
        assert_eq!(bar.style.backdrop, Backdrop::Acrylic);
        assert_eq!(bar.tint.a, ZOOM_BAR_MIN_TINT_ALPHA);
    }

    /// An app with one space shown in `mode` (W39), and its id.
    fn view_app(mode: ViewMode) -> (App, SpaceId) {
        let mut app = zoom_app(Preset::Custom, ICON_SIZE_DEFAULT, None);
        let id = app.config.desktop.next_space_id();
        crate::fixtures::apply(
            &mut app.config.desktop,
            deskspace_core::Command::CreateSpace {
                title: "Files".to_owned(),
                at: deskspace_core::Point::ZERO,
                size: deskspace_core::Size::new(240, 120),
                locked: false,
                icon: deskspace_core::SpaceIcon::None,
                view: SpaceView::default().with_mode(mode),
            },
        );
        (app, id)
    }

    #[test]
    fn the_wheel_steps_a_fixed_view_and_stops_at_its_ends() {
        let view = |mode| SpaceView::default().with_mode(mode);
        let (app, id) = view_app(ViewMode::LargeIcons);
        let surface = Surface::Space(id);
        assert_eq!(
            app.view_step(surface, -1),
            Some((id, view(ViewMode::ExtraLargeIcons))),
            "a wheel up steps toward the bigger views"
        );
        assert_eq!(
            app.view_step(surface, 1),
            Some((id, view(ViewMode::MediumIcons))),
            "a wheel down steps toward the smaller ones"
        );
        assert_eq!(
            app.view_step(surface, 0),
            Some((id, view(ViewMode::LargeIcons))),
            "a partial notch steps nowhere"
        );
        let (app, id) = view_app(ViewMode::Content);
        assert_eq!(
            app.view_step(Surface::Space(id), 3),
            Some((id, view(ViewMode::Content))),
            "a step past the last view stops there"
        );
        let (app, id) = view_app(ViewMode::ExtraLargeIcons);
        assert_eq!(
            app.view_step(Surface::Space(id), -2),
            Some((id, view(ViewMode::ExtraLargeIcons))),
            "a step past the first view stops there"
        );
    }

    #[test]
    fn a_space_without_a_fixed_view_zooms_as_before() {
        let (app, id) = view_app(ViewMode::Icons);
        assert_eq!(
            app.view_step(Surface::Space(id), -1),
            None,
            "the default view zooms"
        );
        assert_eq!(app.view_step(Surface::Desktop, -1), None, "the layer");
        assert_eq!(
            app.view_step(Surface::Space(SpaceId(99)), -1),
            None,
            "a gone space"
        );
    }

    #[test]
    fn the_zoom_keys_step_and_ctrl_zero_returns_to_the_default_view() {
        let view = |mode| SpaceView::default().with_mode(mode);
        let (mut app, id) = view_app(ViewMode::Details);
        let surface = Surface::Space(id);
        app.selection.surface = Some(surface);
        assert_eq!(
            app.zoom_step(Some(surface), ZoomAction::In),
            Some((id, view(ViewMode::List))),
            "Ctrl+Plus steps toward the bigger views"
        );
        assert_eq!(
            app.zoom_step(Some(surface), ZoomAction::Out),
            Some((id, view(ViewMode::Tiles))),
            "Ctrl+Minus steps toward the smaller ones"
        );
        assert_eq!(
            app.zoom_step(Some(surface), ZoomAction::Reset),
            Some((id, view(ViewMode::Icons))),
            "Ctrl+0 returns to Settings icon size"
        );
        assert_eq!(
            app.zoom_step(None, ZoomAction::In),
            None,
            "with no current space the key zooms"
        );
    }
}
