//! Versioned JSON output schema.
//!
//! The schema is additive across minor versions: documents produced by
//! older versions deserialize with defaults for newly added fields.
//!
//! ## Version history
//!
//! - **1.1.0**: `metrics.depth` is `null` for unreachable files (was
//!   `usize::MAX`); added `metrics.transitive_dependents`,
//!   `metadata.entry_points`, `analysis.unresolved` and the statistics
//!   `cycle_count`, `unresolved_imports` and `import_directives`. Cycles are
//!   ordered closed paths. Nodes are sorted by ID and edges by endpoints.
//! - **1.0.0**: initial schema.

use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::graph::{
    DependencyGraph, DirectiveType, EdgeMeta, NodeFlag, NodeMetrics, UnresolvedImport,
};
use crate::parser::Location;

/// Current schema version.
pub const SCHEMA_VERSION: &str = "1.1.0";

/// URL of the JSON Schema describing [`OutputSchema`].
pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/emiliodominguez/sass-dep/main/schema/v1.1.0.json";

/// Depth values at or above this are treated as legacy "unreachable" markers.
const LEGACY_UNREACHABLE_DEPTH: usize = 1 << 53;

/// Root output document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSchema {
    /// JSON Schema reference.
    #[serde(rename = "$schema")]
    pub schema: String,
    /// Schema version.
    pub version: String,
    /// Information about the analysis run.
    pub metadata: Metadata,
    /// Nodes keyed by root-relative file ID, sorted by ID.
    pub nodes: IndexMap<String, OutputNode>,
    /// Dependency edges, sorted by `(from, to)`.
    pub edges: Vec<OutputEdge>,
    /// Graph-level analysis results.
    pub analysis: Analysis,
}

/// Information about the analysis run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// RFC 3339 timestamp.
    pub generated_at: String,
    /// Absolute project root.
    pub root: String,
    /// Version of sass-dep that produced the document.
    pub sass_dep_version: String,
    /// Entry point file IDs.
    #[serde(default)]
    pub entry_points: Vec<String>,
}

/// A file in the output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputNode {
    /// Absolute file path.
    pub path: String,
    /// Computed metrics.
    pub metrics: NodeMetrics,
    /// Assigned flags.
    pub flags: Vec<NodeFlag>,
}

/// A dependency in the output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputEdge {
    /// Importing file ID.
    pub from: String,
    /// Imported file ID.
    pub to: String,
    /// Directive that created the dependency.
    pub directive_type: DirectiveType,
    /// Location of the directive in `from`.
    pub location: Location,
    /// Namespace / configuration metadata.
    #[serde(flatten)]
    pub meta: EdgeMeta,
}

/// Graph-level analysis results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    /// Ordered cycles (`cycle[i]` imports `cycle[i + 1]`, last imports first).
    pub cycles: Vec<Vec<String>>,
    /// Directives whose targets could not be resolved.
    #[serde(default)]
    pub unresolved: Vec<UnresolvedImport>,
    /// Summary statistics.
    pub statistics: Statistics,
}

/// Summary statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Statistics {
    /// Number of files in the graph.
    pub total_files: usize,
    /// Number of dependency edges.
    pub total_dependencies: usize,
    /// Number of entry points.
    pub entry_points: usize,
    /// Number of files unreachable from any entry point.
    pub orphan_files: usize,
    /// Number of files without dependencies.
    pub leaf_files: usize,
    /// Largest depth among reachable files.
    pub max_depth: usize,
    /// Largest fan-in.
    pub max_fan_in: usize,
    /// Largest fan-out.
    pub max_fan_out: usize,
    /// Number of reported cycles.
    #[serde(default)]
    pub cycle_count: usize,
    /// Number of unresolved directive targets.
    #[serde(default)]
    pub unresolved_imports: usize,
    /// Number of edges created by legacy `@import`.
    #[serde(default)]
    pub import_directives: usize,
}

impl OutputSchema {
    /// Builds the output document from an analyzed graph.
    ///
    /// # Arguments
    ///
    /// * `graph` - A graph that has already been run through the analyzer
    /// * `root` - Project root directory
    pub fn from_graph(graph: &DependencyGraph, root: &Path) -> Self {
        let mut nodes: Vec<(&String, OutputNode)> = graph
            .nodes()
            .map(|(id, node)| {
                (
                    id,
                    OutputNode {
                        path: node.absolute_path.to_string_lossy().into_owned(),
                        metrics: node.metrics.clone(),
                        flags: node.flags.clone(),
                    },
                )
            })
            .collect();
        nodes.sort_unstable_by(|a, b| a.0.cmp(b.0));

        let mut edges: Vec<OutputEdge> = graph
            .edges()
            .map(|(from, to, edge)| OutputEdge {
                from: from.to_string(),
                to: to.to_string(),
                directive_type: edge.directive_type,
                location: edge.location,
                meta: edge.meta.clone(),
            })
            .collect();
        edges.sort_unstable_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));

        let mut unresolved = graph.unresolved().to_vec();
        unresolved.sort_by(|a, b| {
            (&a.from, a.location.line, a.location.column).cmp(&(
                &b.from,
                b.location.line,
                b.location.column,
            ))
        });

        let nodes: IndexMap<String, OutputNode> =
            nodes.into_iter().map(|(id, n)| (id.clone(), n)).collect();
        let statistics =
            Statistics::compute(&nodes, &edges, graph.get_cycles().len(), unresolved.len());

        Self {
            schema: SCHEMA_URL.to_string(),
            version: SCHEMA_VERSION.to_string(),
            metadata: Metadata {
                generated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                root: root.to_string_lossy().into_owned(),
                sass_dep_version: env!("CARGO_PKG_VERSION").to_string(),
                entry_points: graph.entry_points().iter().cloned().collect(),
            },
            nodes,
            edges,
            analysis: Analysis {
                cycles: graph.get_cycles().to_vec(),
                unresolved,
                statistics,
            },
        }
    }

    /// Parses a document produced by any 1.x version of sass-dep.
    ///
    /// Legacy documents are normalized: `usize::MAX` depths become `None`
    /// and statistics added in later versions are recomputed.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is not a valid sass-dep document.
    pub fn from_json(input: &str) -> Result<Self> {
        let mut schema: Self =
            serde_json::from_str(input).context("Input is not a valid sass-dep JSON document")?;

        if !schema.version.starts_with("1.") {
            anyhow::bail!("Unsupported schema version: {}", schema.version);
        }

        for node in schema.nodes.values_mut() {
            if node
                .metrics
                .depth
                .is_some_and(|d| d >= LEGACY_UNREACHABLE_DEPTH)
            {
                node.metrics.depth = None;
            }
        }

        schema.analysis.statistics = Statistics::compute(
            &schema.nodes,
            &schema.edges,
            schema.analysis.cycles.len(),
            schema.analysis.unresolved.len(),
        );

        Ok(schema)
    }
}

impl Statistics {
    /// Computes statistics from nodes and edges.
    fn compute(
        nodes: &IndexMap<String, OutputNode>,
        edges: &[OutputEdge],
        cycle_count: usize,
        unresolved_imports: usize,
    ) -> Self {
        let mut stats = Statistics {
            total_files: nodes.len(),
            total_dependencies: edges.len(),
            cycle_count,
            unresolved_imports,
            import_directives: edges
                .iter()
                .filter(|e| e.directive_type == DirectiveType::Import)
                .count(),
            ..Default::default()
        };

        for node in nodes.values() {
            let has = |flag: NodeFlag| node.flags.contains(&flag);
            stats.entry_points += usize::from(has(NodeFlag::EntryPoint));
            stats.orphan_files += usize::from(has(NodeFlag::Orphan));
            stats.leaf_files += usize::from(has(NodeFlag::Leaf));
            stats.max_depth = stats.max_depth.max(node.metrics.depth.unwrap_or(0));
            stats.max_fan_in = stats.max_fan_in.max(node.metrics.fan_in);
            stats.max_fan_out = stats.max_fan_out.max(node.metrics.fan_out);
        }

        stats
    }
}
