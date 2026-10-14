# ADR 002: Path Resolution Algorithm

## Status

Accepted

## Context

Sass has a specific algorithm for resolving `@use`, `@forward`, and `@import` paths. We need to implement this correctly for accurate dependency graph construction.

## Decision

Implement Sass-compliant path resolution following the official specification.

## Resolution Order

For `@use "foo"` from `/project/src/main.scss`:

1. `/project/src/foo.scss`, `/project/src/_foo.scss`
2. `/project/src/foo.sass`, `/project/src/_foo.sass`
3. `/project/src/foo.css`, `/project/src/_foo.css` (`@use`/`@forward` only)
4. `/project/src/foo/index.*`, `/project/src/foo/_index.*` (same extension order)
5. Repeat for each configured load path

`@import` first tries import-only files (`foo.import.scss`, `_foo.import.scss`) and never loads `.css`. A target with an explicit extension (`"foo.scss"`) only matches `foo.scss` / `_foo.scss`. Webpack-style `"~pkg/path"` is looked up in the load paths, then in `node_modules` walking up from the importing file. `sass:` and `pkg:` URLs (and any other URL scheme) are treated as external and skipped.

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

## Update: cached directory listings (v0.2.0)

Resolution originally issued one `stat` per candidate (up to 8 per directory searched) and a `realpath` per resolved import. The resolver now caches, per directory, a listing of files and sub-directories plus the directory's canonical path (`RwLock<HashMap<PathBuf, Arc<DirListing>>>`, shared across the parallel builder threads). Candidate checks become hash lookups and a resolved path is `canonical_dir.join(name)`; only symlinked files still pay for a `realpath`. Paths are canonicalized with `dunce` so Windows does not produce `\\?\` verbatim paths, and targets are split on `/` explicitly.

Load paths given on the CLI or in the config are resolved against `--root`.

## Update: Dart Sass parity (v0.2.0)

An adversarial comparison against Dart Sass's `loadedUrls` found several divergences, so resolution now follows `resolveImportPath` from Dart Sass's `lib/src/importer/utils.dart` literally:

- `.sass`/`.scss` candidates (plain and partial) are collected together and must match exactly one file; ambiguity is reported (`reason: "ambiguous: ..."`) instead of silently picking one. `.css` is the fallback for `@use`, `@forward` **and** `@import`, including `foo.import.css`.
- Explicit extensions still honour import-only files for `@import`.
- Targets are URLs (query/fragment dropped, percent-decoded); `""` never resolves; `"."`, `".."` and trailing-slash targets only resolve to index files.
- Paths are lexical. Dart Sass does not resolve symlinks, so a symlinked file resolves its relative imports from the link's directory; canonicalizing paths made sass-dep load different files.
- Plain `.css` files are never scanned for dependencies.

`scripts/sass-parity.sh` runs the comparison on Bootstrap, Bulma, Foundation and Angular Material in CI.
