//! # sass-dep
//!
//! A library for analyzing SCSS file dependencies and building dependency graphs.
//!
//! ## Overview
//!
//! `sass-dep` parses SCSS files to extract `@use`, `@forward`, and `@import` directives,
//! resolves paths according to Sass conventions, and builds a directed dependency graph
//! that can be analyzed for cycles, metrics, and other structural properties.
//!
//! ## Modules
//!
//! - [`cli`] - Command-line interface definitions
//! - [`commands`] - Command implementations (analyze, check, export, why, impact, init)
//! - [`config`] - `.sass-dep.toml` configuration
//! - [`parser`] - SCSS directive parsing (single-pass byte scanner)
//! - [`resolver`] - Sass-compliant path resolution
//! - [`graph`] - Dependency graph construction and representation
//! - [`analyzer`] - Graph analysis (cycles, metrics, flags)
//! - [`output`] - JSON schema and serialization
//! - [`web`] - Embedded web server for interactive visualization
//!
//! ## Example
//!
//! ```no_run
//! use sass_dep::{Analyzer, DependencyGraph, OutputSchema, Resolver};
//! use std::path::Path;
//!
//! # fn main() -> anyhow::Result<()> {
//! let root = Path::new(".").canonicalize()?;
//! let resolver = Resolver::default();
//! let mut graph = DependencyGraph::new();
//!
//! graph.build_from_entry(&root.join("src/main.scss"), &resolver, &root)?;
//! Analyzer::default().analyze(&mut graph);
//!
//! let schema = OutputSchema::from_graph(&graph, &root);
//! println!("{} files, {} cycles", schema.nodes.len(), schema.analysis.cycles.len());
//! # Ok(())
//! # }
//! ```

pub mod analyzer;
pub mod cli;
pub mod commands;
pub mod config;
pub mod graph;
pub mod output;
pub mod parser;
pub mod resolver;
pub mod web;

// Re-export commonly used types
pub use analyzer::Analyzer;
pub use config::Config;
pub use graph::DependencyGraph;
pub use output::OutputSchema;
pub use parser::Directive;
pub use resolver::Resolver;
