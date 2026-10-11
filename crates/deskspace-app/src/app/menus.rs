//! `App` menus: the tray, space, and item context menus (R1).

use deskspace_core::geometry::Point;
use deskspace_core::{Command, ItemId, Palette, SortKey, Space, SpaceId, SpaceLook, Theme};
use deskspace_platform::{ItemMenuChoice, MenuEntry, Ui, work_area_at};

use super::{
    App, ICON_SIZE_PRESETS, OPACITY_PRESETS, SORT_PRESETS, SpaceColor, Surface,
    delete_needs_confirmation, menu,
};

impl App {
    /// Opens the context menu for the focused item, or for the current space
    /// when no item is selected, as the Menu key or Shift+F10 does.
    pub(super) fn open_context_menu(&mut self, ui: &mut Ui) {
        let Some(surface) = self.selected_surface else {
            return;
        };
        let Some(at) = self.menu_anchor() else {
            return;
        };
        match self.selected_items.last().copied() {
            Some(item) => self.show_item_menu(ui, surface, item, at),
            None => {
                if let Surface::Space(id) = surface {
                    self.show_space_menu(ui, id, at);
                }
            }
        }
    }

    /// Where to anchor a keyboard-opened menu: the center of the focused item's
    /// cell, or the current surface's center when no item is selected.
    pub(super) fn menu_anchor(&self) -> Option<Point> {
        let surface = self.selected_surface?;
        let Some(item) = self.selected_items.last().copied() else {
            return Some(self.surface_window_rect(surface)?.center());
        };
        let entry = self.surface_item(surface, item)?;
        let cell = self.surface_cell(surface, entry);
        Some(Point::new(
            cell.x + cell.width / 2,
            cell.y + cell.height / 2,
        ))
    }

    /// A context-menu appearance edit. It changes one value of the space's own
    /// look, starting from what the space shows now (so a space that followed
    /// Global changes only that value), and customizes the space. It commits
    /// at once.
    fn edit_space_look(
        &mut self,
        ui: &mut Ui,
        id: SpaceId,
        at: Point,
        edit: impl FnOnce(&mut SpaceLook, &Palette),
    ) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let mut look = SpaceLook::from_global(
            space.theme(&self.config.theme),
            &space.grid(self.config.desktop.layout),
        );
        edit(&mut look, &self.palette_for(id));
        let command = Command::SetSpaceLook {
            id,
            custom: true,
            look: Some(look),
        };
        if self.dispatch(command, at) {
            self.space_look_changed(ui, id);
        }
    }

    fn set_space_tint_opacity(&mut self, ui: &mut Ui, id: SpaceId, percent: u8, at: Point) {
        self.edit_space_look(ui, id, at, |look, _| look.theme.tint_opacity = percent);
    }

    fn set_space_icon_size(&mut self, ui: &mut Ui, id: SpaceId, percent: u8, at: Point) {
        self.edit_space_look(ui, id, at, |look, _| look.theme.icon_size = percent);
    }

    /// Picks a custom tint or text color. Custom colors replace both, so
    /// turning them on keeps the other color as it is shown now.
    fn pick_space_color(&mut self, ui: &mut Ui, id: SpaceId, at: Point, which: SpaceColor) {
        let palette = self.palette_for(id);
        let current = match which {
            SpaceColor::Tint => palette.tint.with_alpha(255),
            SpaceColor::Text => palette.title_text,
        };
        let Some(color) = ui.pick_color(current) else {
            return;
        };
        self.edit_space_look(ui, id, at, |look, palette| {
            let theme = &mut look.theme;
            if !theme.custom_colors {
                theme.custom_colors = true;
                theme.tint = palette.tint.with_alpha(255);
                theme.text = palette.title_text;
            }
            match which {
                SpaceColor::Tint => theme.tint = color.with_alpha(255),
                SpaceColor::Text => theme.text = color.with_alpha(255),
            }
        });
    }

    /// The menu's "Use global appearance": the space follows Global again.
    /// Its look is kept, so customizing it later restores it.
    fn use_global_appearance(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let command = Command::CustomizeSpace {
            id,
            on: false,
            seed: SpaceLook::from_global(&self.config.theme, &self.config.desktop.layout),
        };
        if self.dispatch(command, at) {
            self.space_look_changed(ui, id);
        }
    }

    /// Shows a space's changed look, saves it, and keeps an open settings
    /// window in step: the change is taken into its Cancel snapshot (a menu
    /// edit commits at once, so Cancel must not undo it) and shown.
    fn space_look_changed(&mut self, ui: &mut Ui, id: SpaceId) {
        self.restyle_space(ui, id);
        self.reflow_space(ui, id);
        self.schedule_save(ui);
        if let (Some(session), Some(space)) =
            (self.settings.as_mut(), self.config.desktop.space(id))
        {
            session.resnapshot(space);
            self.refresh_settings(ui);
        }
    }

    fn sort_space(&mut self, ui: &mut Ui, id: SpaceId, key: SortKey, descending: bool, at: Point) {
        self.apply_to_space(
            ui,
            id,
            Command::SortSpace {
                id,
                key,
                descending,
            },
            at,
        );
    }

    pub(super) fn show_item_menu(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        item: ItemId,
        at: Point,
    ) {
        let Some(path) = self.config.desktop.item(item).map(|item| item.path.clone()) else {
            return;
        };
        // A loose desktop item has no space to remove it from, so only
        // Explorer's own menu is offered for it.
        let entries = match surface {
            Surface::Space(_) => {
                let remove_label = if self.storage.contains(&path) {
                    "Move back to desktop"
                } else {
                    "Remove from space"
                };
                vec![
                    MenuEntry::item(menu::RENAME_ITEM, "Rename"),
                    MenuEntry::item(menu::REMOVE_ITEM, remove_label),
                ]
            }
            Surface::Desktop => Vec::new(),
        };
        match ui.item_menu(&path, &entries, at) {
            Some(ItemMenuChoice::App(menu::RENAME_ITEM)) => {
                if let Surface::Space(id) = surface {
                    self.begin_item_rename(ui, id, item);
                }
            }
            Some(ItemMenuChoice::App(menu::REMOVE_ITEM)) => {
                if self.release_item(item) {
                    self.selected_items.retain(|selected| *selected != item);
                    self.forget_item(item);
                    self.hovered = None;
                    self.refresh_surface(ui, surface);
                    self.refresh_tray(ui);
                    self.save();
                }
            }
            Some(ItemMenuChoice::Shell) => {
                self.icons.invalidate(&path);
                match surface {
                    Surface::Space(id) => {
                        self.prune_missing(ui, id);
                        self.refresh_space(ui, id);
                    }
                    Surface::Desktop => self.resync_desktop_layer(ui),
                }
            }
            _ => {}
        }
    }

    pub(super) fn show_space_menu(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let roll = if space.rolled_up {
            "Roll down"
        } else {
            "Roll up"
        };
        let mut entries = vec![
            MenuEntry::item(menu::ROLL, roll),
            MenuEntry::item(menu::RENAME, "Rename"),
            MenuEntry::item(menu::LOCK, "Lock position").checked(space.locked),
        ];
        if space.user_sized && !space.locked {
            entries.push(MenuEntry::item(menu::FIT, "Fit to contents"));
        }
        entries.push(MenuEntry::Separator);
        entries.push(space_appearance_menu(
            space,
            space.theme(&self.config.theme),
        ));
        entries.push(space_sort_menu());
        entries.push(MenuEntry::Separator);
        // The ellipsis promises a dialog, which only non-empty spaces show.
        let delete = if delete_needs_confirmation(space) {
            "Delete space\u{2026}"
        } else {
            "Delete space"
        };
        entries.extend([
            MenuEntry::item(menu::DELETE, delete),
            MenuEntry::Separator,
            MenuEntry::item(menu::NEW_SPACE, "New space"),
            MenuEntry::Separator,
            MenuEntry::item(menu::SETTINGS, "Settings\u{2026}"),
            self.visibility_entry(),
            MenuEntry::item(menu::EXIT, "Exit DeskSpace"),
        ]);
        match ui.popup_menu(&entries, at) {
            Some(menu::ROLL) => self.toggle_rollup(ui, id),
            Some(menu::RENAME) => self.begin_rename(ui, id),
            Some(menu::DELETE) => self.delete_space(ui, id),
            Some(menu::FIT) => {
                if self.apply_to_space(ui, id, Command::FitToContents { id }, at) {
                    self.scroll.remove(&id);
                }
            }
            Some(menu::LOCK) => {
                self.apply_to_space(ui, id, Command::ToggleLock { id }, at);
            }
            Some(menu::SPACE_TINT) => self.pick_space_color(ui, id, at, SpaceColor::Tint),
            Some(menu::SPACE_TEXT) => self.pick_space_color(ui, id, at, SpaceColor::Text),
            Some(menu::RESET_APPEARANCE) => self.use_global_appearance(ui, id, at),
            Some(menu::SPACE_SETTINGS) => self.open_settings_with(ui, Some(id)),
            Some(choice) => {
                if let Some((_, _, percent)) = OPACITY_PRESETS
                    .iter()
                    .find(|(preset, ..)| *preset == choice)
                {
                    self.set_space_tint_opacity(ui, id, *percent, at);
                } else if let Some((_, _, icon_size)) = ICON_SIZE_PRESETS
                    .iter()
                    .find(|(preset, ..)| *preset == choice)
                {
                    self.set_space_icon_size(ui, id, *icon_size, at);
                } else if let Some((_, _, key, descending)) =
                    SORT_PRESETS.iter().find(|(preset, ..)| *preset == choice)
                {
                    self.sort_space(ui, id, *key, *descending, at);
                } else {
                    self.run_common_menu(ui, choice, at);
                }
            }
            None => {}
        }
    }

    pub(super) fn show_tray_menu(&mut self, ui: &mut Ui, at: Point) {
        let entries = [
            MenuEntry::item(menu::SETTINGS, "Settings\u{2026}"),
            self.visibility_entry(),
            MenuEntry::item(menu::NEW_SPACE, "New space"),
            MenuEntry::Separator,
            MenuEntry::item(menu::EXIT, "Exit DeskSpace"),
        ];
        if let Some(choice) = ui.popup_menu(&entries, at) {
            let work = work_area_at(at);
            self.run_common_menu(ui, choice, work.center());
        }
    }

    pub(super) fn run_common_menu(&mut self, ui: &mut Ui, choice: u32, at: Point) {
        match choice {
            menu::NEW_SPACE => self.create_space(ui, at),
            menu::SETTINGS => self.open_settings(ui),
            menu::TOGGLE_VISIBLE => self.set_visible(ui, !self.visible),
            menu::EXIT => ui.quit(),
            _ => {}
        }
    }

    pub(super) fn create_space(&mut self, ui: &mut Ui, at: Point) {
        let command = Command::CreateSpace {
            title: "New space".to_string(),
            at,
        };
        if self.dispatch(command, at) {
            if !self.visible {
                self.set_visible(ui, true);
            }
            self.sync_windows(ui);
            self.schedule_save(ui);
        }
    }
}

/// The "Appearance" submenu for a space drawn with `theme`: tint-opacity
/// presets, the icon size, custom tint and text colors, a way back to the
/// global appearance, and the space's settings page. Every edit customizes
/// the space.
fn space_appearance_menu(space: &Space, theme: &Theme) -> MenuEntry {
    let mut entries: Vec<MenuEntry> = OPACITY_PRESETS
        .iter()
        .map(|(id, label, value)| {
            MenuEntry::item(*id, *label).checked(theme.tint_opacity == *value)
        })
        .collect();
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::submenu(
        "Icon size",
        ICON_SIZE_PRESETS
            .iter()
            .map(|(id, label, value)| {
                MenuEntry::item(*id, *label).checked(theme.icon_size == *value)
            })
            .collect(),
    ));
    entries.push(MenuEntry::Separator);
    entries.push(
        MenuEntry::item(menu::SPACE_TINT, "Custom tint\u{2026}").checked(theme.custom_colors),
    );
    entries.push(
        MenuEntry::item(menu::SPACE_TEXT, "Custom text\u{2026}").checked(theme.custom_colors),
    );
    entries.push(
        MenuEntry::item(menu::RESET_APPEARANCE, "Use global appearance").enabled(space.custom),
    );
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::item(
        menu::SPACE_SETTINGS,
        "Space settings\u{2026}",
    ));
    MenuEntry::submenu("Appearance", entries)
}

/// The "Sort by" submenu; a one-off reorder of the space's items.
fn space_sort_menu() -> MenuEntry {
    MenuEntry::submenu(
        "Sort by",
        SORT_PRESETS
            .iter()
            .map(|(id, label, ..)| MenuEntry::item(*id, *label))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::{Item, Rect};

    #[test]
    fn keyboard_menu_anchor_follows_the_focus() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = app
            .config
            .desktop
            .add_space("S", Rect::new(100, 100, 240, 200));
        let mut item = Item::new(ItemId(1), "C:/missing/a.txt");
        item.position = Point::new(12, 12);
        app.config.desktop.add_item(id, item);
        app.selected_surface = Some(Surface::Space(id));

        let (rect, content) = {
            let space = app.config.desktop.space(id).unwrap();
            (space.rect, space.content_rect())
        };
        let layout = app.config.desktop.layout;

        app.selected_items.clear();
        assert_eq!(app.menu_anchor(), Some(rect.center()));

        app.selected_items = vec![ItemId(1)];
        assert_eq!(
            app.menu_anchor(),
            Some(Point::new(
                content.x + 12 + layout.cell_width / 2,
                content.y + 12 + layout.cell_height / 2,
            ))
        );
    }
}
