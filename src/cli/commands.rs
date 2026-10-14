//! CLI command definitions.
//!
//! Defines the main CLI structure and all available subcommands
//! for the sass-dep tool.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// SCSS dependency graph analyzer.
///
/// Analyzes SCSS codebases to build file-level dependency graphs,
/// detect cycles, and compute structural metrics.
#[derive(Parser, Debug)]
#[command(name = "sass-dep")]
#[command(author = "Emilio Dominguez")]
#[command(version)]
#[command(about = "SCSS dependency graph analyzer", long_about = None)]
pub struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,

    /// Project root directory.
    ///
    /// File IDs, config paths and relative load paths are resolved from here.
    #[arg(long, default_value = ".", global = true)]
    pub root: PathBuf,

    /// Config file path [default: <root>/.sass-dep.toml if present].
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Add Sass load path (can be repeated).
    ///
    /// Directories to search when resolving @use, @forward, and @import paths.
    /// Similar to the Sass --load-path option. Relative paths are resolved
    /// against --root.
    #[arg(long = "load-path", short = 'I', global = true)]
    pub load_paths: Vec<PathBuf>,

    /// Suppress non-error output.
    #[arg(long, short, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Increase verbosity (-v: config path, timings and every unresolved import).
    #[arg(long, short, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

/// Options shared by every command that builds a graph.
#[derive(Args, Debug, Clone, Default)]
pub struct GraphArgs {
    /// Entry point files [default: `entry_points` from the config file].
    pub entry_points: Vec<PathBuf>,

    /// Include files not reachable from any entry point (flagged `orphan`).
    #[arg(long)]
    pub include_orphans: bool,
}

/// Available subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Build dependency graph and run analysis.
    ///
    /// Parses all SCSS files reachable from the entry points,
    /// builds a dependency graph, and outputs analysis results.
    Analyze {
        #[command(flatten)]
        graph: GraphArgs,

        /// Output file (default: stdout).
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Output format.
        #[arg(long, default_value = "json", value_enum)]
        format: OutputFormat,

        /// Open interactive web visualization.
        ///
        /// Starts a local HTTP server and opens the browser
        /// to view the dependency graph interactively.
        #[arg(long)]
        web: bool,

        /// Port for the web server (tries the next ports if busy).
        #[arg(long, default_value = "3000", requires = "web")]
        port: u16,

        /// Do not open the browser automatically (with --web).
        #[arg(long, requires = "web")]
        no_open: bool,
    },

    /// Verify graph integrity and architecture rules (CI mode).
    ///
    /// Exits with status 1 if any constraint is violated. Constraints
    /// from the `[check]` and `[[rules]]` config sections are applied
    /// in addition to the flags below.
    Check {
        #[command(flatten)]
        graph: GraphArgs,

        /// Fail on circular dependencies.
        #[arg(long)]
        no_cycles: bool,

        /// Maximum allowed depth in the dependency tree.
        #[arg(long)]
        max_depth: Option<usize>,

        /// Maximum allowed direct dependencies per file.
        #[arg(long)]
        max_fan_out: Option<usize>,

        /// Maximum allowed direct dependents per file.
        #[arg(long)]
        max_fan_in: Option<usize>,

        /// Fail on files unreachable from every entry point (implies --include-orphans).
        #[arg(long)]
        no_orphans: bool,

        /// Fail on imports that cannot be resolved.
        #[arg(long)]
        no_unresolved: bool,

        /// Fail on legacy @import usage (removed in Dart Sass 3.0).
        #[arg(long)]
        no_legacy_imports: bool,

        /// Report format.
        #[arg(long, default_value = "text", value_enum)]
        format: CheckFormat,
    },

    /// Export a JSON analysis to a visualization format.
    ///
    /// Converts a previously generated JSON analysis file
    /// to various graph visualization formats.
    Export {
        /// Input JSON file (`-` reads from stdin).
        input: PathBuf,

        /// Output format.
        #[arg(long, default_value = "dot", value_enum)]
        format: ExportFormat,

        /// Output file (default: stdout).
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// Explain why a file is included: shortest import chain from an entry point.
    Why {
        /// File to explain (path or file ID).
        file: PathBuf,

        /// Entry point (can be repeated) [default: config `entry_points`].
        #[arg(long = "entry", short = 'e')]
        entry_points: Vec<PathBuf>,

        /// Report format.
        #[arg(long, default_value = "text", value_enum)]
        format: ReportFormat,
    },

    /// List every file and entry point affected by changes to the given files.
    ///
    /// Example: `sass-dep impact $(git diff --name-only main) --format json`
    Impact {
        /// Changed files (paths or file IDs). Non-Sass files are ignored.
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Entry point (can be repeated) [default: config `entry_points`].
        #[arg(long = "entry", short = 'e')]
        entry_points: Vec<PathBuf>,

        /// Report format.
        #[arg(long, default_value = "text", value_enum)]
        format: ReportFormat,
    },

    /// Create a starter .sass-dep.toml with detected entry points.
    Init {
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },
}

/// Output formats for the analyze command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Full analysis as a JSON document conforming to the sass-dep schema.
    Json,
    /// Graphviz DOT.
    Dot,
    /// Mermaid flowchart.
    Mermaid,
    /// D2 diagram.
    D2,
}

/// Export formats for graph visualization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExportFormat {
    /// Graphviz DOT format.
    ///
    /// Can be rendered using `dot`, `neato`, or other
    /// Graphviz tools.
    Dot,

    /// Mermaid diagram format.
    ///
    /// Can be rendered in Markdown files or using
    /// the Mermaid CLI.
    Mermaid,

    /// D2 diagram format.
    ///
    /// Can be rendered using the D2 CLI tool.
    D2,
}

/// Report formats for `check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CheckFormat {
    /// Human-readable text on stderr.
    Text,
    /// Machine-readable JSON on stdout.
    Json,
    /// GitHub Actions workflow annotations on stdout.
    Github,
}

/// Report formats for `why` and `impact`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReportFormat {
    /// Human-readable text.
    Text,
    /// Machine-readable JSON.
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
