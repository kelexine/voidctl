// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: CLI entry point and Clap subcommand routing for voidctl

use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;
use std::process::ExitCode;
use voidctl::clean::{CleanCategory, interactive_select_and_clean, scan_hygiene};
use voidctl::config::{load_config, resolve_home_dir, save_config};
use voidctl::drift::{audit_drift, verify_symlinks};
use voidctl::jump::{add_alias, execute_jump, generate_init_script, list_aliases};
use voidctl::report::{print_clean_report, print_drift_report, print_symlink_records};
use voidctl::runner::{add_command, execute_command, list_commands, resolve_command};
use voidctl::search::{SearchOptions, SearchType, execute_search};

#[derive(Parser)]
#[command(
    name = "voidctl",
    author = "kelexine <https://github.com/kelexine>",
    version,
    about = "Unified personal machine operations CLI for TheVoid"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Navigate to registered project directories
    Jump(JumpArgs),
    /// Execute saved command sequences for registered projects
    Run(RunArgs),
    /// System hygiene scanner and interactive cleaner
    Clean(CleanArgs),
    /// Dotfiles symlink integrity and git repository drift
    Drift(DriftArgs),
    /// Search for files or directories by glob pattern
    Search(SearchArgs),
    /// Generate shell completion scripts (bash, zsh, fish)
    Completions {
        /// Target shell for autocompletion
        shell: clap_complete::Shell,
    },
}

#[derive(Args)]
struct JumpArgs {
    /// Alias to resolve
    alias: Option<String>,
    /// Register a new jump alias: --add <alias> <path>
    #[arg(long, num_args = 2, value_names = ["ALIAS", "PATH"])]
    add: Option<Vec<String>>,
    /// List all registered jump aliases
    #[arg(long)]
    list: bool,
    /// Generate shell wrapper script and completion (zsh, bash, fish)
    #[arg(long, value_name = "SHELL")]
    init: Option<String>,
}

#[derive(Args)]
struct RunArgs {
    /// Project alias
    alias: Option<String>,
    /// Specific command to execute (defaults if single command or 'default'/'run' exists)
    cmd_name: Option<String>,
    /// Register a command: --add <alias> <cmd-name> <shell-command>
    #[arg(long, num_args = 3, value_names = ["ALIAS", "CMD", "COMMAND"])]
    add: Option<Vec<String>>,
    /// List all registered commands for a project: --list <alias>
    #[arg(long, value_name = "ALIAS")]
    list: Option<String>,
}

#[derive(Args)]
struct CleanArgs {
    #[command(subcommand)]
    command: CleanCommands,
}

#[derive(Subcommand)]
enum CleanCommands {
    /// Scan and report reclaimable space across the machine
    Scan {
        /// Display all targets without truncation
        #[arg(short, long)]
        all: bool,
    },
    /// Interactively select and delete cleanable targets
    Select {
        /// Filter by specific category (e.g. 'package-cache', 'logs-cache', 'thumbnails')
        #[arg(short, long)]
        category: Option<String>,
    },
}

#[derive(Args)]
struct DriftArgs {
    #[command(subcommand)]
    command: DriftCommands,
}

#[derive(Subcommand)]
enum DriftCommands {
    /// Audit both symlinks and git repository status
    Status,
    /// Validate symlink integrity and mode bits
    Links,
}

#[derive(Args)]
struct SearchArgs {
    /// Glob pattern to match against entry names (e.g. '*.log', 'config*')
    pattern: String,
    /// Entry type to match: file (f), dir (d), any (a) [default: any]
    #[arg(short = 't', long = "type", value_name = "TYPE", default_value = "any")]
    entry_type: String,
    /// Root directory to search from [default: $HOME]
    #[arg(short, long, value_name = "PATH")]
    root: Option<PathBuf>,
    /// Case-insensitive matching
    #[arg(short = 'i', long)]
    case_insensitive: bool,
    /// Maximum results to display (0 = unlimited)
    #[arg(short = 'n', long, default_value = "50")]
    limit: usize,
    /// Follow symbolic links during traversal
    #[arg(long)]
    follow_links: bool,
    /// Include hidden files and directories
    #[arg(short = 'H', long)]
    hidden: bool,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match run_cli(cli) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("Error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_cli(cli: Cli) -> Result<u8> {
    match cli.command {
        Commands::Jump(args) => handle_jump(args).map(|_| 0),
        Commands::Run(args) => handle_run(args),
        Commands::Clean(args) => handle_clean(args).map(|_| 0),
        Commands::Drift(args) => handle_drift(args).map(|_| 0),
        Commands::Search(args) => handle_search(args).map(|_| 0),
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "voidctl", &mut std::io::stdout());
            Ok(0)
        }
    }
}


fn handle_jump(args: JumpArgs) -> Result<()> {
    if let Some(shell) = args.init {
        let script = generate_init_script(&shell)?;
        print!("{script}");
        return Ok(());
    }

    let mut config = load_config().context("Failed to load configuration")?;

    if let Some(pair) = args.add {
        let alias = pair[0].clone();
        let path = PathBuf::from(&pair[1]);
        add_alias(&mut config, alias.clone(), path.clone());
        save_config(&config).context("Failed to save configuration")?;
        eprintln!("Registered jump alias '{}' -> '{}'", alias, path.display());
        return Ok(());
    }

    if args.list {
        for (alias, path) in list_aliases(&config) {
            println!("{:<16} {}", alias, path.display());
        }
        return Ok(());
    }

    if let Some(alias) = args.alias {
        let path = execute_jump(&config, &alias)?;
        // Pure stdout for shell wrapper `j` (e.g. cd "$target")
        println!("{}", path.display());
        return Ok(());
    }

    bail!("No jump action specified. Use 'voidctl jump <alias>', '--init', '--add', or '--list'.");
}

fn handle_run(args: RunArgs) -> Result<u8> {
    let mut config = load_config().context("Failed to load configuration")?;

    if let Some(triple) = args.add {
        let alias = &triple[0];
        let name = &triple[1];
        let cmd = &triple[2];
        add_command(&mut config, alias, name, cmd);
        save_config(&config).context("Failed to save configuration")?;
        eprintln!("Registered command '{name}' under alias '{alias}': {cmd}");
        return Ok(0);
    }

    if let Some(alias) = args.list {
        return print_project_commands(&config, &alias);
    }

    if let Some(alias) = args.alias {
        let (dir, cmd_str) = resolve_command(&config, &alias, args.cmd_name.as_deref())?;
        eprintln!("Executing in {}: {}", dir.display(), cmd_str);
        let code = execute_command(&dir, &cmd_str)?;
        return Ok(code as u8);
    }

    bail!("No run action specified. Use 'voidctl run <alias> [cmd]', '--add', or '--list'.");
}

fn print_project_commands(config: &voidctl::config::Config, alias: &str) -> Result<u8> {
    let cmds = list_commands(config, alias)
        .ok_or_else(|| anyhow::anyhow!("No commands found for alias '{alias}'"))?;
    for (name, command) in cmds {
        println!("{:<16} {}", name, command);
    }
    Ok(0)
}

fn handle_clean(args: CleanArgs) -> Result<()> {
    let config = load_config().context("Failed to load configuration")?;

    match args.command {
        CleanCommands::Scan { all } => {
            let report = scan_hygiene(&config.clean);
            print_clean_report(&report, all);
        }
        CleanCommands::Select { category } => {
            let cat = match category {
                Some(s) => match s.parse::<CleanCategory>() {
                    Ok(c) => Some(c),
                    Err(e) => {
                        eprintln!("{}", e.red());
                        let valid: Vec<String> =
                            CleanCategory::all().iter().map(|c| c.to_string()).collect();
                        eprintln!("Valid categories: {}", valid.join(", "));
                        return Ok(());
                    }
                },
                None => None,
            };
            let report = scan_hygiene(&config.clean);
            interactive_select_and_clean(&report, cat)?;
        }
    }
    Ok(())
}

fn handle_drift(args: DriftArgs) -> Result<()> {
    let config = load_config().context("Failed to load configuration")?;
    let home = std::env::var("HOME").context("Could not determine HOME directory")?;
    let home_dir = PathBuf::from(home);

    match args.command {
        DriftCommands::Status => {
            let report = audit_drift(&config.drift.dotfiles_dir, &home_dir, &config.drift.links);
            print_drift_report(&report);
        }
        DriftCommands::Links => {
            let records =
                verify_symlinks(&config.drift.dotfiles_dir, &home_dir, &config.drift.links);
            print_symlink_records(&records);
        }
    }
    Ok(())
}

fn handle_search(args: SearchArgs) -> Result<()> {
    let entry_type: SearchType = args.entry_type.parse().map_err(|e: String| anyhow::anyhow!(e))?;

    let root = match args.root {
        Some(p) => p,
        None => resolve_home_dir()
            .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))),
    };

    let opts = SearchOptions {
        pattern: args.pattern,
        entry_type,
        root,
        case_insensitive: args.case_insensitive,
        limit: args.limit,
        follow_links: args.follow_links,
        hidden: args.hidden,
    };

    execute_search(&opts)?;
    Ok(())
}
