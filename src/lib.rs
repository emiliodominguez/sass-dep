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
//! - [`parser`] - SCSS directive parsing using nom
//! - [`resolver`] - Sass-compliant path resolution
//! - [`graph`] - Dependency graph construction and representation
//! - [`analyzer`] - Graph analysis (cycles, metrics, flags)
//! - [`output`] - JSON schema and serialization
//! - [`web`] - Embedded web server for interactive visualization
//!
//! ## Example
//!
//! ```no_run
//! use sass_dep::graph::DependencyGraph;
//! use sass_dep::resolver::{Resolver, ResolverConfig};
//! use std::path::PathBuf;
//!
//! let config = ResolverConfig::default();
//! let resolver = Resolver::new(config);
//! let mut graph = DependencyGraph::new();
//!
//! // Build graph from entry point
//! // graph.build_from_entry(&PathBuf::from("src/main.scss"), &resolver)?;
//! ```

pub mod analyzer;
pub mod cli;
pub mod commands;
pub mod graph;
pub mod output;
pub mod parser;
pub mod resolver;
pub mod web;

// Re-export commonly used types
pub use analyzer::Analyzer;
pub use graph::DependencyGraph;
pub use output::OutputSchema;
pub use parser::Directive;
pub use resolver::Resolver;
