# ADR 004: Graph Representation

## Status

Accepted

## Context

We need a data structure to represent the SCSS dependency graph that supports:

1. Efficient traversal
2. Cycle detection
3. Metric calculation
4. Deterministic iteration

## Decision

Use `petgraph::DiGraph` with `IndexMap` for node indexing.

## Rationale

### Why petgraph?

- Battle-tested graph library
- Rich algorithm support (Tarjan's SCC, DFS, BFS)
- Good performance characteristics
- Active maintenance

### Why DiGraph specifically?

- Directed edges (A depends on B ≠ B depends on A)
- Multiple edges between same nodes supported
- Allows self-loops (though rare in practice)

### Why IndexMap for indexing?

- Deterministic iteration order
- O(1) lookup by file ID
- Maintains insertion order
- Compatible with serde

## Data Structures

### FileNode

```rust
pub struct FileNode {
    pub id: String,           // Relative path (canonical ID)
    pub absolute_path: PathBuf,
    pub metrics: NodeMetrics,
    pub flags: Vec<NodeFlag>,
}
```

### DependencyEdge

```rust
pub struct DependencyEdge {
    pub directive_type: DirectiveType,
    pub location: Location,
    pub meta: EdgeMeta,
}
```

### DependencyGraph

```rust
pub struct DependencyGraph {
    graph: DiGraph<FileNode, DependencyEdge>,
    node_index: IndexMap<String, NodeId>,
    entry_points: HashSet<String>,
    cycles: Vec<Vec<String>>,
}
```

## Node Identification

Files are identified by their **relative path** from the project root:

- Canonical (no symlink resolution issues)
- Portable across machines
- Human-readable
- Works for output/display

Absolute paths are stored for file operations.

## Edge Semantics

An edge from A to B means "A depends on B" (A imports/uses B).

This matches the import direction:
```scss
// In A.scss
@use "B";  // Creates edge A -> B
```

## Algorithm Support

The representation supports:

1. **Cycle Detection** - `petgraph::algo::tarjan_scc`
2. **Topological Sort** - `petgraph::algo::toposort`
3. **Reachability** - DFS/BFS traversal
4. **Fan-in/out** - Node in/out degree

## Consequences

### Positive

- Efficient algorithms out of the box
- Clear separation of node data and graph structure
- Easy serialization

### Negative

- Memory overhead of petgraph structures
- Two lookups needed (ID -> NodeIndex -> Node)

### Mitigations

- Graph is typically small (hundreds to thousands of nodes)
- NodeIndex lookup is O(1) with IndexMap

## Alternatives Considered

### Adjacency List (HashMap)

- Simpler structure
- Would need manual algorithm implementation
- Less type safety

### Custom Graph

- Full control
- Significant implementation effort
- Risk of bugs in graph algorithms

## Related

- [002-path-resolution.md](002-path-resolution.md) - Provides node paths
- [003-json-schema-design.md](003-json-schema-design.md) - Serialization target
