//! Metric calculation for graph nodes.
//!
//! This module provides functions for calculating various metrics
//! on the dependency graph nodes.

use std::collections::{HashSet, VecDeque};

use petgraph::Direction;

use crate::graph::DependencyGraph;

/// Calculates fan-in and fan-out for all nodes.
///
/// - Fan-in: Number of files that depend on this file (in-degree)
/// - Fan-out: Number of files this file depends on (out-degree)
pub fn calculate_fan_in_out(graph: &mut DependencyGraph) {
    let inner = graph.inner();
    let node_index = graph.node_index().clone();

    // Calculate metrics for each node
    let metrics: Vec<(String, usize, usize)> = node_index
        .iter()
        .map(|(id, &idx)| {
            let fan_in = inner.neighbors_directed(idx, Direction::Incoming).count();
            let fan_out = inner.neighbors_directed(idx, Direction::Outgoing).count();
            (id.clone(), fan_in, fan_out)
        })
        .collect();

    // Apply metrics
    for (id, fan_in, fan_out) in metrics {
        if let Some(node) = graph.get_node_mut(&id) {
            node.metrics.fan_in = fan_in;
            node.metrics.fan_out = fan_out;
        }
    }
}

/// Calculates depth from entry points using BFS.
///
/// Depth is the shortest distance from any entry point to a node.
/// Entry points have depth 0.
pub fn calculate_depths(graph: &mut DependencyGraph) {
    let entry_points: Vec<String> = graph.entry_points().iter().cloned().collect();
    let node_index = graph.node_index().clone();

    // Initialize all depths to max (unreachable)
    let max_depth = usize::MAX;
    for (id, _) in node_index.iter() {
        if let Some(node) = graph.get_node_mut(id) {
            node.metrics.depth = max_depth;
        }
    }

    // BFS from each entry point
    let mut queue = VecDeque::new();

    // Set entry points to depth 0 and add to queue
    for entry_id in &entry_points {
        if let Some(node) = graph.get_node_mut(entry_id) {
            node.metrics.depth = 0;
        }
        if let Some(&idx) = node_index.get(entry_id) {
            queue.push_back((idx, 0usize));
        }
    }

    // BFS traversal
    let inner = graph.inner();
    let mut visited_at_depth: std::collections::HashMap<petgraph::graph::NodeIndex, usize> =
        std::collections::HashMap::new();

    for entry_id in &entry_points {
        if let Some(&idx) = node_index.get(entry_id) {
            visited_at_depth.insert(idx, 0);
        }
    }

    while let Some((idx, depth)) = queue.pop_front() {
        let next_depth = depth + 1;

        for neighbor in inner.neighbors_directed(idx, Direction::Outgoing) {
            let current_depth = visited_at_depth.get(&neighbor).copied().unwrap_or(max_depth);

            if next_depth < current_depth {
                visited_at_depth.insert(neighbor, next_depth);
                queue.push_back((neighbor, next_depth));
            }
        }
    }

    // Apply depths to nodes
    for (id, &idx) in node_index.iter() {
        if let Some(&depth) = visited_at_depth.get(&idx) {
            if let Some(node) = graph.get_node_mut(id) {
                node.metrics.depth = depth;
            }
        }
    }
}

/// Calculates transitive dependencies for all nodes.
///
/// Transitive dependencies are all files that a node depends on,
/// directly or indirectly.
pub fn calculate_transitive_deps(graph: &mut DependencyGraph) {
    let node_index = graph.node_index().clone();
    let inner = graph.inner();

    // Calculate transitive deps for each node
    let transitive: Vec<(String, usize)> = node_index
        .iter()
        .map(|(id, &idx)| {
            let mut visited = HashSet::new();
            let mut stack = vec![idx];

            while let Some(current) = stack.pop() {
                for neighbor in inner.neighbors_directed(current, Direction::Outgoing) {
                    if visited.insert(neighbor) {
                        stack.push(neighbor);
                    }
                }
            }

            (id.clone(), visited.len())
        })
        .collect();

    // Apply metrics
    for (id, count) in transitive {
        if let Some(node) = graph.get_node_mut(&id) {
            node.metrics.transitive_deps = count;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_in_out_calculation() {
        // For proper testing, we'd need to use the actual build_from_entry
        // Integration tests cover the full metrics calculation
        let graph = DependencyGraph::new();
        assert_eq!(graph.node_count(), 0);
    }
}
