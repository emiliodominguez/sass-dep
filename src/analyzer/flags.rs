//! Flag assignment based on analysis results.
//!
//! This module assigns flags to nodes based on their metrics
//! and position in the graph.

use crate::graph::{DependencyGraph, NodeFlag};

/// Thresholds for flag assignment.
#[derive(Debug, Clone)]
pub struct FlagThresholds {
    /// Fan-in threshold for HighFanIn flag.
    pub high_fan_in: usize,
    /// Fan-out threshold for HighFanOut flag.
    pub high_fan_out: usize,
}

impl Default for FlagThresholds {
    fn default() -> Self {
        Self {
            high_fan_in: 5,
            high_fan_out: 10,
        }
    }
}

/// Assigns flags to all nodes based on their metrics and position.
///
/// Flags assigned:
/// - `Leaf`: Nodes with no outgoing dependencies (fan-out = 0)
/// - `HighFanIn`: Nodes with fan-in >= threshold
/// - `HighFanOut`: Nodes with fan-out >= threshold
/// - `InCycle`: Nodes that are part of a detected cycle
///
/// Note: `EntryPoint` and `Orphan` flags are assigned during graph construction.
pub fn assign_flags(graph: &mut DependencyGraph, thresholds: &FlagThresholds) {
    // Collect cycle members
    let cycle_members: std::collections::HashSet<String> = graph
        .get_cycles()
        .iter()
        .flatten()
        .cloned()
        .collect();

    // Collect node IDs for iteration
    let node_ids: Vec<String> = graph.nodes().map(|(id, _)| id.clone()).collect();

    for id in node_ids {
        let (fan_in, fan_out, is_in_cycle) = {
            let node = graph.get_node(&id).unwrap();
            (
                node.metrics.fan_in,
                node.metrics.fan_out,
                cycle_members.contains(&id),
            )
        };

        if let Some(node) = graph.get_node_mut(&id) {
            // Leaf: no outgoing dependencies
            if fan_out == 0 {
                node.add_flag(NodeFlag::Leaf);
            }

            // High fan-in
            if fan_in >= thresholds.high_fan_in {
                node.add_flag(NodeFlag::HighFanIn);
            }

            // High fan-out
            if fan_out >= thresholds.high_fan_out {
                node.add_flag(NodeFlag::HighFanOut);
            }

            // In cycle
            if is_in_cycle {
                node.add_flag(NodeFlag::InCycle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_thresholds() {
        let thresholds = FlagThresholds::default();
        assert_eq!(thresholds.high_fan_in, 5);
        assert_eq!(thresholds.high_fan_out, 10);
    }

    #[test]
    fn custom_thresholds() {
        let thresholds = FlagThresholds {
            high_fan_in: 3,
            high_fan_out: 5,
        };
        assert_eq!(thresholds.high_fan_in, 3);
        assert_eq!(thresholds.high_fan_out, 5);
    }
}
