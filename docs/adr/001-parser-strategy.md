# ADR 001: Parser Strategy

## Status

Accepted

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
