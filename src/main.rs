mod config;
mod harmony;
mod model;
mod session;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use crate::{config::Config, session::AgentSession, ui::App};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "A Harmony-based local coding harness for gpt-oss models"
)]
struct Args {
    #[arg(short, long)]
    config: Option<PathBuf>,

    #[arg(long, help = "Render the current empty Harmony prompt and exit")]
    print_prompt: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config_path = args.config.clone().or_else(Config::default_path);
    let config = Config::load(args.config)?;
    let session = AgentSession::new(config);

    if args.print_prompt {
        println!("{}", session.render_prompt());
        return Ok(());
    }

    let mut app = App::new(session, config_path);
    app.run().await
}
