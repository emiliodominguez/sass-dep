//! Cycle detection using Tarjan's algorithm.
//!
//! This module detects strongly connected components (SCCs) in the
//! dependency graph to identify circular dependencies.

use petgraph::algo::tarjan_scc;

use crate::graph::DependencyGraph;

/// Detects cycles in the dependency graph.
///
/// Uses Tarjan's algorithm to find strongly connected components (SCCs).
/// Any SCC with more than one node represents a cycle.
///
/// # Arguments
///
/// * `graph` - The dependency graph to analyze
///
/// # Returns
///
/// A vector of cycles, where each cycle is a vector of file IDs
/// in the order they form the cycle.
pub fn detect_cycles(graph: &DependencyGraph) -> Vec<Vec<String>> {
    let inner = graph.inner();
    let node_index = graph.node_index();

    // Find strongly connected components
    let sccs = tarjan_scc(inner);

    // Filter to SCCs with more than one node (actual cycles)
    let mut cycles = Vec::new();
    for scc in sccs {
        if scc.len() > 1 {
            // Convert node indices to file IDs
            let cycle: Vec<String> = scc
                .iter()
                .filter_map(|idx| {
                    node_index
                        .iter()
                        .find(|(_, &i)| i == *idx)
                        .map(|(id, _)| id.clone())
                })
                .collect();

            if !cycle.is_empty() {
                cycles.push(cycle);
            }
        }
    }

    cycles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_no_cycles_empty_graph() {
        let graph = DependencyGraph::new();
        let cycles = detect_cycles(&graph);
        assert!(cycles.is_empty());
    }

    // Note: More comprehensive cycle detection tests are in integration_tests.rs
    // using the actual build_from_entry API to construct graphs properly.
}
