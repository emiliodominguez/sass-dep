//! Cycle detection using Tarjan's algorithm.
//!
//! Strongly connected components (SCCs) identify *which* files take part in
//! circular dependencies. Because an SCC is an unordered set, each one is
//! then decomposed into concrete, ordered cycles: starting from the
//! lexicographically smallest uncovered member, a BFS restricted to the
//! component finds the shortest cycle through it. This repeats until every
//! member is covered, so every `in_cycle` file appears in at least one
//! reported cycle and every reported cycle is a real closed path.

use std::collections::{HashMap, VecDeque};

use petgraph::algo::tarjan_scc;
use petgraph::graph::NodeIndex;
use petgraph::Direction;

use crate::graph::DependencyGraph;

/// Detects cycles in the dependency graph.
///
/// # Arguments
///
/// * `graph` - The dependency graph to analyze
///
/// # Returns
///
/// A sorted vector of cycles. Each cycle lists file IDs in import order:
/// `cycle[i]` imports `cycle[i + 1]`, and the last entry imports the first.
/// A file that imports itself is reported as a single-element cycle.
pub fn detect_cycles(graph: &DependencyGraph) -> Vec<Vec<String>> {
    cycles_from_sccs(graph, &tarjan_scc(graph.inner()))
}

/// Decomposes precomputed SCCs into ordered cycles.
pub(crate) fn cycles_from_sccs(
    graph: &DependencyGraph,
    sccs: &[Vec<NodeIndex>],
) -> Vec<Vec<String>> {
    let inner = graph.inner();
    let id = |idx: NodeIndex| inner[idx].id.as_str();

    let mut component = vec![usize::MAX; inner.node_count()];
    for (c, scc) in sccs.iter().enumerate() {
        for node in scc {
            component[node.index()] = c;
        }
    }

    let mut covered = vec![false; inner.node_count()];
    let mut cycles = Vec::new();

    for (c, scc) in sccs.iter().enumerate() {
        if scc.len() == 1 {
            let node = scc[0];
            if inner.contains_edge(node, node) {
                cycles.push(vec![id(node).to_string()]);
            }
            continue;
        }

        let mut members = scc.clone();
        members.sort_unstable_by(|a, b| id(*a).cmp(id(*b)));

        for &start in &members {
            if covered[start.index()] {
                continue;
            }
            let path = shortest_cycle_through(graph, start, &component, c);
            for node in &path {
                covered[node.index()] = true;
            }
            cycles.push(path.into_iter().map(|n| id(n).to_string()).collect());
        }
    }

    cycles.sort_unstable();
    cycles
}

/// Finds the shortest cycle through `start` inside component `c` (BFS).
fn shortest_cycle_through(
    graph: &DependencyGraph,
    start: NodeIndex,
    component: &[usize],
    c: usize,
) -> Vec<NodeIndex> {
    let inner = graph.inner();
    let mut parent: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut queue = VecDeque::from([start]);

    while let Some(current) = queue.pop_front() {
        for next in inner.neighbors_directed(current, Direction::Outgoing) {
            if component[next.index()] != c {
                continue;
            }
            if next == start {
                let mut path = vec![current];
                let mut node = current;
                while node != start {
                    node = parent[&node];
                    path.push(node);
                }
                path.reverse();
                return path;
            }
            if let std::collections::hash_map::Entry::Vacant(entry) = parent.entry(next) {
                entry.insert(current);
                queue.push_back(next);
            }
        }
    }

    // Unreachable for a genuine SCC member; degrade gracefully.
    vec![start]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_no_cycles_empty_graph() {
        let graph = DependencyGraph::new();
        assert!(detect_cycles(&graph).is_empty());
    }

    // Cycle ordering, self-imports and coverage are tested in
    // tests/integration_tests.rs against real files.
}
