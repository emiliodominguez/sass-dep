//! Flag assignment based on analysis results.
//!
//! This module assigns flags to nodes based on their metrics
//! and position in the graph.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::graph::{DependencyGraph, NodeFlag};

/// Thresholds for flag assignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FlagThresholds {
    /// Fan-in at or above which a node gets the `HighFanIn` flag.
    pub high_fan_in: usize,
    /// Fan-out at or above which a node gets the `HighFanOut` flag.
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
/// - `Orphan`: Nodes unreachable from every entry point (requires depths)
///
/// Note: `EntryPoint` is assigned during graph construction.
pub fn assign_flags(graph: &mut DependencyGraph, thresholds: &FlagThresholds) {
    let cycle_members: HashSet<String> = graph.get_cycles().iter().flatten().cloned().collect();
    let has_entry_points = !graph.entry_points().is_empty();

    for node in graph.inner_mut().node_weights_mut() {
        let metrics = &node.metrics;
        let (fan_in, fan_out, unreachable) =
            (metrics.fan_in, metrics.fan_out, metrics.depth.is_none());

        if fan_out == 0 {
            node.add_flag(NodeFlag::Leaf);
        }
        if fan_in >= thresholds.high_fan_in {
            node.add_flag(NodeFlag::HighFanIn);
        }
        if fan_out >= thresholds.high_fan_out {
            node.add_flag(NodeFlag::HighFanOut);
        }
        if cycle_members.contains(&node.id) {
            node.add_flag(NodeFlag::InCycle);
        }
        if has_entry_points && unreachable && !node.has_flag(&NodeFlag::EntryPoint) {
            node.add_flag(NodeFlag::Orphan);
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
}
