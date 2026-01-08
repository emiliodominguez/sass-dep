# ADR 003: JSON Schema Design

## Status

Accepted

## Context

We need to define a JSON output format that:

1. Is stable and versioned
2. Contains all analysis results
3. Is easy to consume by other tools
4. Is deterministic across runs

## Decision

Design a versioned JSON schema with explicit structure for nodes, edges, and analysis results.

## Schema Structure

```json
{
  "$schema": "...",
  "version": "1.0.0",
  "metadata": {},
  "nodes": {},
  "edges": [],
  "analysis": {}
}
```

### Version Strategy

- Semantic versioning: `MAJOR.MINOR.PATCH`
- Major: Breaking changes to schema structure
- Minor: New optional fields
- Patch: Documentation or clarification

### Metadata

```json
{
  "generated_at": "ISO 8601 timestamp",
  "root": "absolute path to project root",
  "sass_dep_version": "tool version"
}
```

### Nodes

Map of file ID to node data:

```json
{
  "src/main.scss": {
    "path": "/absolute/path",
    "metrics": {
      "fan_in": 0,
      "fan_out": 2,
      "depth": 0,
      "transitive_deps": 5
    },
    "flags": ["entry_point"]
  }
}
```

Using a map instead of array:
- O(1) lookup by ID
- Natural deduplication
- Easier to merge results

### Edges

Array of edge objects:

```json
{
  "from": "source file ID",
  "to": "target file ID",
  "directive_type": "use|forward|import",
  "location": { "line": 1, "column": 1 },
  "namespace": "optional namespace",
  "configured": false
}
```

Using array instead of adjacency list:
- Edges have metadata
- Easier to filter/map
- Cleaner JSON structure

### Analysis

```json
{
  "cycles": [["a.scss", "b.scss", "c.scss"]],
  "statistics": {
    "total_files": 10,
    "total_dependencies": 15,
    ...
  }
}
```

## Determinism

To ensure identical output across runs:

1. Use `IndexMap` instead of `HashMap` for ordered iteration
2. Sort nodes by key alphabetically
3. Sort edges by (from, to, line)
4. Use consistent floating-point formatting
5. Pretty-print with consistent indentation

## Consequences

### Positive

- Clear contract for consumers
- Version allows evolution
- Deterministic output enables diffing
- Self-documenting with `$schema`

### Negative

- Schema must be maintained
- Version bumps require documentation
- IndexMap slightly slower than HashMap

## Future Considerations

- JSON Schema validation file
- Support for YAML output
- Streaming output for large graphs

## Related

- [004-graph-representation.md](004-graph-representation.md) - Internal representation
