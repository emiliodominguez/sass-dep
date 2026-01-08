//! Node and edge data structures for the dependency graph.
//!
//! This module defines the data associated with graph nodes (files)
//! and edges (dependencies).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::parser::Location;

/// A node in the dependency graph representing an SCSS file.
#[derive(Debug, Clone)]
pub struct FileNode {
    /// Relative path from project root (canonical identifier).
    pub id: String,
    /// Absolute path to the file.
    pub absolute_path: PathBuf,
    /// Computed metrics for this node.
    pub metrics: NodeMetrics,
    /// Flags assigned to this node.
    pub flags: Vec<NodeFlag>,
}

impl FileNode {
    /// Creates a new file node.
    ///
    /// # Arguments
    ///
    /// * `id` - Relative path identifier
    /// * `absolute_path` - Absolute path to the file
    pub fn new(id: String, absolute_path: PathBuf) -> Self {
        Self {
            id,
            absolute_path,
            metrics: NodeMetrics::default(),
            flags: Vec::new(),
        }
    }

    /// Adds a flag to this node if not already present.
    pub fn add_flag(&mut self, flag: NodeFlag) {
        if !self.flags.contains(&flag) {
            self.flags.push(flag);
        }
    }

    /// Removes a flag from this node.
    pub fn remove_flag(&mut self, flag: &NodeFlag) {
        self.flags.retain(|f| f != flag);
    }

    /// Checks if this node has a specific flag.
    pub fn has_flag(&self, flag: &NodeFlag) -> bool {
        self.flags.contains(flag)
    }
}

/// Computed metrics for a file node.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetrics {
    /// Number of files that depend on this file (in-degree).
    pub fan_in: usize,
    /// Number of files this file depends on (out-degree).
    pub fan_out: usize,
    /// Distance from the nearest entry point.
    pub depth: usize,
    /// Total number of transitive dependencies.
    pub transitive_deps: usize,
}

/// Flags that can be assigned to nodes based on analysis.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeFlag {
    /// This file is an entry point (explicitly specified).
    EntryPoint,
    /// This file has no dependencies (leaf node).
    Leaf,
    /// This file is not reachable from any entry point.
    Orphan,
    /// This file has unusually high fan-in.
    HighFanIn,
    /// This file has unusually high fan-out.
    HighFanOut,
    /// This file is part of a dependency cycle.
    InCycle,
}

impl std::fmt::Display for NodeFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeFlag::EntryPoint => write!(f, "entry_point"),
            NodeFlag::Leaf => write!(f, "leaf"),
            NodeFlag::Orphan => write!(f, "orphan"),
            NodeFlag::HighFanIn => write!(f, "high_fan_in"),
            NodeFlag::HighFanOut => write!(f, "high_fan_out"),
            NodeFlag::InCycle => write!(f, "in_cycle"),
        }
    }
}

/// An edge in the dependency graph representing a dependency.
#[derive(Debug, Clone)]
pub struct DependencyEdge {
    /// Type of directive that created this dependency.
    pub directive_type: DirectiveType,
    /// Source location of the directive.
    pub location: Location,
    /// Additional metadata about the edge.
    pub meta: EdgeMeta,
}

impl DependencyEdge {
    /// Creates a new dependency edge.
    pub fn new(directive_type: DirectiveType, location: Location) -> Self {
        Self {
            directive_type,
            location,
            meta: EdgeMeta::default(),
        }
    }

    /// Creates a new edge with metadata.
    pub fn with_meta(directive_type: DirectiveType, location: Location, meta: EdgeMeta) -> Self {
        Self {
            directive_type,
            location,
            meta,
        }
    }
}

/// Type of directive that created a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DirectiveType {
    /// `@use` directive.
    Use,
    /// `@forward` directive.
    Forward,
    /// `@import` directive (legacy).
    Import,
}

impl std::fmt::Display for DirectiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirectiveType::Use => write!(f, "use"),
            DirectiveType::Forward => write!(f, "forward"),
            DirectiveType::Import => write!(f, "import"),
        }
    }
}

/// Additional metadata for a dependency edge.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EdgeMeta {
    /// Namespace used for this import (for `@use`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Whether the module is configured (for `@use ... with`).
    pub configured: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_node_new() {
        let node = FileNode::new("src/main.scss".to_string(), PathBuf::from("/project/src/main.scss"));

        assert_eq!(node.id, "src/main.scss");
        assert_eq!(node.absolute_path, PathBuf::from("/project/src/main.scss"));
        assert!(node.flags.is_empty());
    }

    #[test]
    fn file_node_flags() {
        let mut node = FileNode::new("test.scss".to_string(), PathBuf::from("/test.scss"));

        node.add_flag(NodeFlag::EntryPoint);
        assert!(node.has_flag(&NodeFlag::EntryPoint));

        node.add_flag(NodeFlag::EntryPoint); // Duplicate
        assert_eq!(node.flags.len(), 1);

        node.remove_flag(&NodeFlag::EntryPoint);
        assert!(!node.has_flag(&NodeFlag::EntryPoint));
    }

    #[test]
    fn dependency_edge_new() {
        let edge = DependencyEdge::new(DirectiveType::Use, Location::new(1, 1));

        assert_eq!(edge.directive_type, DirectiveType::Use);
        assert_eq!(edge.location.line, 1);
        assert!(!edge.meta.configured);
    }

    #[test]
    fn directive_type_display() {
        assert_eq!(DirectiveType::Use.to_string(), "use");
        assert_eq!(DirectiveType::Forward.to_string(), "forward");
        assert_eq!(DirectiveType::Import.to_string(), "import");
    }

    #[test]
    fn node_flag_display() {
        assert_eq!(NodeFlag::EntryPoint.to_string(), "entry_point");
        assert_eq!(NodeFlag::InCycle.to_string(), "in_cycle");
    }
}
