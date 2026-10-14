//! Command-line interface module.
//!
//! This module defines the CLI structure using `clap` derive macros,
//! including all commands, flags, and arguments.

mod commands;

pub use commands::{
    CheckFormat, Cli, Commands, ExportFormat, GraphArgs, OutputFormat, ReportFormat,
};
