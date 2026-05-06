mod adapter;
mod batch;
mod config;
pub mod crp;
mod eval_diff;
mod harmony;
mod init;
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

    #[arg(long, help = "Render the current empty model prompt and exit")]
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
    #[command(about = "Initialize .cinto/templates in the current workspace")]
    Init,
    #[command(about = "Remove the installed cinto binary")]
    Uninstall {
        #[arg(long, help = "Remove ~/.config/cinto after removing the binary")]
        purge_config: bool,

        #[arg(long, help = "Do not ask for confirmation")]
        yes: bool,
    },
    #[command(about = "Run tasks headlessly to generate synthetic CRP datasets")]
    Batch {
        #[arg(long, help = "Path to the input JSONL tasks file")]
        tasks: PathBuf,
        #[arg(long, help = "Path to the output JSONL file for valid traces")]
        output: PathBuf,
        #[arg(
            long,
            help = "Optional LLM endpoint to use as a semantic evaluator (e.g., https://api.deepseek.com/v1)"
        )]
        evaluator_endpoint: Option<String>,
        #[arg(
            long,
            help = "Optional LLM model name for the evaluator (e.g., deepseek-chat)"
        )]
        evaluator_model: Option<String>,
        #[arg(long, help = "Optional API key for the evaluator model")]
        evaluator_api_key: Option<String>,
        #[arg(
            long,
            help = "Validate the JSONL and fixtures without calling the model"
        )]
        dry_run: bool,
        #[arg(
            long,
            help = "DANGEROUS: Automatically approve all tool executions (including shell commands). Only use in an isolated VM or Docker container!"
        )]
        dangerously_auto_approve: bool,
    },
    #[command(
        about = "Compare two batch evaluation JSONL runs and highlight regressions/improvements"
    )]
    EvalDiff {
        #[arg(help = "Path to the base (control) JSONL file")]
        base: PathBuf,
        #[arg(help = "Path to the compare (experiment) JSONL file")]
        compare: PathBuf,
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
    let config = Config::load(args.config.clone())?;

    if let Some(Command::Batch {
        tasks,
        output,
        evaluator_endpoint,
        evaluator_model,
        evaluator_api_key,
        dry_run,
        dangerously_auto_approve,
    }) = args.command
    {
        batch::run(
            config,
            tasks,
            output,
            evaluator_endpoint,
            evaluator_model,
            evaluator_api_key,
            dry_run,
            dangerously_auto_approve,
        )
        .await?;
        return Ok(());
    }

    if let Some(Command::EvalDiff { base, compare }) = args.command {
        eval_diff::run(base, compare)?;
        return Ok(());
    }

    if let Some(Command::Init) = args.command {
        let config = Config::load(args.config)?;
        init::run(&config)?;
        return Ok(());
    }

    let setup_requested = matches!(args.command, Some(Command::Setup));
    let first_run = setup_requested
        || (!args.skip_setup && config_path.as_ref().is_some_and(|path| !path.exists()));
    let session = AgentSession::new(config);

    if args.print_prompt {
        println!("{}", session.render_prompt());
        return Ok(());
    }

    let mut app = App::new(session, config_path, first_run);
    app.run().await
}
