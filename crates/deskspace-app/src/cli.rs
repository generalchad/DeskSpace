use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use deskspace_platform::Placement;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PlacementArg {
    TopLevel,
    DesktopBackground,
    DesktopForeground,
}

impl From<PlacementArg> for Placement {
    fn from(value: PlacementArg) -> Self {
        match value {
            PlacementArg::TopLevel => Placement::TopLevel,
            PlacementArg::DesktopBackground => Placement::DesktopBackground,
            PlacementArg::DesktopForeground => Placement::DesktopForeground,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "deskspace",
    version,
    about = "A free and open-source desktop space organizer for Windows"
)]
pub struct Cli {
    #[arg(
        long,
        value_name = "FILE",
        help = "Path to an explicit configuration file"
    )]
    pub config: Option<PathBuf>,

    #[arg(
        long,
        value_enum,
        help = "Where space windows live; defaults to top-level (desktop-hosted \
                spaces are opaque, with no blur)"
    )]
    pub placement: Option<PlacementArg>,

    #[arg(long, help = "Ignore the saved configuration and start fresh")]
    pub reset: bool,

    #[arg(long, help = "Start with spaces hidden")]
    pub hidden: bool,

    #[arg(
        long,
        help = "Show spaces: signal a running DeskSpace, or start with them shown"
    )]
    pub show: bool,

    #[arg(long, help = "Open the settings window on start")]
    pub settings: bool,

    #[arg(long, help = "Do not populate spaces from the desktop folder")]
    pub no_icons: bool,

    #[arg(
        long,
        help = "Make spaces click-through (display only, no interaction)"
    )]
    pub click_through: bool,

    /// Development aid: keep spaces above other windows so their appearance
    /// can be checked without uncovering the desktop.
    #[arg(long, hide = true)]
    pub on_top: bool,

    /// Development aid: open the debug window, with a live log console and
    /// test tools (ROADMAP D14).
    #[arg(long, hide = true)]
    pub debug: bool,

    /// Development aid: draw the loose desktop items on a chrome-less layer
    /// window (M2 takeover). Additive: Explorer still draws its own icons.
    #[arg(long, hide = true)]
    pub takeover_layer: bool,

    /// Development aid: take over the desktop icon layer, hiding Explorer's
    /// icons and drawing the loose items on DeskSpace's own layer (M2
    /// takeover).
    #[arg(long, hide = true)]
    pub takeover: bool,

    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

impl Cli {
    /// The placement from `--placement`, if the user chose one.
    pub fn placement(&self) -> Option<Placement> {
        self.placement.map(Placement::from)
    }
}
