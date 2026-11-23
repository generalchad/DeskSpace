//! Look, scale, and layout accessors for `App`.

use deskspace_core::geometry::Size;
use deskspace_core::layout::GridSpec;
use deskspace_core::theme::ICON_SIZE_RANGE;
use deskspace_core::view_layout::{ViewLayout, ViewMetrics, fixed_view_cell_width};
use deskspace_core::{Palette, SpaceId, SpaceView, Theme, ViewMode};
use deskspace_platform::{ITEM_ICON_SIZE, ItemMetrics, SpaceStyle, space_dpi};

use crate::scene::space_style;

use super::super::{App, Surface};

/// Where a grid's cell comes from. A space's row is as tall as its items
/// draw, so a highlight that hugs an item leaves no empty band under it; only
/// the desktop layer keeps a cell it does not fill.
pub(in crate::app) enum Cells {
    /// The look's own cell, whole: under Match Explorer, Explorer's item
    /// spacing. The desktop layer's, whose items sit where Explorer put them.
    Look,
    /// The look's column (Explorer's, under Match Explorer) over a row this
    /// tall, in physical pixels: a space on the default view.
    LookColumn(i32),
    /// A fixed view's own cell, in physical pixels.
    FixedView(Size),
}

/// What `base_layout` scales a grid by, and where its cell comes from.
pub(in crate::app) struct LayoutScale {
    /// The icon factor.
    pub(in crate::app) icon: f32,
    /// The title bar's factor: the DPI alone, so a zoom leaves it alone.
    pub(in crate::app) title: f32,
    pub(in crate::app) cells: Cells,
}

impl App {
    /// The theme a space is drawn with: its own look while customized, else
    /// the global theme. A missing space gets the global theme.
    pub(in crate::app) fn theme_for(&self, id: SpaceId) -> &Theme {
        let global = &self.config.theme;
        self.config
            .desktop
            .space(id)
            .map_or(global, |space| space.theme(global))
    }

    /// The unscaled grid a space lays out on, with its own spacing while
    /// customized.
    pub(in crate::app) fn grid_for(&self, id: SpaceId) -> GridSpec {
        let global = self.config.desktop.layout;
        self.config
            .desktop
            .space(id)
            .map_or(global, |space| space.grid(global))
    }

    /// The concrete colors for a space's scene: its theme's palette, with the
    /// selection highlight and border replaced by Windows' accent when the
    /// theme asks for it and an accent is known.
    pub(in crate::app) fn palette_for(&self, id: SpaceId) -> Palette {
        self.surface_palette(Surface::Space(id))
    }

    /// The window style for a space.
    pub(in crate::app) fn style_for(&self, id: SpaceId) -> SpaceStyle {
        let mut style = space_style(self.theme_for(id), self.system.dark());
        if self.debug.as_ref().is_some_and(|tools| tools.hide_shadows) {
            style.shadow = None;
        }
        style
    }

    /// Whether DeskSpace's own chrome (the settings window and menus) is dark.
    /// `Theme.mode` colors the spaces; this follows the global `ui_mode`.
    pub(in crate::app) fn ui_dark(&self) -> bool {
        self.config.theme.ui_mode.resolve(self.system.dark())
    }

    /// The item-icon scale of `theme` as a factor, without per-monitor DPI.
    /// When mirroring Explorer's display, the desktop's own icon size (in
    /// pixels) wins, so spaces are 1:1 with the desktop; otherwise the theme's
    /// icon-size setting applies.
    pub(in crate::app) fn icon_scale(&self, theme: &Theme) -> f32 {
        if theme.match_explorer()
            && let Some(pixels) = self.system.shell().icon_size
        {
            return (pixels as f32 / ITEM_ICON_SIZE as f32).max(0.25);
        }
        theme.icon_size.clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1) as f32 / 100.0
    }

    /// The full icon factor for a space that follows the look's icon size:
    /// the icon-size factor times the monitor's DPI. Explorer's own icon size
    /// is already in physical pixels, so mirroring does not multiply by DPI
    /// again. A view with its own icon size never asks (`metrics_for`).
    pub(in crate::app) fn space_scale(&self, id: SpaceId) -> f32 {
        let theme = self.theme_for(id);
        let scale = self.icon_scale(theme);
        if theme.match_explorer() && self.system.shell().icon_size.is_some() {
            return scale;
        }
        scale * space_dpi(id.0)
    }

    /// `grid` scaled by `scale`, with the title bar scaled by `scale.title`,
    /// and its cell as `scale.cells` says. When `theme` mirrors Explorer, its
    /// item spacing (in physical pixels) replaces the look's cell and gap, so
    /// the columns match the desktop's. A fixed icon view puts in its own
    /// cell. Explorer's cells touch, so neither kind has a gap. The stored
    /// grids are never scaled, so the settings form keeps editing logical
    /// values.
    pub(in crate::app) fn base_layout(
        &self,
        theme: &Theme,
        grid: GridSpec,
        scale: LayoutScale,
    ) -> GridSpec {
        let mut layout = grid.scaled_with_title(scale.icon, scale.title);
        let explorer = theme
            .match_explorer()
            .then(|| self.system.shell().icon_spacing)
            .flatten();
        let look = explorer.unwrap_or(layout.cell());
        let (cell, touching) = match scale.cells {
            Cells::FixedView(cell) => (cell, true),
            Cells::Look => (look, explorer.is_some()),
            Cells::LookColumn(row) => (Size::new(look.width, row), explorer.is_some()),
        };
        layout.cell_width = cell.width.max(1);
        layout.cell_height = cell.height.max(1);
        if touching {
            layout.gap = 0;
        }
        layout
    }

    /// The scaled grid for one space, including its monitor's DPI. Its row is
    /// what its items draw, with the space's label lines and size. A fixed
    /// icon view's column is Explorer's, and it scales the rest of the grid,
    /// its padding and snap, for the DPI alone, so neither the look's icon
    /// size nor its own grows them.
    pub(in crate::app) fn layout_for(&self, id: SpaceId) -> GridSpec {
        let dpi = space_dpi(id.0);
        let theme = self.theme_for(id);
        let row = self
            .metrics_for(id)
            .grid_cell_height(theme.label_lines, theme.label_size);
        let (icon, cells) = match fixed_view_cell_width(self.view_mode(id), dpi) {
            Some(width) => (dpi, Cells::FixedView(Size::new(width, row))),
            None => (self.space_scale(id), Cells::LookColumn(row)),
        };
        self.base_layout(
            theme,
            self.grid_for(id),
            LayoutScale {
                icon,
                title: dpi,
                cells,
            },
        )
    }

    /// The logical global grid (icon size only, no per-monitor DPI), for
    /// callers that have no space at hand: a space's grid under the global
    /// look.
    pub(in crate::app) fn effective_layout(&self) -> GridSpec {
        let theme = &self.config.theme;
        let icon = self.icon_scale(theme);
        let row =
            ItemMetrics::for_scale(icon).grid_cell_height(theme.label_lines, theme.label_size);
        self.base_layout(
            theme,
            self.config.desktop.layout,
            LayoutScale {
                icon,
                title: 1.0,
                cells: Cells::LookColumn(row),
            },
        )
    }

    /// The desktop layer's grid: the global look's own cell, whole. Its items
    /// sit where Explorer put them, on Explorer's step, so its rows are not
    /// tightened to the items as a space's are.
    pub(in crate::app) fn layer_layout(&self) -> GridSpec {
        let theme = &self.config.theme;
        self.base_layout(
            theme,
            self.config.desktop.layout,
            LayoutScale {
                icon: self.icon_scale(theme),
                title: 1.0,
                cells: Cells::Look,
            },
        )
    }

    /// Scaled icon, label, and check-box sizes for one space. A view with its
    /// own icon size (every view but Icons) draws that icon and scales the
    /// rest for the display alone, so the look and a zoom leave it alone.
    pub(in crate::app) fn metrics_for(&self, id: SpaceId) -> ItemMetrics {
        let dpi = space_dpi(id.0);
        match self.view_mode(id).item_icon_at(dpi) {
            Some(icon_size) => ItemMetrics {
                icon_size,
                ..ItemMetrics::for_scale(dpi)
            },
            None => ItemMetrics::for_scale(self.space_scale(id)),
        }
    }

    /// The space's view mode, or the default when it is gone.
    pub(in crate::app) fn view_mode(&self, id: SpaceId) -> ViewMode {
        self.config
            .desktop
            .space(id)
            .map_or(ViewMode::default(), |space| space.view().mode)
    }

    /// Where `id`'s items sit under its current view, with the same metrics and
    /// scale the scene builder uses.
    pub(in crate::app) fn view_layout(&self, id: SpaceId) -> Option<ViewLayout> {
        let space = self.config.desktop.space(id)?;
        let metrics = self.metrics_of(id);
        Some(ViewLayout::for_space(space, metrics).with_grouping(self.shown_grouping(space)))
    }

    /// The cells space `id` draws its view with: its layout, monitor DPI, and
    /// label size.
    pub(in crate::app) fn metrics_of(&self, id: SpaceId) -> ViewMetrics {
        let view = self
            .config
            .desktop
            .space(id)
            .map_or_else(SpaceView::default, |space| space.view().clone());
        ViewMetrics::for_view(
            &view,
            self.layout_for(id),
            space_dpi(id.0),
            self.theme_for(id).label_size,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::fixtures::{apply, create_space};
    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::geometry::Rect;
    use deskspace_core::{Command, Preset};
    use deskspace_platform::ShellDisplay;

    use super::*;

    /// An app with a known Explorer display snapshot. `space_dpi` is `1.0`
    /// outside the UI thread, so the scale assertions are deterministic.
    fn app_with_display(display: ShellDisplay) -> App {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        app.system.set_shell(display);
        app
    }

    #[test]
    fn icon_size_follows_explorer_when_matching() {
        // Default theme: `match_explorer` on, icon size 100.
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(2 * ITEM_ICON_SIZE),
            ..ShellDisplay::default()
        });
        assert_eq!(app.icon_scale(&app.config.theme), 2.0);
        // Explorer's size wins, so the DeskSpace setting is inert.
        app.config.theme.icon_size = 200;
        assert_eq!(app.icon_scale(&app.config.theme), 2.0);
        app.config.theme.icon_size = 50;
        assert_eq!(app.icon_scale(&app.config.theme), 2.0);
    }

    #[test]
    fn icon_size_setting_applies_without_matching() {
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(2 * ITEM_ICON_SIZE),
            ..ShellDisplay::default()
        });
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 150;
        assert_eq!(app.icon_scale(&app.config.theme), 1.5);
        app.config.theme.icon_size = 50;
        assert_eq!(app.icon_scale(&app.config.theme), 0.5);
    }

    #[test]
    fn matching_without_a_reported_size_falls_back_to_the_setting() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.icon_size = 75;
        assert_eq!(app.icon_scale(&app.config.theme), 0.75);
    }

    #[test]
    fn explorer_spacing_replaces_the_cell_when_matching() {
        let app = app_with_display(ShellDisplay {
            icon_spacing: Some(Size::new(120, 130)),
            ..ShellDisplay::default()
        });
        let layout = app.base_layout(
            &app.config.theme,
            app.config.desktop.layout,
            LayoutScale {
                icon: 1.0,
                title: 1.0,
                cells: Cells::Look,
            },
        );
        assert_eq!(
            (layout.cell_width, layout.cell_height, layout.gap),
            (120, 130, 0)
        );
        // A space takes Explorer's column, over a row of its own.
        let layout = app.base_layout(
            &app.config.theme,
            app.config.desktop.layout,
            LayoutScale {
                icon: 1.0,
                title: 1.0,
                cells: Cells::LookColumn(80),
            },
        );
        assert_eq!(
            (layout.cell_width, layout.cell_height, layout.gap),
            (120, 80, 0)
        );
    }

    #[test]
    fn grid_spacing_applies_without_matching() {
        let mut app = app_with_display(ShellDisplay {
            icon_spacing: Some(Size::new(120, 130)),
            ..ShellDisplay::default()
        });
        app.config.theme.preset = Preset::Custom;
        app.config.desktop.layout.gap = 20;
        let layout = app.base_layout(
            &app.config.theme,
            app.config.desktop.layout,
            LayoutScale {
                icon: 1.0,
                title: 1.0,
                cells: Cells::Look,
            },
        );
        assert_eq!(
            (layout.cell_width, layout.cell_height, layout.gap),
            (96, 104, 20)
        );
        // A space keeps the look's column and gap, over a row of its own.
        let layout = app.base_layout(
            &app.config.theme,
            app.config.desktop.layout,
            LayoutScale {
                icon: 1.0,
                title: 1.0,
                cells: Cells::LookColumn(80),
            },
        );
        assert_eq!(
            (layout.cell_width, layout.cell_height, layout.gap),
            (96, 80, 20)
        );
    }

    #[test]
    fn space_layout_and_metrics_follow_the_icon_size() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 200;
        let (layout, metrics) = (app.layout_for(SpaceId(1)), app.metrics_for(SpaceId(1)));
        assert_eq!(metrics.icon_size, 96);
        assert_eq!(layout.cell_width, 192);
        let large = layout.cell_height;
        assert_eq!(large, metrics.grid_cell_height(2, 100));
        app.config.theme.icon_size = 50;
        let (layout, metrics) = (app.layout_for(SpaceId(1)), app.metrics_for(SpaceId(1)));
        assert_eq!(metrics.icon_size, 24);
        assert_eq!(layout.cell_width, 48);
        assert_eq!(layout.cell_height, metrics.grid_cell_height(2, 100));
        assert!(layout.cell_height < large);
    }

    #[test]
    fn a_space_row_is_as_tall_as_its_items_draw_under_every_look() {
        // Explorer's desktop as measured at 100 %: 40 px icons on a 75 x 92
        // step, with a 74 x 62 highlight around a one-line name.
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(40),
            icon_spacing: Some(Size::new(75, 92)),
            ..ShellDisplay::default()
        });
        let id = space_in_view(&mut app, ViewMode::Icons);
        let cell = |app: &App| {
            let layout = app.layout_for(id);
            (layout.cell_width, layout.cell_height, layout.gap)
        };
        // Two label lines: Explorer's column, and a row that holds the icon,
        // both lines, and the gap to the next row, not Explorer's 92 px.
        assert_eq!(cell(&app), (75, 62 + 16 + 1, 0));
        app.config.theme.label_lines = 1;
        assert_eq!(cell(&app), (75, 62 + 1, 0));
        // The Custom look keeps its own column and spacing over the same row.
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 100;
        let metrics = app.metrics_for(id);
        assert_eq!(
            cell(&app),
            (
                96,
                metrics.grid_cell_height(1, 100),
                app.config.desktop.layout.gap
            )
        );
        assert_eq!(
            app.effective_layout().cell_height,
            app.layout_for(id).cell_height
        );
    }

    #[test]
    fn the_desktop_layer_keeps_explorers_own_cell() {
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(40),
            icon_spacing: Some(Size::new(75, 92)),
            ..ShellDisplay::default()
        });
        let id = space_in_view(&mut app, ViewMode::Icons);
        // Its items sit where Explorer put them, 92 px apart.
        assert_eq!(
            app.surface_layout(Surface::Desktop).cell(),
            Size::new(75, 92)
        );
        assert!(app.layout_for(id).cell_height < 92);
    }

    /// A space in `mode`, in `app`'s model.
    fn space_in_view(app: &mut App, mode: ViewMode) -> SpaceId {
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(0, 0, 300, 300));
        apply(
            &mut app.config.desktop,
            Command::SetSpaceView {
                id,
                view: SpaceView::default().with_mode(mode),
            },
        );
        id
    }

    #[test]
    fn a_fixed_icon_view_ignores_the_look_and_explorer() {
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(ITEM_ICON_SIZE),
            icon_spacing: Some(Size::new(120, 130)),
            ..ShellDisplay::default()
        });
        let id = space_in_view(&mut app, ViewMode::LargeIcons);
        // Large icons are 96 px at 96 DPI, in Explorer's 106 px column.
        let fixed = (app.metrics_for(id), app.layout_for(id));
        assert_eq!(fixed.0.icon_size, 96);
        assert_eq!(fixed.1.cell_width, 106);
        // Neither the look's size nor Explorer's spacing overrides a fixed
        // icon view.
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 50;
        assert_eq!((app.metrics_for(id), app.layout_for(id)), fixed);
    }

    #[test]
    fn a_fixed_view_cell_is_explorers_column_over_what_the_item_draws() {
        let mut app = app_with_display(ShellDisplay::default());
        let id = space_in_view(&mut app, ViewMode::ExtraLargeIcons);
        let metrics = app.metrics_for(id);
        let layout = app.layout_for(id);
        let theme = app.theme_for(id);
        assert_eq!(metrics.icon_size, 256);
        assert_eq!(layout.cell_width, 272);
        assert_eq!(
            layout.cell_height,
            metrics.grid_cell_height(theme.label_lines, theme.label_size)
        );
        assert_eq!(layout.gap, 0, "Explorer's cells touch");
        // The rest of the item, and the grid around it, scale for the display
        // alone, not by the 256 px icon over the 48 px base.
        assert_eq!(
            metrics,
            ItemMetrics {
                icon_size: 256,
                ..ItemMetrics::for_scale(1.0)
            }
        );
        assert_eq!(layout.padding, app.config.desktop.layout.padding);
        assert_eq!(layout.snap, app.config.desktop.layout.snap);
    }

    #[test]
    fn a_looks_spacing_changes_settings_icon_size_and_no_fixed_view() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        let follows = space_in_view(&mut app, ViewMode::Icons);
        let fixed = space_in_view(&mut app, ViewMode::MediumIcons);
        let before = (app.layout_for(follows), app.layout_for(fixed));
        app.config.desktop.layout.cell_width += 40;
        app.config.desktop.layout.cell_height += 40;
        app.config.desktop.layout.gap += 4;
        assert_ne!(app.layout_for(follows), before.0);
        assert_eq!(app.layout_for(fixed), before.1);
    }

    #[test]
    fn a_fixed_view_row_holds_the_label_lines_the_look_asks_for() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        let id = space_in_view(&mut app, ViewMode::MediumIcons);
        app.config.theme.label_lines = 2;
        let two = app.layout_for(id).cell_height;
        app.config.theme.label_lines = 1;
        let one = app.layout_for(id).cell_height;
        assert!(one < two, "{one} < {two}");
        app.config.theme.label_size = 150;
        assert!(app.layout_for(id).cell_height > one, "a larger label");
        assert_eq!(
            app.layout_for(id).cell_width,
            75,
            "the width stays Explorer's"
        );
    }

    #[test]
    fn a_look_zoom_does_not_move_a_fixed_view_space() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 100;
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(0, 0, 300, 300));
        apply(
            &mut app.config.desktop,
            Command::SetSpaceView {
                id,
                view: SpaceView::default().with_mode(ViewMode::LargeIcons),
            },
        );
        let before = (app.metrics_for(id), app.layout_for(id));
        app.config.theme.icon_size = 150;
        assert_eq!((app.metrics_for(id), app.layout_for(id)), before);
    }

    #[test]
    fn a_matching_space_draws_explorer_icon_metrics() {
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(ITEM_ICON_SIZE),
            ..ShellDisplay::default()
        });
        app.config.theme.icon_size = 200;
        assert_eq!(app.metrics_for(SpaceId(1)).icon_size, ITEM_ICON_SIZE);
        assert_eq!(app.layout_for(SpaceId(1)).cell_width, 96);
    }

    #[test]
    fn effective_layout_uses_the_global_icon_size() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 200;
        assert_eq!(app.effective_layout().cell_width, 192);
    }

    #[test]
    fn a_zoom_leaves_the_title_bar_alone() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 200;
        assert_eq!(
            app.effective_layout().title_height,
            app.config.desktop.layout.title_height
        );
    }
}
