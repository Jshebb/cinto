mod adapter;
mod batch;
mod config;
pub mod crp;
mod eval_diff;
mod harmony;
mod init;
mod kernel;
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
use crate::config as config_mod;

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
    #[command(about = "Apply a built-in preset to the saved config without opening the TUI")]
    UsePreset {
        #[arg(help = "Preset name (e.g. lm-studio-small, ollama, vllm, minimal)")]
        preset: String,
        #[arg(long, help = "Override the model name from the preset")]
        model: Option<String>,
        #[arg(long, help = "Override the context window size in tokens")]
        context: Option<u32>,
        #[arg(long, help = "Override max output tokens")]
        max_tokens: Option<u32>,
    },
    #[command(about = "List available built-in presets")]
    Presets,
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
        #[arg(long, help = "Run tasks through the kernel pipeline instead of the conversational agent")]
        kernel: bool,
        #[arg(long, help = "Context pack budget in chars for kernel mode (default 16000, reduce for small models)")]
        kernel_budget: Option<usize>,
        #[arg(long, help = "Save full stage traces (prompt + response) to this directory for fine-tuning dataset generation")]
        save_traces: Option<PathBuf>,
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
    #[command(about = "Index the workspace and write .cinto/project_map.json and symbol_index.json")]
    Index,
    #[command(about = "Search the workspace (kernel syscall, scoped output)")]
    Search {
        #[arg(help = "Query string (smart-case)")]
        query: String,
        #[arg(long, help = "Restrict to files matching a glob, e.g. '*.rs'")]
        glob: Option<String>,
    },
    #[command(about = "Read a line range from a workspace file")]
    ReadRange {
        #[arg(help = "Relative file path")]
        file: String,
        #[arg(help = "Start line (1-indexed)")]
        start: usize,
        #[arg(help = "End line (inclusive)")]
        end: usize,
    },
    #[command(about = "Read lines around a query match in a workspace file")]
    ReadAround {
        #[arg(help = "Relative file path")]
        file: String,
        #[arg(help = "Search query")]
        query: String,
        #[arg(long, default_value = "5")]
        before: usize,
        #[arg(long, default_value = "5")]
        after: usize,
    },
    #[command(about = "List symbols defined in a workspace file")]
    ListSymbols {
        #[arg(help = "Relative file path")]
        file: String,
    },
    #[command(about = "Build and print a context pack for a task (kernel preview)")]
    Pack {
        #[arg(help = "Task description")]
        task: String,
        #[arg(long = "file", help = "Hint: include symbols for this file (repeatable)")]
        files: Vec<String>,
        #[arg(long = "search", help = "Hint: run this search and include results (repeatable)")]
        search_terms: Vec<String>,
        #[arg(long, help = "Budget in chars (default 16000)")]
        budget: Option<usize>,
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
        kernel,
        kernel_budget,
        save_traces,
    }) = args.command
    {
        if kernel {
            batch::run_kernel(config, tasks, output, dry_run, kernel_budget, save_traces).await?;
        } else {
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
        }
        return Ok(());
    }

    if let Some(Command::EvalDiff { base, compare }) = args.command {
        eval_diff::run(base, compare)?;
        return Ok(());
    }

    if let Some(Command::Presets) = args.command {
        for preset in config_mod::PRESETS {
            println!("{:<20} {}", preset.name, preset.description);
        }
        return Ok(());
    }

    if let Some(Command::UsePreset { preset: preset_name, model: model_override, context, max_tokens }) = args.command {
        let preset = config_mod::preset_by_name(&preset_name)
            .ok_or_else(|| anyhow::anyhow!(
                "unknown preset '{}'. Run `cinto presets` to list available presets.",
                preset_name
            ))?;
        let mut new_config = preset.to_config()?;
        new_config.harness.workspace = config.harness.workspace.clone();
        if let Some(m) = model_override {
            new_config.model.model = m;
        }
        if let Some(c) = context {
            new_config.model.context_window = c;
        }
        if let Some(t) = max_tokens {
            new_config.model.max_tokens = t;
        }
        let path = new_config.save(config_path)?;
        println!("Preset '{}' written to {}", preset_name, path.display());
        println!("  endpoint : {}", new_config.model.endpoint);
        println!("  model    : {}", new_config.model.model);
        println!("  format   : {}", new_config.model.format);
        println!("  context  : {} tokens", new_config.model.context_window);
        println!("  max_tok  : {}", new_config.model.max_tokens);
        return Ok(());
    }

    if let Some(Command::Init) = args.command {
        let config = Config::load(args.config)?;
        init::run(&config)?;
        return Ok(());
    }

    if let Some(Command::Search { query, glob }) = args.command {
        let config = Config::load(args.config)?;
        let workspace = &config.harness.workspace;
        let mut params = kernel::search::SearchParams::new(query);
        params.file_glob = glob;
        let result = kernel::search::search(workspace, params)?;
        print!("{}", result.formatted);
        return Ok(());
    }

    if let Some(Command::Pack { task, files, search_terms, budget }) = args.command {
        let config = Config::load(args.config)?;
        let workspace = &config.harness.workspace;
        let mut builder = kernel::context_pack::ContextPackBuilder::new(workspace)
            .with_index();
        if let Some(b) = budget {
            builder = builder.with_budget(b);
        }
        let hints = kernel::context_pack::ContextHints {
            files,
            search_terms,
            ranges: vec![],
        };
        let pack = builder.build(&task, &hints)?;
        print!("{}", pack.formatted);
        return Ok(());
    }

    if let Some(Command::ReadRange { file, start, end }) = args.command {
        let config = Config::load(args.config)?;
        let workspace = &config.harness.workspace;
        let out = kernel::read::read_range(workspace, &file, start, end)?;
        print!("{out}");
        return Ok(());
    }

    if let Some(Command::ReadAround { file, query, before, after }) = args.command {
        let config = Config::load(args.config)?;
        let workspace = &config.harness.workspace;
        let params = kernel::read::AroundParams { before, after };
        let out = kernel::read::read_around(workspace, &file, &query, params)?;
        print!("{out}");
        return Ok(());
    }

    if let Some(Command::ListSymbols { file }) = args.command {
        let config = Config::load(args.config)?;
        let workspace = &config.harness.workspace;
        let out = kernel::read::list_symbols(workspace, &file)?;
        print!("{out}");
        return Ok(());
    }

    if let Some(Command::Index) = args.command {
        let config = Config::load(args.config)?;
        let workspace = &config.harness.workspace;
        let result = kernel::index::index_repo(workspace)?;
        kernel::index::save(workspace, &result)?;
        println!("Indexed workspace: {}", result.project_map.root);
        println!("  Files:   {}", result.file_count);
        println!("  Symbols: {}", result.symbol_count);
        println!(
            "  Written: {}",
            workspace.join(".cinto/project_map.json").display()
        );
        println!(
            "           {}",
            workspace.join(".cinto/symbol_index.json").display()
        );
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
