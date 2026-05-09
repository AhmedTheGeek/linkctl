mod app;
mod camera;
mod input;
mod profiles;
mod ui;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "linkctl",
    version,
    about = "Terminal controller for the Insta360 Link webcam (V4L2 PTZ)"
)]
struct Cli {
    /// V4L2 device path
    #[arg(short, long, default_value = "/dev/video0")]
    device: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    app::run(cli.device)
}
