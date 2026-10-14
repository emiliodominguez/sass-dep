# ADR 001: Parser Strategy

## Status

Superseded in part (v0.2.0) — see *Update: hand-written scanner* below

## Context

We need to parse SCSS files to extract dependency directives (`@use`, `@forward`, `@import`). There are several approaches:

1. **Fork grass** - Use the full Sass parser from the `grass` crate
2. **Use tree-sitter-scss** - Use a tree-sitter grammar
3. **Custom parser with nom** - Write a minimal parser using `nom`
4. **Regex-based extraction** - Use regular expressions

## Decision

We chose to implement a **custom parser using nom**.

## Rationale

### Why not grass?

- `grass` is a full Sass compiler, not just a parser
- Significant dependency weight for our needs
- We only need to extract directive information, not evaluate Sass
- Coupling to their internal AST structure

### Why not tree-sitter-scss?

- Adds native code compilation requirements
- More complex build process
- Overkill for extracting just directives

### Why not regex?

- Fragile with edge cases (strings containing `@`, comments, etc.)
- Difficult to handle nested structures
- Hard to maintain and extend

### Why nom?

- Pure Rust, no native dependencies
- Zero-copy parsing for performance
- Composable parser combinators
- Well-suited for our limited grammar
- Easy to test individual parsers
- Good error messages

## Implementation Notes

The parser:

1. Skips over comments (single-line and multi-line)
2. Skips over string literals that might contain `@` symbols
3. Only parses `@use`, `@forward`, and `@import` directives
4. Ignores all other `@` rules (mixins, media, keyframes, etc.)
5. Tracks source locations for error reporting

## Consequences

### Positive

- Minimal dependencies
- Fast parsing
- Easy to understand and maintain
- Can be extended if needed

### Negative

- Must maintain parser ourselves
- Edge cases may be missed compared to official Sass parser
- No semantic validation (that's okay for our use case)

## Related

- [002-path-resolution.md](002-path-resolution.md) - Uses parser output

## Update: hand-written scanner (v0.2.0)

The nom-based implementation was replaced by a hand-written, single-pass byte scanner (`src/parser/lexer.rs`). The decision to write a minimal custom parser stands; only the implementation technique changed.

Problems with the nom version:

- `skip_to_at_or_end` collected the remaining input into a `Vec<char>` on every `@`, making scanning quadratic in files with many at-rules.
- Whitespace consumed inside a directive (multi-line `with (...)` maps, missing semicolons) was not counted, so later line numbers drifted.
- `with (...)` used `take_until(")")`, which broke on nested maps.
- Plain CSS imports (`url(...)`, `*.css`, media queries) were reported as dependencies.

The scanner consumes every byte through one `bump()` that tracks lines, so locations are exact; columns are counted in characters. It skips strings (with escapes, stopping at unterminated lines), comments and `url(...)` tokens, works on bytes (invalid UTF-8 is tolerated) and drops the `nom` dependency.
