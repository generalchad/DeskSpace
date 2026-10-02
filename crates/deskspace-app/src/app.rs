use std::path::PathBuf;

use deskspace_config::{Config, ConfigStore};
use deskspace_core::geometry::Point;
use deskspace_core::{Command, Desktop, spaceId, Rect};
use deskspace_platform::{
    DesktopWindow, EventHandler, Hotkey, IconCache, MouseButton, Placement, WindowConfig,
    WindowContext, WindowEvent, hotkey_id, parse_hotkey, register_hotkey, unregister_hotkey,
};

use crate::bootstrap::seed_default_desktop;
use crate::cli::Cli;
use crate::scene::{SceneRequest, build_scene};

const DEFAULT_WIDTH: i32 = 1280;
const DEFAULT_HEIGHT: i32 = 800;

enum Drag {
    None,
    space { id: spaceId, grab: Point },
}

pub struct App {
    config: Config,
    store: ConfigStore,
    icons: IconCache,
    width: i32,
    height: i32,
    selected: Option<spaceId>,
    status: Option<String>,
    drag: Drag,
    visible: bool,
    hotkey: Option<Hotkey>,
}

impl App {
    pub fn new(config: Config, store: ConfigStore, hidden: bool) -> Self {
        let status = Some(compose_status(&config.desktop));
        Self {
            config,
            store,
            icons: IconCache::new(32),
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            selected: None,
            status,
            drag: Drag::None,
            visible: !hidden,
            hotkey: None,
        }
    }

    fn bounds(&self) -> Rect {
        let width = if self.width > 0 { self.width } else { 1920 };
        let height = if self.height > 0 { self.height } else { 1080 };
        Rect::new(0, 0, width, height)
    }

    fn dispatch(&mut self, command: Command) -> bool {
        let outcome = self.config.desktop.apply(command, self.bounds());
        if outcome.is_applied() {
            self.status = Some(compose_status(&self.config.desktop));
        }
        outcome.is_applied()
    }

    fn begin_interaction(&mut self, point: Point) {
        let Some(id) = self.config.desktop.space_at(point) else {
            self.selected = None;
            return;
        };
        self.selected = Some(id);

        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        if space.locked || !space.title_bar().contains(point) {
            return;
        }
        self.drag = Drag::space {
            id,
            grab: Point::new(point.x - space.rect.x, point.y - space.rect.y),
        };
    }

    fn drag_to(&mut self, point: Point) -> bool {
        let Drag::space { id, grab } = self.drag else {
            return false;
        };
        let Some(space) = self.config.desktop.space(id) else {
            self.drag = Drag::None;
            return false;
        };
        let rect = Rect::new(
            point.x - grab.x,
            point.y - grab.y,
            space.rect.width,
            space.rect.height,
        );
        self.dispatch(Command::SetspaceRect { id, rect })
    }

    fn save(&self) {
        if let Err(error) = self.store.save(&self.config) {
            tracing::error!(%error, "failed to persist configuration");
        } else {
            tracing::debug!(path = %self.store.path().display(), "configuration saved");
        }
    }
}

impl EventHandler for App {
    fn on_event(&mut self, event: WindowEvent, ctx: &mut WindowContext<'_>) {
        match event {
            WindowEvent::Created { width, height, dpi } => {
                self.width = width;
                self.height = height;
                tracing::info!(width, height, dpi, "overlay created");
                if let Some(spec) = parse_hotkey(&self.config.window.toggle_hotkey) {
                    match register_hotkey(ctx.hwnd(), hotkey_id(0), spec) {
                        Ok(hotkey) => self.hotkey = Some(hotkey),
                        Err(error) => {
                            tracing::warn!(%error, "could not register toggle hotkey")
                        }
                    }
                }
                ctx.request_repaint();
            }
            WindowEvent::Resized { width, height } => {
                self.width = width;
                self.height = height;
                ctx.request_repaint();
            }
            WindowEvent::Paint => {
                let request = SceneRequest {
                    desktop: &self.config.desktop,
                    theme: &self.config.theme,
                    width: self.width,
                    height: self.height,
                    selected: self.selected,
                    status: self.status.clone(),
                };
                let scene = build_scene(&request, &mut self.icons);
                if let Err(error) = ctx.paint(&scene) {
                    tracing::error!(%error, "paint failed");
                }
            }
            WindowEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                self.begin_interaction(Point::new(x, y));
                ctx.request_repaint();
            }
            WindowEvent::MouseMove { x, y } => {
                if self.drag_to(Point::new(x, y)) {
                    ctx.request_repaint();
                }
            }
            WindowEvent::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if !matches!(self.drag, Drag::None) {
                    self.drag = Drag::None;
                    self.save();
                }
            }
            WindowEvent::DoubleClick { x, y } => {
                if let Some(id) = self.config.desktop.space_at(Point::new(x, y)) {
                    self.selected = Some(id);
                    self.dispatch(Command::ToggleRollup { id });
                    ctx.request_repaint();
                }
            }
            WindowEvent::Hotkey { id } if id == hotkey_id(0) => {
                self.visible = !self.visible;
                ctx.show(self.visible);
                tracing::info!(visible = self.visible, "toggled overlay visibility");
            }
            WindowEvent::CloseRequested => {
                if let Some(hotkey) = self.hotkey.take() {
                    unregister_hotkey(ctx.hwnd(), hotkey);
                }
                self.save();
            }
            _ => {}
        }
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    deskspace_platform::enable_per_monitor_awareness();

    let store = match &cli.config {
        Some(path) => ConfigStore::with_path(PathBuf::from(path)),
        None => ConfigStore::discover()?,
    };

    let mut config = if cli.reset {
        Config::new()
    } else {
        store.load_or_default()
    };

    if config.desktop.spaces.is_empty() {
        seed_default_desktop(&mut config.desktop, !cli.no_icons);
    }

    let window_config = WindowConfig {
        title: "DeskSpace".to_string(),
        x: 60,
        y: 60,
        width: cli.width.unwrap_or(DEFAULT_WIDTH),
        height: cli.height.unwrap_or(DEFAULT_HEIGHT),
        placement: cli.orientation(),
        click_through: cli.click_through,
        color_key: config.theme.color_key,
        visible: !cli.hidden,
    };

    let app = App::new(config, store, cli.hidden);
    tracing::info!(
        path = %app.store.path().display(),
        "starting DeskSpace"
    );

    let window = DesktopWindow::create(window_config, Box::new(app))?;
    window.run()?;
    Ok(())
}

fn compose_status(desktop: &Desktop) -> String {
    let spaces = desktop.spaces.len();
    let items = desktop.item_count();
    format!(
        "DeskSpace  |  {} space(s), {} item(s)  |  drag title bar to move, double-click to roll up",
        spaces, items
    )
}

#[allow(dead_code)]
fn placement_name(placement: Placement) -> &'static str {
    match placement {
        Placement::TopLevel => "top-level",
        Placement::DesktopBackground => "desktop-background",
        Placement::DesktopForeground => "desktop-foreground",
    }
}
