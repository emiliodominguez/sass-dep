//! Analyzer module for dependency graph analysis.
//!
//! This module provides algorithms for analyzing the dependency graph,
//! including:
//!
//! - Cycle detection using Tarjan's algorithm
//! - Depth calculation via BFS from entry points
//! - Fan-in/fan-out computation
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
    /// 1. Detects cycles using Tarjan's algorithm
    /// 2. Calculates fan-in/fan-out for all nodes
    /// 3. Calculates depth from entry points
    /// 4. Calculates transitive dependencies
    /// 5. Assigns flags based on thresholds
    pub fn analyze(&self, graph: &mut crate::graph::DependencyGraph) {
        // Step 1: Detect cycles
        let cycles = detect_cycles(graph);
        graph.set_cycles(cycles);

        // Step 2: Calculate fan-in/fan-out
        calculate_fan_in_out(graph);

        // Step 3: Calculate depths
        calculate_depths(graph);

        // Step 4: Calculate transitive dependencies
        calculate_transitive_deps(graph);

        // Step 5: Assign flags
        assign_flags(graph, &self.config.thresholds);
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
