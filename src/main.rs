//! sass-dep - SCSS dependency graph analyzer
//!
//! This is the main entry point for the CLI application.

use anyhow::Result;
use clap::Parser;
use sass_dep::cli::{Cli, Commands};
use sass_dep::commands::AnalyzeOptions;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            entry_points,
            output,
            format,
            include_orphans,
            web,
            port,
        } => {
            sass_dep::commands::analyze(AnalyzeOptions {
                root: &cli.root,
                load_paths: &cli.load_paths,
                entry_points: &entry_points,
                output: output.as_deref(),
                format,
                include_orphans,
                quiet: cli.quiet,
                verbose: cli.verbose,
                web,
                port,
            })?;
        }
        Commands::Check {
            entry_points,
            no_cycles,
            max_depth,
            max_fan_out,
            max_fan_in,
        } => {
            let violations = sass_dep::commands::check(
                &cli.root,
                &cli.load_paths,
                &entry_points,
                no_cycles,
                max_depth,
                max_fan_out,
                max_fan_in,
                cli.quiet,
                cli.verbose,
            )?;

            if !violations.is_empty() {
                std::process::exit(1);
            }
        }
        Commands::Export {
            input,
            format,
        } => {
            sass_dep::commands::export(&input, format)?;
        }
    }

    Ok(())
}
