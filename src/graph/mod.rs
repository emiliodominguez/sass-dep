//! Graph module for dependency representation.
//!
//! This module provides data structures and algorithms for building
//! and querying SCSS dependency graphs.
//!
//! # Architecture
//!
//! The dependency graph is implemented using `petgraph::DiGraph` with:
//! - Nodes representing SCSS files
//! - Edges representing dependency relationships (`@use`, `@forward`, `@import`)
//!
//! An `IndexMap` is used for node indexing to ensure deterministic iteration order.
//!
//! # Example
//!
//! ```no_run
//! use sass_dep::graph::DependencyGraph;
//! use sass_dep::resolver::{Resolver, ResolverConfig};
//! use std::path::PathBuf;
//!
//! let resolver = Resolver::new(ResolverConfig::default());
//! let mut graph = DependencyGraph::new();
//!
//! let root = PathBuf::from("/project");
//! graph.build_from_entry(
//!     &PathBuf::from("/project/src/main.scss"),
//!     &resolver,
//!     &root
//! ).unwrap();
//! ```

mod builder;
mod node;

pub use builder::DependencyGraph;
pub use node::{DependencyEdge, DirectiveType, EdgeMeta, FileNode, NodeFlag, NodeMetrics};

/// Type alias for node indices in the graph.
pub type NodeId = petgraph::graph::NodeIndex;
