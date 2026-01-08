//! Dependency graph builder.
//!
//! This module implements the graph construction algorithm that
//! recursively discovers and adds dependencies.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use petgraph::graph::DiGraph;
use walkdir::WalkDir;

use super::node::{DependencyEdge, DirectiveType, EdgeMeta, FileNode, NodeFlag};
use super::NodeId;
use crate::parser::{Directive, Namespace, Parser};
use crate::resolver::Resolver;

/// A dependency graph representing SCSS file relationships.
///
/// The graph uses `petgraph::DiGraph` for efficient graph operations
/// and `IndexMap` for deterministic node ordering.
pub struct DependencyGraph {
    /// The underlying directed graph.
    graph: DiGraph<FileNode, DependencyEdge>,
    /// Map from file ID to node index.
    node_index: IndexMap<String, NodeId>,
    /// Set of entry point file IDs.
    entry_points: HashSet<String>,
    /// Detected cycles (populated after analysis).
    cycles: Vec<Vec<String>>,
}

impl DependencyGraph {
    /// Creates a new empty dependency graph.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_index: IndexMap::new(),
            entry_points: HashSet::new(),
            cycles: Vec::new(),
        }
    }

    /// Builds the dependency graph starting from an entry point file.
    ///
    /// This method recursively discovers all dependencies and adds them
    /// to the graph.
    ///
    /// # Arguments
    ///
    /// * `entry` - Path to the entry point SCSS file
    /// * `resolver` - Resolver for import paths
    /// * `root` - Project root directory for computing relative paths
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The entry file cannot be read
    /// - Parsing fails
    /// - A dependency cannot be resolved
    pub fn build_from_entry(
        &mut self,
        entry: &Path,
        resolver: &Resolver,
        root: &Path,
    ) -> Result<NodeId> {
        let entry = entry.canonicalize().context("Failed to canonicalize entry path")?;

        // Add entry point node
        let entry_id = self.add_file(&entry, root)?;

        // Mark as entry point
        self.entry_points.insert(entry_id.clone());
        if let Some(node) = self.get_node_mut(&entry_id) {
            node.add_flag(NodeFlag::EntryPoint);
        }

        // Process the entry point
        self.process_file(&entry, resolver, root)?;

        // Return the node ID
        Ok(*self.node_index.get(&entry_id).unwrap())
    }

    /// Processes a file, extracting and following its dependencies.
    fn process_file(&mut self, path: &Path, resolver: &Resolver, root: &Path) -> Result<()> {
        // Parse the file
        let directives = Parser::parse_file(path)
            .with_context(|| format!("Failed to parse: {}", path.display()))?;

        let from_id = self.get_file_id(path, root);

        // Process each directive
        for directive in directives {
            self.process_directive(&directive, path, resolver, root, &from_id)?;
        }

        Ok(())
    }

    /// Checks if a target is a Sass built-in module.
    ///
    /// Built-in modules like `sass:math`, `sass:map`, `sass:color`, etc.
    /// are provided by the Sass compiler and don't exist as files.
    fn is_builtin_module(target: &str) -> bool {
        target.starts_with("sass:")
    }

    /// Processes a single directive.
    fn process_directive(
        &mut self,
        directive: &Directive,
        from_path: &Path,
        resolver: &Resolver,
        root: &Path,
        from_id: &str,
    ) -> Result<()> {
        let paths = directive.paths();
        let location = directive.location().clone();

        for target in paths {
            // Skip Sass built-in modules (sass:math, sass:map, etc.)
            if Self::is_builtin_module(target) {
                continue;
            }

            // Resolve the import path
            let resolved = match resolver.resolve(from_path, target) {
                Ok(p) => p,
                Err(e) => {
                    // Log warning but continue (soft failure)
                    eprintln!(
                        "Warning: Could not resolve '{}' from '{}': {}",
                        target,
                        from_path.display(),
                        e
                    );
                    continue;
                }
            };

            // Add the target file
            let to_id = self.add_file(&resolved, root)?;
            let already_processed = self.node_index.contains_key(&to_id)
                && self.get_node(&to_id).map(|n| !n.flags.is_empty() || n.metrics.fan_in > 0 || n.metrics.fan_out > 0).unwrap_or(false);

            // Create edge
            let (directive_type, meta) = match directive {
                Directive::Use(u) => {
                    let namespace = match &u.namespace {
                        Some(Namespace::Named(n)) => Some(n.clone()),
                        Some(Namespace::Star) => Some("*".to_string()),
                        Some(Namespace::Default) | None => None,
                    };
                    (
                        DirectiveType::Use,
                        EdgeMeta {
                            namespace,
                            configured: u.configured,
                        },
                    )
                }
                Directive::Forward(_) => (DirectiveType::Forward, EdgeMeta::default()),
                Directive::Import(_) => (DirectiveType::Import, EdgeMeta::default()),
            };

            let edge = DependencyEdge::with_meta(directive_type, location.clone(), meta);

            // Add edge to graph
            self.add_edge(from_id, &to_id, edge);

            // Recursively process the target if not already done
            // Check if we've already started processing this file
            let is_new = !already_processed;
            if is_new {
                self.process_file(&resolved, resolver, root)?;
            }
        }

        Ok(())
    }

    /// Adds a file to the graph if not already present.
    ///
    /// Returns the file's ID.
    fn add_file(&mut self, path: &Path, root: &Path) -> Result<String> {
        let id = self.get_file_id(path, root);

        if !self.node_index.contains_key(&id) {
            let node = FileNode::new(id.clone(), path.to_path_buf());
            let idx = self.graph.add_node(node);
            self.node_index.insert(id.clone(), idx);
        }

        Ok(id)
    }

    /// Computes the file ID (relative path) from an absolute path.
    fn get_file_id(&self, path: &Path, root: &Path) -> String {
        path.strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Adds an edge between two files.
    fn add_edge(&mut self, from: &str, to: &str, edge: DependencyEdge) {
        let from_idx = *self.node_index.get(from).expect("from node not found");
        let to_idx = *self.node_index.get(to).expect("to node not found");

        // Check if edge already exists
        if self.graph.find_edge(from_idx, to_idx).is_none() {
            self.graph.add_edge(from_idx, to_idx, edge);
        }
    }

    /// Discovers orphan files in the project root.
    ///
    /// Orphan files are SCSS files that are not reachable from any entry point.
    pub fn discover_orphans(&mut self, root: &Path, _resolver: &Resolver) -> Result<()> {
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "scss" || ext == "sass")
                    .unwrap_or(false)
            })
        {
            let path = entry.path().canonicalize()?;
            let id = self.get_file_id(&path, root);

            if !self.node_index.contains_key(&id) {
                let mut node = FileNode::new(id.clone(), path);
                node.add_flag(NodeFlag::Orphan);
                let idx = self.graph.add_node(node);
                self.node_index.insert(id, idx);
            }
        }

        Ok(())
    }

    /// Returns the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Returns an iterator over all node IDs and their data.
    pub fn nodes(&self) -> impl Iterator<Item = (&String, &FileNode)> {
        self.node_index
            .iter()
            .map(|(id, idx)| (id, &self.graph[*idx]))
    }

    /// Returns a mutable reference to a node by ID.
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut FileNode> {
        self.node_index.get(id).map(|idx| &mut self.graph[*idx])
    }

    /// Returns a reference to a node by ID.
    pub fn get_node(&self, id: &str) -> Option<&FileNode> {
        self.node_index.get(id).map(|idx| &self.graph[*idx])
    }

    /// Returns the entry point file IDs.
    pub fn entry_points(&self) -> &HashSet<String> {
        &self.entry_points
    }

    /// Returns a reference to the underlying petgraph.
    pub fn inner(&self) -> &DiGraph<FileNode, DependencyEdge> {
        &self.graph
    }

    /// Returns a mutable reference to the underlying petgraph.
    pub fn inner_mut(&mut self) -> &mut DiGraph<FileNode, DependencyEdge> {
        &mut self.graph
    }

    /// Returns the node index map.
    pub fn node_index(&self) -> &IndexMap<String, NodeId> {
        &self.node_index
    }

    /// Sets the detected cycles.
    pub fn set_cycles(&mut self, cycles: Vec<Vec<String>>) {
        self.cycles = cycles;
    }

    /// Returns the detected cycles.
    pub fn get_cycles(&self) -> &[Vec<String>] {
        &self.cycles
    }

    /// Returns all edges as (from_id, to_id, edge) tuples.
    pub fn edges(&self) -> impl Iterator<Item = (&str, &str, &DependencyEdge)> {
        self.graph.edge_indices().map(move |idx| {
            let (from_idx, to_idx) = self.graph.edge_endpoints(idx).unwrap();
            let from_id = self
                .node_index
                .iter()
                .find(|(_, &i)| i == from_idx)
                .map(|(id, _)| id.as_str())
                .unwrap();
            let to_id = self
                .node_index
                .iter()
                .find(|(_, &i)| i == to_idx)
                .map(|(id, _)| id.as_str())
                .unwrap();
            (from_id, to_id, &self.graph[idx])
        })
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_simple_project(dir: &Path) {
        fs::write(
            dir.join("main.scss"),
            r#"@use "variables" as vars;
@use "mixins";
"#,
        )
        .unwrap();

        fs::write(dir.join("_variables.scss"), "$primary: blue;\n").unwrap();

        fs::write(
            dir.join("_mixins.scss"),
            r#"@use "variables" as vars;
@mixin test { color: vars.$primary; }
"#,
        )
        .unwrap();
    }

    #[test]
    fn build_simple_graph() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().canonicalize().unwrap();
        setup_simple_project(&root);

        let resolver = Resolver::default();
        let mut graph = DependencyGraph::new();

        graph
            .build_from_entry(&root.join("main.scss"), &resolver, &root)
            .unwrap();

        assert_eq!(graph.node_count(), 3);
        // main -> variables, main -> mixins, mixins -> variables
        assert_eq!(graph.edge_count(), 3);
    }

    #[test]
    fn entry_point_flagged() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().canonicalize().unwrap();
        setup_simple_project(&root);

        let resolver = Resolver::default();
        let mut graph = DependencyGraph::new();

        graph
            .build_from_entry(&root.join("main.scss"), &resolver, &root)
            .unwrap();

        let main_node = graph.get_node("main.scss").unwrap();
        assert!(main_node.has_flag(&NodeFlag::EntryPoint));

        let vars_node = graph.get_node("_variables.scss").unwrap();
        assert!(!vars_node.has_flag(&NodeFlag::EntryPoint));
    }

    #[test]
    fn relative_ids() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("src/components")).unwrap();
        fs::write(
            root.join("src/main.scss"),
            r#"@use "components/button";"#,
        )
        .unwrap();
        fs::write(root.join("src/components/_button.scss"), "").unwrap();

        let resolver = Resolver::default();
        let mut graph = DependencyGraph::new();

        graph
            .build_from_entry(&root.join("src/main.scss"), &resolver, &root)
            .unwrap();

        assert!(graph.get_node("src/main.scss").is_some());
        assert!(graph.get_node("src/components/_button.scss").is_some());
    }
}
