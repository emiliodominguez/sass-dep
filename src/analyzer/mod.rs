//! Analyzer module for dependency graph analysis.
//!
//! This module provides algorithms for analyzing the dependency graph,
//! including:
//!
//! - Cycle detection using Tarjan's algorithm (with ordered cycle paths)
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

use petgraph::algo::tarjan_scc;

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
    /// 1. Computes strongly connected components once (Tarjan)
    /// 2. Derives ordered cycles from them
    /// 3. Calculates fan-in/fan-out for all nodes
    /// 4. Calculates depth from entry points
    /// 5. Calculates transitive dependencies and dependents
    /// 6. Assigns flags based on thresholds
    pub fn analyze(&self, graph: &mut crate::graph::DependencyGraph) {
        let sccs = tarjan_scc(graph.inner());

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

    #[test]
    fn analyzer_default_config() {
        let analyzer = Analyzer::default();
        assert_eq!(analyzer.config.thresholds.high_fan_in, 5);
        assert_eq!(analyzer.config.thresholds.high_fan_out, 10);
    }
}
