//! Analyzer module for dependency graph analysis.
//!
//! This module provides algorithms for analyzing the dependency graph,
//! including:
//!
//! - Cycle detection via iterative SCCs (Kosaraju) decomposed into ordered cycles
//! - Depth calculation via multi-source BFS from entry points
//! - Fan-in/fan-out computation
//! - Transitive dependencies/dependents via SCC condensation + bitsets
//! - Flag assignment based on configurable thresholds
//!
//! # Example
//!
//! ```no_run
//! use sass_dep::analyzer::Analyzer;
//! use sass_dep::graph::DependencyGraph;
//!
//! let mut graph = DependencyGraph::new();
//! // ... build graph ...
//!
//! let analyzer = Analyzer::default();
//! analyzer.analyze(&mut graph);
//! ```

mod cycles;
mod flags;
mod metrics;

use petgraph::algo::kosaraju_scc;

pub use cycles::detect_cycles;
pub use flags::{assign_flags, FlagThresholds};
pub use metrics::{calculate_depths, calculate_fan_in_out, calculate_transitive_deps};

/// Configuration for the analyzer.
#[derive(Debug, Clone, Default)]
pub struct AnalyzerConfig {
    /// Thresholds for flag assignment.
    pub thresholds: FlagThresholds,
}

/// Analyzer for dependency graphs.
///
/// Performs comprehensive analysis including cycle detection,
/// metric calculation, and flag assignment.
pub struct Analyzer {
    config: AnalyzerConfig,
}

impl Analyzer {
    /// Creates a new analyzer with the given configuration.
    pub fn new(config: AnalyzerConfig) -> Self {
        Self { config }
    }

    /// Performs full analysis on the dependency graph.
    ///
    /// This method:
    /// 1. Computes strongly connected components once (Kosaraju, iterative)
    /// 2. Derives ordered cycles from them
    /// 3. Calculates fan-in/fan-out for all nodes
    /// 4. Calculates depth from entry points
    /// 5. Calculates transitive dependencies and dependents
    /// 6. Assigns flags based on thresholds
    pub fn analyze(&self, graph: &mut crate::graph::DependencyGraph) {
        let sccs = kosaraju_scc(graph.inner());

        let cycles = cycles::cycles_from_sccs(graph, &sccs);
        graph.set_cycles(cycles);

        calculate_fan_in_out(graph);
        calculate_depths(graph);
        metrics::calculate_transitive_from_sccs(graph, &sccs);
        assign_flags(graph, &self.config.thresholds);
    }

    /// Returns the analyzer configuration.
    pub fn config(&self) -> &AnalyzerConfig {
        &self.config
    }
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new(AnalyzerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::graph::{DependencyEdge, DependencyGraph, DirectiveType, FileNode};
    use crate::parser::Location;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// Builds a graph directly (no files) with nodes `n0..n{count}` and the given edges.
    fn synthetic(count: usize, edges: impl IntoIterator<Item = (usize, usize)>) -> DependencyGraph {
        let mut graph = DependencyGraph::new();
        let inner = graph.inner_mut();
        let nodes: Vec<_> = (0..count)
            .map(|i| {
                inner.add_node(FileNode::new(
                    format!("n{i:06}"),
                    PathBuf::from(format!("/n{i}")),
                ))
            })
            .collect();
        for (from, to) in edges {
            inner.add_edge(
                nodes[from],
                nodes[to],
                DependencyEdge::new(DirectiveType::Use, Location::new(1, 1)),
            );
        }
        graph
    }

    #[test]
    fn deep_chain_does_not_overflow_the_stack() {
        // The recursive Tarjan implementation overflowed at ~87k levels.
        let count = 120_000;
        let mut graph = synthetic(count, (0..count - 1).map(|i| (i, i + 1)));
        Analyzer::default().analyze(&mut graph);

        assert!(graph.get_cycles().is_empty());
        assert_eq!(
            graph.inner()[petgraph::graph::NodeIndex::new(0)]
                .metrics
                .transitive_deps,
            count - 1
        );
    }

    #[test]
    fn large_cyclic_component_is_decomposed_quickly() {
        // Barrel star: hub -> every leaf, every leaf -> hub. One SCC of 20k members.
        let count = 20_000;
        let edges = (1..count).flat_map(|i| [(0, i), (i, 0)]);
        let mut graph = synthetic(count, edges);

        let started = Instant::now();
        Analyzer::default().analyze(&mut graph);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "took {:?}",
            started.elapsed()
        );

        let cycles = graph.get_cycles();
        assert_eq!(cycles.len(), count - 1);
        assert!(cycles.iter().all(|c| c.len() == 2 && c[0] == "n000000"));
    }

    #[test]
    fn analyzer_default_config() {
        let analyzer = Analyzer::default();
        assert_eq!(analyzer.config.thresholds.high_fan_in, 5);
        assert_eq!(analyzer.config.thresholds.high_fan_out, 10);
    }
}
