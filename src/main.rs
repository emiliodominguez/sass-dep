//! sass-dep - SCSS dependency graph analyzer
//!
//! This is the main entry point for the CLI application. It maps command
//! outcomes to the documented exit codes:
//!
//! | Code | Meaning                              |
//! | ---- | ------------------------------------ |
//! | 0    | Success                              |
//! | 1    | `check` found violations             |
//! | 2    | Invalid arguments or configuration   |
//! | 3    | File not found or IO error           |
//! | 4    | Parse error                          |

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use sass_dep::cli::{Cli, Commands};
use sass_dep::commands::{self, AnalyzeOptions, CheckOptions, CheckSettings, GlobalOptions};

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli) {
        Ok(code) => ExitCode::from(code),
        // `sass-dep analyze | head`: the reader went away, nothing to report.
        Err(error) if commands::is_broken_pipe(&error) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {}", commands::display_safe(&format!("{error:#}")));
            ExitCode::from(commands::exit_code(&error) as u8)
        }
    }
}

/// Dispatches the parsed command and returns the process exit code.
fn run(cli: Cli) -> Result<u8> {
    let global = GlobalOptions {
        root: cli.root,
        config: cli.config,
        load_paths: cli.load_paths,
        quiet: cli.quiet,
        verbose: cli.verbose,
    };

    match cli.command {
        Commands::Analyze {
            graph,
            output,
            format,
            web,
            port,
            no_open,
        } => {
            commands::analyze(
                &global,
                AnalyzeOptions {
                    graph,
                    output,
                    format,
                    web,
                    port,
                    open_browser: !no_open,
                },
            )?;
            Ok(0)
        }
        Commands::Check {
            graph,
            no_cycles,
            max_depth,
            max_fan_out,
            max_fan_in,
            no_orphans,
            no_unresolved,
            no_legacy_imports,
            format,
        } => {
            let violations = commands::check(
                &global,
                CheckOptions {
                    graph,
                    settings: CheckSettings {
                        no_cycles,
                        max_depth,
                        max_fan_out,
                        max_fan_in,
                        no_orphans,
                        no_unresolved,
                        no_legacy_imports,
                    },
                    format,
                },
            )?;
            Ok(u8::from(!violations.is_empty()))
        }
        Commands::Export {
            input,
            format,
            output,
        } => {
            commands::export(&input, format, output.as_deref())?;
            Ok(0)
        }
        Commands::Why {
            file,
            entry_points,
            format,
        } => {
            commands::why(&global, &file, &entry_points, format)?;
            Ok(0)
        }
        Commands::Impact {
            files,
            entry_points,
            format,
        } => {
            commands::impact(&global, &files, &entry_points, format)?;
            Ok(0)
        }
        Commands::Init { force } => {
            commands::init(&global, force)?;
            Ok(0)
        }
    }
}
