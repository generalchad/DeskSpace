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

    #[arg(long, value_enum, default_value_t = PlacementArg::TopLevel)]
    pub placement: PlacementArg,

    #[arg(long, help = "Overlay width in physical pixels")]
    pub width: Option<i32>,

    #[arg(long, help = "Overlay height in physical pixels")]
    pub height: Option<i32>,

    #[arg(long, help = "Ignore the saved configuration and start fresh")]
    pub reset: bool,

    #[arg(long, help = "Start with the overlay hidden")]
    pub hidden: bool,

    #[arg(long, help = "Do not populate spaces from the desktop folder")]
    pub no_icons: bool,

    #[arg(
        long,
        help = "Make the whole overlay click-through (display only, no interaction)"
    )]
    pub click_through: bool,

    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

impl Cli {
    pub fn orientation(&self) -> Placement {
        self.placement.into()
    }
}
