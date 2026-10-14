//! Metric calculation for graph nodes.
//!
//! All metrics are computed in (near-)linear time:
//!
//! - fan-in / fan-out: one pass over the adjacency lists,
//! - depth: one multi-source BFS from all entry points,
//! - transitive dependencies / dependents: dynamic programming over the
//!   SCC condensation (a DAG) with word-packed bitsets. Memory is bounded by
//!   processing node bits in chunks, so very large graphs trade a few extra
//!   passes for a fixed memory budget instead of quadratic memory.

use std::collections::VecDeque;

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;

use crate::graph::{DependencyEdge, DependencyGraph, FileNode};

/// Upper bound for the reachability bitset matrix, in 64-bit words (64 MiB).
const BITSET_BUDGET_WORDS: usize = 8 * 1024 * 1024;

/// Calculates fan-in and fan-out for all nodes.
///
/// - Fan-in: Number of files that depend on this file (in-degree)
/// - Fan-out: Number of files this file depends on (out-degree)
pub fn calculate_fan_in_out(graph: &mut DependencyGraph) {
    let inner = graph.inner_mut();
    let degrees: Vec<(usize, usize)> = inner
        .node_indices()
        .map(|idx| {
            (
                inner.neighbors_directed(idx, Direction::Incoming).count(),
                inner.neighbors_directed(idx, Direction::Outgoing).count(),
            )
        })
        .collect();

    for (node, (fan_in, fan_out)) in inner.node_weights_mut().zip(degrees) {
        node.metrics.fan_in = fan_in;
        node.metrics.fan_out = fan_out;
    }
}

/// Calculates depth from entry points using a multi-source BFS.
///
/// Depth is the shortest distance from any entry point to a node.
/// Entry points have depth `Some(0)`; unreachable nodes get `None`.
pub fn calculate_depths(graph: &mut DependencyGraph) {
    let sources: Vec<NodeIndex> = graph
        .entry_points()
        .iter()
        .filter_map(|id| graph.index_of(id))
        .collect();

    let inner = graph.inner_mut();
    let mut depths: Vec<Option<usize>> = vec![None; inner.node_count()];
    let mut queue = VecDeque::with_capacity(inner.node_count());

    for idx in sources {
        if depths[idx.index()].is_none() {
            depths[idx.index()] = Some(0);
            queue.push_back(idx);
        }
    }

    while let Some(idx) = queue.pop_front() {
        let next_depth = depths[idx.index()].map(|d| d + 1);
        for neighbor in inner.neighbors_directed(idx, Direction::Outgoing) {
            if depths[neighbor.index()].is_none() {
                depths[neighbor.index()] = next_depth;
                queue.push_back(neighbor);
            }
        }
    }

    for (node, depth) in inner.node_weights_mut().zip(depths) {
        node.metrics.depth = depth;
    }
}

/// Calculates transitive dependencies and dependents for all nodes.
///
/// - `transitive_deps`: files this file depends on, directly or indirectly
/// - `transitive_dependents`: files that depend on this file, directly or indirectly
///
/// Files inside a cycle reach themselves, so they are counted in their own totals.
pub fn calculate_transitive_deps(graph: &mut DependencyGraph) {
    let sccs = kosaraju_scc(graph.inner());
    calculate_transitive_from_sccs(graph, &sccs);
}

/// Computes transitive metrics from precomputed SCCs (in Kosaraju order: reverse topological).
pub(crate) fn calculate_transitive_from_sccs(graph: &mut DependencyGraph, sccs: &[Vec<NodeIndex>]) {
    let inner = graph.inner();
    let condensation = Condensation::new(inner, sccs);

    let (deps, dependents) = rayon::join(
        || condensation.reach_counts(inner, sccs, Direction::Outgoing),
        || condensation.reach_counts(inner, sccs, Direction::Incoming),
    );

    let component = condensation.component;
    for (idx, node) in graph.inner_mut().node_weights_mut().enumerate() {
        node.metrics.transitive_deps = deps[component[idx]];
        node.metrics.transitive_dependents = dependents[component[idx]];
    }
}

/// SCC condensation of the dependency graph.
struct Condensation {
    /// Component index for each node index.
    component: Vec<usize>,
    /// Whether each component contains a cycle (size > 1 or a self-import).
    cyclic: Vec<bool>,
}

impl Condensation {
    /// Builds the node-to-component mapping.
    fn new(inner: &DiGraph<FileNode, DependencyEdge>, sccs: &[Vec<NodeIndex>]) -> Self {
        let mut component = vec![0; inner.node_count()];
        let mut cyclic = vec![false; sccs.len()];

        for (c, scc) in sccs.iter().enumerate() {
            for node in scc {
                component[node.index()] = c;
            }
            cyclic[c] = scc.len() > 1 || inner.contains_edge(scc[0], scc[0]);
        }

        Self { component, cyclic }
    }

    /// Counts, per component, the nodes reachable in `direction`.
    ///
    /// Kosaraju emits SCCs in reverse topological order, so for outgoing
    /// reachability every successor component is finalized before its
    /// predecessors; incoming reachability walks the list backwards.
    fn reach_counts(
        &self,
        inner: &DiGraph<FileNode, DependencyEdge>,
        sccs: &[Vec<NodeIndex>],
        direction: Direction,
    ) -> Vec<usize> {
        let node_count = inner.node_count();
        let comp_count = sccs.len();
        if comp_count == 0 {
            return Vec::new();
        }

        // Deduplicated neighbour components, computed once for all chunks.
        let mut seen = vec![usize::MAX; comp_count];
        let neighbours: Vec<Vec<usize>> = sccs
            .iter()
            .enumerate()
            .map(|(c, scc)| {
                let mut out = Vec::new();
                for &node in scc {
                    for next in inner.neighbors_directed(node, direction) {
                        let d = self.component[next.index()];
                        if d != c && seen[d] != c {
                            seen[d] = c;
                            out.push(d);
                        }
                    }
                }
                out
            })
            .collect();

        let order: Vec<usize> = match direction {
            Direction::Outgoing => (0..comp_count).collect(),
            Direction::Incoming => (0..comp_count).rev().collect(),
        };

        let total_words = node_count.div_ceil(64);
        let chunk_words = (BITSET_BUDGET_WORDS / comp_count).clamp(1, total_words.max(1));
        let chunk_bits = chunk_words * 64;

        let mut counts = vec![0usize; comp_count];
        let mut reach = vec![0u64; comp_count * chunk_words];
        let mut acc = vec![0u64; chunk_words];

        for lo in (0..node_count).step_by(chunk_bits) {
            let hi = (lo + chunk_bits).min(node_count);
            reach.fill(0);

            for &c in &order {
                acc.fill(0);
                for &d in &neighbours[c] {
                    let row = &reach[d * chunk_words..(d + 1) * chunk_words];
                    for (a, r) in acc.iter_mut().zip(row) {
                        *a |= r;
                    }
                }

                counts[c] += acc.iter().map(|w| w.count_ones() as usize).sum::<usize>();

                for node in &sccs[c] {
                    let i = node.index();
                    if (lo..hi).contains(&i) {
                        let bit = i - lo;
                        acc[bit / 64] |= 1 << (bit % 64);
                        if self.cyclic[c] {
                            counts[c] += 1;
                        }
                    }
                }

                reach[c * chunk_words..(c + 1) * chunk_words].copy_from_slice(&acc);
            }
        }

        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_metrics() {
        let mut graph = DependencyGraph::new();
        calculate_fan_in_out(&mut graph);
        calculate_depths(&mut graph);
        calculate_transitive_deps(&mut graph);
        assert_eq!(graph.node_count(), 0);
    }
}
