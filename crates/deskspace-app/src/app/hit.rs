//! `App` hit-testing: the `surface_*` accessors and item lookup (R1).

use std::path::{Path, PathBuf};

use deskspace_core::geometry::{Point, Rect};
use deskspace_core::layout::GridSpec;
use deskspace_core::{Item, ItemId, Palette, SpaceId, Theme};
use deskspace_platform::{CheckBoxMode, ItemMetrics, item_check_box_hit, virtual_desktop_rect};

use super::{App, Surface};

impl App {
    /// The space's scroll offset, clamped to its current content.
    pub(super) fn scroll_of(&self, id: SpaceId) -> i32 {
        let layout = self.layout_for(id);
        let max = self
            .config
            .desktop
            .space(id)
            .map_or(0, |space| space.max_scroll(layout.cell(), layout.padding));
        self.scroll.get(&id).copied().unwrap_or(0).clamp(0, max)
    }

    /// A surface's items, in model order.
    pub(super) fn surface_items(&self, surface: Surface) -> &[Item] {
        match surface {
            Surface::Space(id) => self
                .config
                .desktop
                .space(id)
                .map_or(&[][..], |space| &space.items),
            Surface::Desktop => self.config.desktop.layer().items(),
        }
    }

    /// The item with `id` on `surface`, when it is there.
    pub(super) fn surface_item(&self, surface: Surface, id: ItemId) -> Option<&Item> {
        self.surface_items(surface)
            .iter()
            .find(|item| item.id == id)
    }

    /// The content-area origin of a surface in desktop coordinates: a space's
    /// content area, or the virtual-desktop origin for the layer, whose item
    /// positions are already absolute.
    pub(super) fn surface_content_origin(&self, surface: Surface) -> Point {
        match surface {
            Surface::Space(id) => self
                .config
                .desktop
                .space(id)
                .map_or(Point::ZERO, |space| space.content_rect().origin()),
            Surface::Desktop => Point::ZERO,
        }
    }

    /// The rect a surface's window occupies on screen.
    pub(super) fn surface_window_rect(&self, surface: Surface) -> Option<Rect> {
        match surface {
            Surface::Space(id) => self
                .config
                .desktop
                .space(id)
                .map(|space| space.visible_rect()),
            Surface::Desktop => Some(virtual_desktop_rect()),
        }
    }

    /// The area a rubber-band selection clamps to, in desktop coordinates.
    pub(super) fn surface_content(&self, surface: Surface) -> Option<Rect> {
        match surface {
            Surface::Space(id) => self
                .config
                .desktop
                .space(id)
                .map(|space| space.content_rect()),
            Surface::Desktop => Some(virtual_desktop_rect()),
        }
    }

    /// A surface's scroll offset; the layer never scrolls.
    pub(super) fn surface_scroll(&self, surface: Surface) -> i32 {
        match surface {
            Surface::Space(id) => self.scroll_of(id),
            Surface::Desktop => 0,
        }
    }

    /// The scaled grid a surface lays out on.
    pub(super) fn surface_layout(&self, surface: Surface) -> GridSpec {
        match surface {
            Surface::Space(id) => self.layout_for(id),
            Surface::Desktop => self.effective_layout(),
        }
    }

    /// Scaled icon, label, and check-box sizes for a surface.
    pub(super) fn surface_metrics(&self, surface: Surface) -> ItemMetrics {
        match surface {
            Surface::Space(id) => self.metrics_for(id),
            Surface::Desktop => ItemMetrics::for_scale(self.icon_scale(&self.config.theme)),
        }
    }

    /// The theme a surface is drawn with: a space's own look while customized,
    /// else the global theme.
    pub(super) fn surface_theme(&self, surface: Surface) -> &Theme {
        match surface {
            Surface::Space(id) => self.theme_for(id),
            Surface::Desktop => &self.config.theme,
        }
    }

    /// The concrete colors for a surface, with the Windows accent when asked.
    pub(super) fn surface_palette(&self, surface: Surface) -> Palette {
        let theme = self.surface_theme(surface);
        let mut palette = theme.palette(self.system_dark);
        if let Some(accent) = self.system_accent {
            // Explorer always fills a checked item check box with the accent,
            // whatever the space's own selection colors are. The registry
            // accent's alpha is unreliable, so force it opaque.
            palette.check_fill = accent.with_alpha(255);
            if theme.use_accent {
                palette = palette.with_accent(accent);
            }
        }
        palette
    }

    /// Whether a surface mirrors Explorer's display settings.
    pub(super) fn surface_match_explorer(&self, surface: Surface) -> bool {
        self.surface_theme(surface).match_explorer()
    }

    /// How a surface shows item check boxes, honoring its `match_explorer`.
    pub(super) fn surface_check_box_mode(&self, surface: Surface) -> CheckBoxMode {
        if self.surface_match_explorer(surface) {
            self.shell_display.check_boxes
        } else {
            CheckBoxMode::Hidden
        }
    }

    /// The rect of an item's cell on `surface`, in desktop coordinates.
    pub(super) fn surface_cell(&self, surface: Surface, item: &Item) -> Rect {
        let origin = self.surface_content_origin(surface);
        let cell = self.surface_layout(surface).cell();
        Rect::new(
            origin.x + item.position.x,
            origin.y + item.position.y - self.surface_scroll(surface),
            cell.width,
            cell.height,
        )
    }

    /// Every item's cell on `surface`, in desktop coordinates.
    pub(super) fn surface_cells(&self, surface: Surface) -> Vec<(ItemId, Rect)> {
        let origin = self.surface_content_origin(surface);
        let scroll = self.surface_scroll(surface);
        let cell = self.surface_layout(surface).cell();
        self.surface_items(surface)
            .iter()
            .map(|item| {
                (
                    item.id,
                    Rect::new(
                        origin.x + item.position.x,
                        origin.y + item.position.y - scroll,
                        cell.width,
                        cell.height,
                    ),
                )
            })
            .collect()
    }

    /// The item whose cell contains `point` (desktop coordinates).
    pub(super) fn surface_item_at(&self, surface: Surface, point: Point) -> Option<ItemId> {
        let cell = self.surface_layout(surface).cell();
        match surface {
            Surface::Space(id) => self
                .config
                .desktop
                .space(id)
                .and_then(|space| space.item_at(point, cell, self.surface_scroll(surface))),
            Surface::Desktop => self.config.desktop.layer().item_at(point, cell),
        }
    }

    /// The item whose Explorer-style check box contains `point`, when check
    /// boxes are shown. `point` is in desktop coordinates.
    pub(super) fn checkbox_at(&self, surface: Surface, point: Point) -> Option<ItemId> {
        if self.surface_check_box_mode(surface) == CheckBoxMode::Hidden {
            return None;
        }
        let item = self.surface_item_at(surface, point)?;
        let entry = self.surface_item(surface, item)?;
        let cell = self.surface_cell(surface, entry);
        item_check_box_hit(cell, self.surface_metrics(surface))
            .contains(point)
            .then_some(item)
    }

    /// The item holding the file at `path`, comparing identity rather than the
    /// literal string so a shell-reported short name or case difference still
    /// matches. A path in a space must be moved, not added again.
    pub(super) fn find_item_by_file(&self, path: &Path) -> Option<ItemId> {
        self.config
            .desktop
            .spaces
            .iter()
            .flat_map(|space| space.items.iter())
            .find(|item| crate::storage::same_path(&item.path, path))
            .map(|item| item.id)
    }

    /// Whether `point` (desktop coordinates) is still over the surface's own
    /// content, as opposed to another window such as a space on top of the
    /// desktop layer.
    pub(super) fn surface_contains(&self, surface: Surface, point: Point) -> bool {
        match surface {
            Surface::Space(id) => self
                .config
                .desktop
                .space(id)
                .is_some_and(|space| space.visible_rect().contains(point)),
            Surface::Desktop => self.config.desktop.space_at(point).is_none(),
        }
    }

    /// The path of each of `ids` on `surface` and the top-left of its cell in
    /// desktop coordinates, where it is drawn now (off the visible area when
    /// scrolled out), in `ids` order.
    pub(super) fn item_cells(&self, surface: Surface, ids: &[ItemId]) -> Vec<(PathBuf, Point)> {
        ids.iter()
            .filter_map(|id| {
                let item = self.surface_item(surface, *id)?;
                let cell = self.surface_cell(surface, item);
                Some((item.path.clone(), Point::new(cell.x, cell.y)))
            })
            .collect()
    }

    pub(super) fn space_center(&self, id: SpaceId) -> Point {
        self.config
            .desktop
            .space(id)
            .map_or(Point::ZERO, |space| space.rect.center())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::ItemFacts;

    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::Item;
    use deskspace_platform::ShellDisplay;

    #[test]
    fn checkbox_hit_test_matches_the_icon_corner() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = app
            .config
            .desktop
            .add_space("S", Rect::new(100, 100, 240, 200));
        let mut item = Item::new(ItemId(1), "C:/missing/a.txt", ItemFacts::default());
        item.position = Point::ZERO;
        app.config.desktop.add_item(id, item);
        app.shell_display = ShellDisplay {
            check_boxes: CheckBoxMode::Selection,
            ..ShellDisplay::default()
        };
        let layout = app.config.desktop.layout;
        let cell = Rect::new(100, 100 + 28, layout.cell_width, layout.cell_height);
        let checkbox = item_check_box_hit(cell, app.metrics_for(id));
        assert_eq!(
            app.checkbox_at(
                Surface::Space(id),
                Point::new(checkbox.x + 3, checkbox.y + 3)
            ),
            Some(ItemId(1))
        );
        assert_eq!(
            app.checkbox_at(Surface::Space(id), Point::new(cell.x + 60, cell.y + 60)),
            None
        );
    }

    #[test]
    fn checked_boxes_use_the_windows_accent() {
        use deskspace_core::Color;

        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = app
            .config
            .desktop
            .add_space("S", Rect::new(100, 100, 240, 200));
        // A registry accent often carries alpha 0; the box forces it opaque.
        app.system_accent = Some(Color::rgba(0x00, 0x78, 0xD4, 0x00));
        assert!(
            !app.config.theme.use_accent,
            "the default selection is not the accent"
        );
        let palette = app.surface_palette(Surface::Space(id));
        assert_eq!(palette.check_fill, Color::rgb(0x00, 0x78, 0xD4));
        // Without a known accent, the Windows-blue fallback stands.
        app.system_accent = None;
        assert_eq!(
            app.surface_palette(Surface::Space(id)).check_fill,
            deskspace_core::theme::CHECK_ACCENT
        );
    }
}
