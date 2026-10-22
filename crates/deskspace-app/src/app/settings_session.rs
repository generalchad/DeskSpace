//! `App` settings session: the settings window, its edits, and previews (R1).

use std::path::PathBuf;

use deskspace_core::geometry::{Point, Rect};
use deskspace_core::{Command, SpaceId};
use deskspace_platform::{
    FieldValue, PreviewItem, SettingsEvent, SpacePreview, Ui, WatchKey, windows_build,
};

use crate::scene::item_label;
use crate::settings::{self, ButtonAction, Effect, ScopeKind, Session, SpaceSnapshot};

use super::{App, Preview, Surface, Timer};

/// How long a settings preview may lag the edit that caused it. `WM_TIMER`
/// is only delivered once the queue is empty, so a burst of slider ticks
/// collapses into one restyle or re-flow.
const PREVIEW_DELAY_MS: u32 = 15;
/// How long the space selected in the settings window stays highlighted.
const HIGHLIGHT_MS: u32 = 1200;

/// The pixel size of the icons rendered for the Shortcuts page preview. The
/// preview draws them at 40 px at 100 % scaling, so 64 px stays sharp up to
/// 150 %.
const PREVIEW_ICON_SIZE: i32 = 64;

/// The open settings window's state: its session, the preview it still owes
/// the spaces, and the space it highlights.
#[derive(Debug, Default)]
pub(super) struct SettingsUi {
    session: Option<Session>,
    /// A settings edit is waiting for `Timer::Preview` to restyle or re-flow
    /// every space.
    pending_preview: Preview,
    /// The space highlighted for the settings window, until `Timer::Highlight`.
    highlighted: Option<SpaceId>,
}

impl SettingsUi {
    pub(super) fn is_open(&self) -> bool {
        self.session.is_some()
    }

    pub(super) fn session_mut(&mut self) -> Option<&mut Session> {
        self.session.as_mut()
    }

    pub(super) fn highlighted(&self) -> Option<SpaceId> {
        self.highlighted
    }
}

impl App {
    /// Deletes the space shown in the settings window, from the header's
    /// "Delete space" button. Deletion is immediate and Cancel does not bring
    /// the space back, matching the space menu. The window falls back to
    /// Global when the scoped space is gone.
    pub(super) fn delete_space_from_settings(&mut self, ui: &mut Ui, id: SpaceId) {
        self.delete_space(ui, id);
        if self.config.desktop.space(id).is_some() {
            // Not confirmed, or kept because a stored file could not be moved
            // back to the desktop.
            self.refresh_settings(ui);
            return;
        }
        if let Some(session) = self.settings_ui.session.as_mut() {
            session.forget_space(id);
        }
        self.clear_highlight(ui);
        self.refresh_settings(ui);
    }

    fn settings_form(&mut self) -> Option<deskspace_platform::SettingsForm> {
        let scope = self.settings_ui.session.as_ref()?.scope;
        let preview = self.space_preview(scope);
        self.settings_ui.session.as_ref().map(|session| {
            settings::build_form(
                &self.config,
                session,
                self.system.dark(),
                self.system.animations(),
                self.store.path(),
                preview,
            )
        })
    }

    /// What the Shortcuts page shows for the space `scope`: its title, its
    /// colors as opaque fills, and its items' labels and icons as the space
    /// draws them. Empty for Global, which never lists the page.
    fn space_preview(&mut self, scope: Option<SpaceId>) -> SpacePreview {
        let Some(space) = scope.and_then(|id| self.config.desktop.space(id)) else {
            return SpacePreview::default();
        };
        let id = space.id;
        let show_extensions =
            self.surface_match_explorer(Surface::Space(id)) && self.system.shell().show_extensions;
        let overlay_size = space.theme(&self.config.theme).overlay_size;
        let title = space.title().to_owned();
        let entries: Vec<(PathBuf, String)> = space
            .items()
            .iter()
            .map(|item| (item.path.clone(), item_label(item, show_extensions)))
            .collect();
        let (fill, title_fill, text) = settings::space_preview_colors(&self.palette_for(id));
        let items = entries
            .into_iter()
            .map(|(path, label)| PreviewItem {
                label,
                icon: self.icons.icon_for(&path, PREVIEW_ICON_SIZE, overlay_size),
            })
            .collect();
        SpacePreview {
            title,
            fill,
            title_fill,
            text,
            available: !self.unavailable_lenses.contains(&id),
            items,
        }
    }

    pub(super) fn open_settings(&mut self, ui: &mut Ui) {
        self.open_settings_with(ui, None);
    }

    /// Opens the settings window, or brings it to the front, on `scope` when
    /// one is given (a space's "Space settings…"); otherwise where it was.
    pub(super) fn open_settings_with(&mut self, ui: &mut Ui, scope: Option<SpaceId>) {
        let config = &self.config;
        let session = self
            .settings_ui
            .session
            .get_or_insert_with(|| Session::new(config));
        if scope.is_some() {
            let kind = ScopeKind::of(scope.and_then(|id| config.desktop.space(id)));
            session.select_scope(scope, kind);
        }
        if let Some(form) = self.settings_form() {
            ui.open_settings(&form);
        }
        if let Some(id) = scope {
            self.watch_highlight(ui, id);
        }
    }

    /// Highlights a space so it can be found on the desktop, and arms the
    /// timer that clears it. Selecting another space moves the highlight.
    pub(super) fn watch_highlight(&mut self, ui: &mut Ui, id: SpaceId) {
        if self.settings_ui.highlighted != Some(id) {
            self.clear_highlight(ui);
            self.settings_ui.highlighted = Some(id);
            self.refresh_space(ui, id);
        }
        Self::set_timer(ui, Timer::Highlight, HIGHLIGHT_MS);
    }

    /// Removes the settings-window highlight, if any.
    pub(super) fn clear_highlight(&mut self, ui: &mut Ui) {
        if let Some(id) = self.settings_ui.highlighted.take() {
            Self::clear_timer(ui, Timer::Highlight);
            if self.config.desktop.space(id).is_some() {
                self.refresh_space(ui, id);
            }
        }
    }

    pub(super) fn refresh_settings(&mut self, ui: &mut Ui) {
        if let Some(form) = self.settings_form() {
            ui.update_settings(&form);
        }
    }

    pub(super) fn on_settings_event(&mut self, ui: &mut Ui, event: SettingsEvent) {
        let Some(session) = self.settings_ui.session.as_mut() else {
            return;
        };
        let editing_space = session.scope.is_some();
        match event {
            SettingsEvent::Changed { field, value } if editing_space => {
                self.on_space_field(ui, field, value);
            }
            SettingsEvent::Changed { field, value } => {
                let effect = settings::apply_field(
                    &mut self.config,
                    session,
                    field,
                    value,
                    self.system.dark(),
                );
                if effect == Effect::Appearance {
                    ui.set_ui_dark(self.ui_dark());
                    self.schedule_preview(ui, Preview::restyle_all());
                } else if effect == Effect::Layout {
                    self.schedule_preview(ui, Preview::relayout_all());
                }
                self.refresh_settings(ui);
            }
            SettingsEvent::Button { field } => {
                if let Some(action) = settings::button_action(field) {
                    self.on_settings_button(ui, action);
                }
            }
            SettingsEvent::HeaderSwitch { on } => {
                let command = settings::customize_space(&self.config, session, on);
                self.apply_space_setting(ui, command);
            }
            SettingsEvent::HeaderButton => {
                if let Some(id) = session.scope {
                    self.delete_space_from_settings(ui, id);
                }
            }
            SettingsEvent::ScopeChanged { scope } => {
                let id = settings::scope_at(&self.config, scope);
                let kind = ScopeKind::of(id.and_then(|id| self.config.desktop.space(id)));
                session.select_scope(id, kind);
                self.refresh_settings(ui);
                match id {
                    Some(id) => self.watch_highlight(ui, id),
                    None => self.clear_highlight(ui),
                }
            }
            SettingsEvent::NavChanged { entry } => {
                session.select_entry(entry);
                self.refresh_settings(ui);
            }
            SettingsEvent::PreviewRemove { index, .. } => {
                if let Some(id) = session.scope {
                    self.delete_space_item(ui, id, index);
                }
            }
            SettingsEvent::PreviewAdd { x, y, .. } => {
                if let Some(id) = session.scope {
                    self.offer_new_shortcut(ui, id, Point::new(x, y));
                }
            }
            SettingsEvent::Apply => {
                self.apply_settings(ui);
                self.refresh_settings(ui);
            }
            SettingsEvent::Ok => {
                if self.apply_settings(ui) {
                    self.settings_ui.session = None;
                    self.clear_highlight(ui);
                    ui.close_settings();
                } else {
                    self.refresh_settings(ui);
                }
            }
            SettingsEvent::Cancel => {
                self.cancel_settings(ui);
                ui.close_settings();
            }
        }
    }

    fn on_settings_button(&mut self, ui: &mut Ui, action: ButtonAction) {
        let Some(session) = self.settings_ui.session.as_mut() else {
            return;
        };
        match (action, session.scope) {
            (ButtonAction::ResetAppearance, Some(_)) => {
                let command = settings::match_global(&self.config, session);
                self.apply_space_setting(ui, command);
            }
            (ButtonAction::ResetAppearance, None) => {
                settings::reset_appearance(&mut self.config, session);
                ui.set_ui_dark(self.ui_dark());
                self.restyle_all(ui);
                self.refresh_settings(ui);
            }
            (ButtonAction::FitToContents, Some(id)) => {
                let at = self.space_center(id);
                if self
                    .commit(ui, Command::FitToContents { id }, at)
                    .is_applied()
                {
                    self.view_cache.set_scroll(id, 0);
                }
                self.refresh_settings(ui);
            }
            // The button is grey for Global, which has no space to fit.
            (ButtonAction::FitToContents, None) => {}
            (ButtonAction::CheckForUpdates, _) => self.start_update_check(ui, true),
            (ButtonAction::OpenLensFolder, Some(id)) => self.open_lens_folder(ui, id),
            (ButtonAction::ChangeLensFolder, Some(id)) => {
                let at = self.space_center(id);
                self.change_lens_folder(ui, id, at);
            }
            (ButtonAction::OpenLensFolder | ButtonAction::ChangeLensFolder, None) => {}
            (ButtonAction::OpenWindowsAnimationSettings, _) => {
                ui.open_url(settings::windows_animation_settings_uri(windows_build()));
            }
        }
    }

    /// Defers the space refresh a settings edit needs. The timer is armed once
    /// and not restarted, so the spaces keep following a slider drag while the
    /// ticks in between collapse into one pass.
    pub(super) fn schedule_preview(&mut self, ui: &mut Ui, preview: Preview) {
        if !self.settings_ui.pending_preview.is_pending() {
            Self::set_timer(ui, Timer::Preview, PREVIEW_DELAY_MS);
        }
        self.settings_ui.pending_preview.merge(preview);
    }

    /// Runs the deferred settings preview.
    pub(super) fn flush_preview(&mut self, ui: &mut Ui) {
        Self::clear_timer(ui, Timer::Preview);
        let preview = std::mem::take(&mut self.settings_ui.pending_preview);
        if preview.restyle_all {
            self.restyle_all(ui);
        } else {
            for id in preview.restyle {
                self.restyle_space(ui, id);
            }
        }
        if preview.relayout_all {
            self.relayout_all(ui);
        } else {
            for id in preview.relayout {
                self.reflow_space(ui, id);
            }
        }
    }

    /// An edit on a space's page: dispatch its command at once, and preview
    /// it on that space alone.
    fn on_space_field(&mut self, ui: &mut Ui, field: u32, value: FieldValue) {
        let Some(session) = self.settings_ui.session.as_mut() else {
            return;
        };
        let edit = settings::space_edit(&self.config, session, field, value, self.system.dark());
        // Not `commit`: the edit is previewed now and saved by Apply.
        if let Some((id, command, effect)) = edit
            && self.dispatch(command, self.space_center(id)).is_applied()
        {
            match effect {
                Effect::Layout => self.schedule_preview(ui, Preview::relayout(id)),
                Effect::Lens => {
                    // A changed sort or filter needs a fresh listing.
                    ui.unwatch_folder(WatchKey::Space(id));
                    self.sync_watches(ui);
                }
                _ => self.schedule_preview(ui, Preview::restyle(id)),
            }
        }
        self.refresh_settings(ui);
    }

    /// Dispatches a space command from the settings window (the Customize
    /// switch, "Match global appearance") and shows its result.
    fn apply_space_setting(&mut self, ui: &mut Ui, command: Option<Command>) {
        let id = match &command {
            Some(Command::CustomizeSpace { id, .. } | Command::SetSpaceLook { id, .. }) => *id,
            _ => return self.refresh_settings(ui),
        };
        // Not `commit`: a new look needs a restyle and re-flow, not a repaint.
        if let Some(command) = command
            && self.dispatch(command, self.space_center(id)).is_applied()
        {
            self.restyle_space(ui, id);
            self.reflow_space(ui, id);
        }
        self.refresh_settings(ui);
    }

    /// Commits pending settings. Returns false (and keeps the window open)
    /// when the hotkey cannot be used.
    pub(super) fn apply_settings(&mut self, ui: &mut Ui) -> bool {
        let Some(texts) = self
            .settings_ui
            .session
            .as_ref()
            .map(|session| session.hotkey_texts.clone())
        else {
            return false;
        };
        for (slot, binding) in settings::HOTKEY_BINDINGS.iter().enumerate() {
            let text = texts[slot].trim().to_string();
            let current = binding.text(&self.config.window).to_string();
            if text == current && self.hotkeys.is_registered(slot) {
                continue;
            }
            if let Err(message) = self.hotkeys.register(ui, slot, &text) {
                // Put the previous binding back so the hotkey keeps working.
                let _ = self.hotkeys.register(ui, slot, &current);
                if let Some(session) = self.settings_ui.session.as_mut() {
                    session.hotkey_errors[slot] = Some(message);
                }
                return false;
            }
            binding.set_text(&mut self.config.window, text);
        }
        let start_changed = self.settings_ui.session.as_ref().is_some_and(|session| {
            session.saved_window.start_with_windows != self.config.window.start_with_windows
        });
        if start_changed {
            self.sync_start_with_windows(self.config.window.start_with_windows);
        }
        let takeover_changed = self.settings_ui.session.as_ref().is_some_and(|session| {
            session.saved_window.take_over_desktop != self.config.window.take_over_desktop
        });
        if takeover_changed {
            self.apply_takeover(ui);
        }
        let update_check_changed = self.settings_ui.session.as_ref().is_some_and(|session| {
            session.saved_window.check_for_updates != self.config.window.check_for_updates
        });
        if update_check_changed {
            self.sync_update_schedule(ui);
        }
        self.save(ui);
        if let Some(session) = self.settings_ui.session.as_mut() {
            session.commit(&self.config);
        }
        true
    }

    pub(super) fn cancel_settings(&mut self, ui: &mut Ui) {
        let Some(session) = self.settings_ui.session.take() else {
            return;
        };
        // The revert below restyles and re-flows from the saved settings, which
        // covers any preview still waiting for its timer.
        if std::mem::take(&mut self.settings_ui.pending_preview).is_pending() {
            Self::clear_timer(ui, Timer::Preview);
        }
        self.clear_highlight(ui);
        let restyle = settings::needs_restyle(&session, &self.config);
        // A revert re-flows when the grid changed: the layout itself, the
        // icon size that scales it, or a Match Explorer toggle, which
        // imports Explorer's own icon size and spacing.
        let relayout = settings::needs_relayout(&session, &self.config);
        self.restore_spaces(ui, &session.saved_spaces);
        session.revert(&mut self.config);
        ui.set_ui_dark(self.ui_dark());
        if restyle {
            self.restyle_all(ui);
        }
        if relayout {
            self.relayout_all(ui);
        }
        self.unpin_spaces(ui, &session.pinned);
    }

    /// Cancel: spaces an icon-size preview pinned fit their items again,
    /// unless they were resized since.
    fn unpin_spaces(&mut self, ui: &mut Ui, pinned: &[(SpaceId, Rect)]) {
        for (id, rect) in pinned {
            let still_pinned = self
                .config
                .desktop
                .space(*id)
                .is_some_and(|space| space.user_sized() && space.rect() == *rect);
            if still_pinned {
                self.commit(ui, Command::FitToContents { id: *id }, rect.center());
            }
        }
    }

    /// Cancel: puts back every space whose customization changed while the
    /// window was open. Spaces deleted meanwhile are skipped.
    fn restore_spaces(&mut self, ui: &mut Ui, saved: &[SpaceSnapshot]) {
        for snapshot in saved {
            let Some(space) = self.config.desktop.space(snapshot.id) else {
                continue;
            };
            if SpaceSnapshot::of(space) == *snapshot {
                continue;
            }
            let at = space.rect().center();
            // Not `commit`: a restored look needs a restyle and re-flow.
            let look_restored = self.dispatch(snapshot.restore(), at).is_applied();
            let view_restored = snapshot
                .restore_view()
                .is_some_and(|command| self.dispatch(command, at).is_applied());
            if look_restored || view_restored {
                self.restyle_space(ui, snapshot.id);
                self.reflow_space(ui, snapshot.id);
            }
            if view_restored {
                ui.unwatch_folder(WatchKey::Space(snapshot.id));
                self.sync_watches(ui);
            }
        }
    }
}
