# ADR 002: Path Resolution Algorithm

## Status

Accepted

## Context

Sass has a specific algorithm for resolving `@use`, `@forward`, and `@import` paths. We need to implement this correctly for accurate dependency graph construction.

## Decision

Implement Sass-compliant path resolution following the official specification.

## Resolution Order

For `@use "foo"` from `/project/src/main.scss`:

1. `/project/src/foo.scss`
2. `/project/src/foo.sass`
3. `/project/src/_foo.scss`
4. `/project/src/_foo.sass`
5. `/project/src/foo/index.scss`
6. `/project/src/foo/_index.scss`
7. `/project/src/foo/index.sass`
8. `/project/src/foo/_index.sass`
9. Repeat for each configured load path

## Key Behaviors

### Extension Resolution

- Try `.scss` before `.sass` (configurable)
- Never require extension in the directive

### Partial Files

- Files starting with `_` are "partials"
- Partials are not compiled to CSS directly
- Can be imported without the underscore prefix

### Index Files

- `@use "foo"` can resolve to `foo/_index.scss`
- Allows "barrel" exports for directories

### Load Paths

- Additional directories to search
- Searched after relative resolution
- Similar to `--load-path` in Sass CLI

### Relative vs Absolute

- Paths starting with `./` or `../` are relative to importer
- Other paths try relative first, then load paths

## Implementation

```rust
pub struct ResolverConfig {
    pub load_paths: Vec<PathBuf>,
    pub extensions: Vec<String>,
}

impl Resolver {
    pub fn resolve(&self, base: &Path, target: &str) -> Result<PathBuf, ResolveError>;
}
```

## Consequences

### Positive

- Matches Sass CLI behavior
- Handles complex project structures
- Supports monorepo setups with load paths

### Negative

- Many filesystem operations per resolution
- May need caching for large projects

### Mitigations

- Short-circuit on first match
- Canonical paths prevent duplicates
- Consider adding a resolution cache

## Not Supported

- `pkg:` protocol (package imports)
- Remote URLs
- Sass indented syntax (`.sass` files parsed but not specially handled)

## Related

- [001-parser-strategy.md](001-parser-strategy.md) - Provides directive paths
- [004-graph-representation.md](004-graph-representation.md) - Uses resolved paths
