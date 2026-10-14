//! Cycle detection.
//!
//! Strongly connected components (SCCs) identify *which* files take part in
//! circular dependencies. They are computed with Kosaraju's algorithm, which
//! petgraph implements iteratively, so arbitrarily deep import chains cannot
//! overflow the stack.
//!
//! Because an SCC is an unordered set, each one is decomposed into concrete,
//! ordered, simple cycles that together cover every member. Per component
//! this costs one forward BFS tree from its smallest member `s`, one reverse
//! BFS tree towards `s`, and then, for every member `v` not yet covered,
//! splicing the tree paths `s → v` and `v → s` into a simple cycle through
//! `v`. Total work is `O(E + output)` instead of one BFS per member.

use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;

use crate::graph::{DependencyEdge, DependencyGraph, FileNode};

/// Detects cycles in the dependency graph.
///
/// # Arguments
///
/// * `graph` - The dependency graph to analyze
///
/// # Returns
///
/// A sorted vector of cycles. Each cycle lists file IDs in import order,
/// starting at its lexicographically smallest file: `cycle[i]` imports
/// `cycle[i + 1]`, and the last entry imports the first. A file that imports
/// itself is reported as a single-element cycle.
pub fn detect_cycles(graph: &DependencyGraph) -> Vec<Vec<String>> {
    cycles_from_sccs(graph, &kosaraju_scc(graph.inner()))
}

/// Decomposes precomputed SCCs into ordered cycles covering every member.
pub(crate) fn cycles_from_sccs(
    graph: &DependencyGraph,
    sccs: &[Vec<NodeIndex>],
) -> Vec<Vec<String>> {
    let inner = graph.inner();

    let mut component = vec![usize::MAX; inner.node_count()];
    for (c, scc) in sccs.iter().enumerate() {
        for node in scc {
            component[node.index()] = c;
        }
    }

    let mut covered = vec![false; inner.node_count()];
    let mut seen: HashSet<Vec<NodeIndex>> = HashSet::new();
    let mut cycles: Vec<Vec<String>> = Vec::new();

    for (c, scc) in sccs.iter().enumerate() {
        if scc.len() == 1 {
            let node = scc[0];
            if inner.contains_edge(node, node) {
                cycles.push(vec![inner[node].id.clone()]);
            }
            continue;
        }

        let mut members = scc.clone();
        members.sort_unstable_by(|a, b| inner[*a].id.cmp(&inner[*b].id));
        let source = members[0];

        let forward = bfs_tree(inner, source, Direction::Outgoing, &component, c);
        let backward = bfs_tree(inner, source, Direction::Incoming, &component, c);

        for &member in &members {
            if covered[member.index()] {
                continue;
            }
            let cycle =
                simple_cycle_through(inner, member, source, &forward, &backward, &component, c);
            for node in &cycle {
                covered[node.index()] = true;
            }

            let canonical = rotate_to_smallest(inner, cycle);
            if seen.insert(canonical.clone()) {
                cycles.push(canonical.iter().map(|n| inner[*n].id.clone()).collect());
            }
        }
    }

    cycles.sort_unstable();
    cycles
}

/// BFS tree restricted to component `c`, rooted at `root`.
///
/// With `Direction::Outgoing` the parent map yields shortest paths
/// `root → v`; with `Direction::Incoming` it yields shortest paths `v → root`
/// (each node maps to its successor on the way to the root).
struct BfsTree {
    parent: HashMap<NodeIndex, NodeIndex>,
    depth: HashMap<NodeIndex, usize>,
}

/// Builds a [`BfsTree`] from `root` following `direction`, staying inside component `c`.
fn bfs_tree(
    inner: &DiGraph<FileNode, DependencyEdge>,
    root: NodeIndex,
    direction: Direction,
    component: &[usize],
    c: usize,
) -> BfsTree {
    let mut parent = HashMap::new();
    let mut depth = HashMap::from([(root, 0)]);
    let mut queue = VecDeque::from([root]);

    while let Some(current) = queue.pop_front() {
        let next_depth = depth[&current] + 1;
        for next in inner.neighbors_directed(current, direction) {
            if component[next.index()] == c && !depth.contains_key(&next) {
                depth.insert(next, next_depth);
                parent.insert(next, current);
                queue.push_back(next);
            }
        }
    }

    BfsTree { parent, depth }
}

/// Returns a simple cycle that passes through `member`.
///
/// `p1` is the tree path `source → member` and `p2` the tree path
/// `member → source`. Both are simple; if `u` is the first node of `p2`
/// (after `member`) that also lies on `p1`, then `p1[u..=member]` followed by
/// `p2[member..u)` is a simple cycle containing `member`.
fn simple_cycle_through(
    inner: &DiGraph<FileNode, DependencyEdge>,
    member: NodeIndex,
    source: NodeIndex,
    forward: &BfsTree,
    backward: &BfsTree,
    component: &[usize],
    c: usize,
) -> Vec<NodeIndex> {
    if member == source {
        // Shortest cycle through the source: the in-component predecessor
        // closest to it in the forward tree, then the edge back.
        let closest = inner
            .neighbors_directed(source, Direction::Incoming)
            .filter(|p| component[p.index()] == c)
            .min_by_key(|p| {
                (
                    forward.depth.get(p).copied().unwrap_or(usize::MAX),
                    inner[*p].id.as_str(),
                )
            });
        return match closest {
            Some(pred) => tree_path_to(&forward.parent, source, pred),
            None => vec![source],
        };
    }

    let p1 = tree_path_to(&forward.parent, source, member);
    let positions: HashMap<NodeIndex, usize> =
        p1.iter().enumerate().map(|(i, n)| (*n, i)).collect();

    let mut tail = Vec::new();
    let mut node = member;
    while let Some(&next) = backward.parent.get(&node) {
        if let Some(&start) = positions.get(&next) {
            let mut cycle = p1[start..].to_vec();
            cycle.extend(tail);
            return cycle;
        }
        tail.push(next);
        node = next;
    }

    // Unreachable for a genuine SCC member; degrade gracefully.
    p1
}

/// Reconstructs the tree path `root → target` from a forward parent map.
fn tree_path_to(
    parent: &HashMap<NodeIndex, NodeIndex>,
    root: NodeIndex,
    target: NodeIndex,
) -> Vec<NodeIndex> {
    let mut path = vec![target];
    let mut node = target;
    while node != root {
        match parent.get(&node) {
            Some(&prev) => {
                path.push(prev);
                node = prev;
            }
            None => break,
        }
    }
    path.reverse();
    path
}

/// Rotates a cycle so it starts at its lexicographically smallest file.
fn rotate_to_smallest(
    inner: &DiGraph<FileNode, DependencyEdge>,
    mut cycle: Vec<NodeIndex>,
) -> Vec<NodeIndex> {
    if let Some(start) =
        (0..cycle.len()).min_by(|a, b| inner[cycle[*a]].id.cmp(&inner[cycle[*b]].id))
    {
        cycle.rotate_left(start);
    }
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_no_cycles_empty_graph() {
        let graph = DependencyGraph::new();
        assert!(detect_cycles(&graph).is_empty());
    }

    // Cycle ordering, coverage, self-imports and performance on large
    // components are tested in tests/integration_tests.rs against real files.
}
