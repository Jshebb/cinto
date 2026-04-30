mod adapter;
mod config;
mod harmony;
mod model;
mod session;
mod theme;
mod ui;
mod uninstall;
mod workspace;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{config::Config, session::AgentSession, ui::App};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "A local terminal coding-agent harness for OpenAI-compatible model servers"
)]
struct Args {
    #[arg(short, long)]
    config: Option<PathBuf>,

    #[arg(long, help = "Render the current empty Harmony prompt and exit")]
    print_prompt: bool,

    #[arg(
        long,
        help = "Skip the first-run setup screen even when no config exists"
    )]
    skip_setup: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Open the initial setup TUI")]
    Setup,
    #[command(about = "Remove the installed cinto binary")]
    Uninstall {
        #[arg(long, help = "Remove ~/.config/cinto after removing the binary")]
        purge_config: bool,

        #[arg(long, help = "Do not ask for confirmation")]
        yes: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(Command::Uninstall { purge_config, yes }) = args.command {
        uninstall::run(purge_config, yes)?;
        return Ok(());
    }

    let config_path = args.config.clone().or_else(Config::default_path);
    let setup_requested = matches!(args.command, Some(Command::Setup));
    let first_run = setup_requested
        || (!args.skip_setup && config_path.as_ref().is_some_and(|path| !path.exists()));
    let config = Config::load(args.config)?;
    let session = AgentSession::new(config);

    if args.print_prompt {
        println!("{}", session.render_prompt());
        return Ok(());
    }

    let mut app = App::new(session, config_path, first_run);
    app.run().await
}
