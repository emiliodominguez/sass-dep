//! Hand-written, single-pass SCSS scanner.
//!
//! This module extracts only dependency directives (`@use`, `@forward`,
//! `@import`) from SCSS source code. It walks the input exactly once at the
//! byte level, skipping strings, comments and `url(...)` tokens so that `@`
//! characters inside them are never mistaken for directives.
//!
//! Every byte is consumed through [`Cursor::bump`], which keeps line/column
//! tracking exact even when a directive spans several lines (for example a
//! multi-line `with (...)` configuration map).

use std::path::Path;

use super::{
    Directive, ForwardDirective, ImportDirective, Location, Namespace, ParseError, UseDirective,
    Visibility,
};

/// Parser for SCSS dependency directives.
pub struct Parser;

impl Parser {
    /// Parses SCSS source code and extracts all dependency directives.
    ///
    /// # Arguments
    ///
    /// * `input` - The SCSS source code to parse
    ///
    /// # Returns
    ///
    /// A vector of parsed directives in source order. The scanner is
    /// error-tolerant: malformed directives are skipped rather than
    /// aborting the whole file, so this currently never returns `Err`.
    ///
    /// # Example
    ///
    /// ```
    /// use sass_dep::parser::Parser;
    ///
    /// let scss = r#"
    /// @use "variables" as vars;
    /// @forward "mixins";
    /// "#;
    ///
    /// let directives = Parser::parse(scss).unwrap();
    /// assert_eq!(directives.len(), 2);
    /// ```
    pub fn parse(input: &str) -> Result<Vec<Directive>, ParseError> {
        Ok(Self::parse_bytes(input.as_bytes()))
    }

    /// Parses raw bytes and extracts all dependency directives.
    ///
    /// Invalid UTF-8 is tolerated: paths and identifiers are decoded lossily.
    pub fn parse_bytes(input: &[u8]) -> Vec<Directive> {
        let mut cursor = Cursor::new(input);
        let mut directives = Vec::new();

        while let Some(byte) = cursor.peek() {
            match byte {
                b'"' | b'\'' => {
                    cursor.bump();
                    cursor.skip_string_body(byte);
                }
                b'/' if cursor.peek_at(1) == Some(b'/') => cursor.skip_line_comment(),
                b'/' if cursor.peek_at(1) == Some(b'*') => cursor.skip_block_comment(),
                b'u' | b'U' if cursor.at_url_token() => cursor.skip_url_token(),
                b'@' => {
                    let location = cursor.location();
                    cursor.bump();
                    if let Some(directive) = parse_directive(&mut cursor, location) {
                        directives.push(directive);
                    }
                }
                _ => {
                    cursor.bump();
                }
            }
        }

        directives
    }

    /// Reads and parses a single file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SCSS file
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::Io`] if the file cannot be read.
    pub fn parse_file(path: &Path) -> Result<Vec<Directive>, ParseError> {
        let content = std::fs::read(path)?;
        Ok(Self::parse_bytes(&content))
    }
}

/// Byte cursor with exact line tracking.
#[derive(Clone)]
struct Cursor<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    line_start: usize,
}

impl<'a> Cursor<'a> {
    /// Creates a cursor at the start of `src`.
    fn new(src: &'a [u8]) -> Self {
        Self {
            src,
            pos: 0,
            line: 1,
            line_start: 0,
        }
    }

    /// Returns the current byte without consuming it.
    #[inline]
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    /// Returns the byte `offset` positions ahead without consuming anything.
    #[inline]
    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.src.get(self.pos + offset).copied()
    }

    /// Consumes one byte, updating line tracking.
    #[inline]
    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        if byte == b'\n' {
            self.line += 1;
            self.line_start = self.pos;
        }
        Some(byte)
    }

    /// Returns the 1-indexed location of the cursor (column counted in characters).
    fn location(&self) -> Location {
        let column = self.src[self.line_start..self.pos]
            .iter()
            .filter(|&&b| (b & 0xC0) != 0x80)
            .count()
            + 1;
        Location::new(self.line, column)
    }

    /// Skips the body of a string whose opening `quote` was already consumed.
    ///
    /// Stops after the closing quote, or before an unescaped newline
    /// (unterminated string), so a malformed string cannot swallow the file.
    fn skip_string_body(&mut self, quote: u8) {
        while let Some(byte) = self.peek() {
            match byte {
                b'\\' => {
                    self.bump();
                    self.bump();
                }
                b'\n' => return,
                _ => {
                    self.bump();
                    if byte == quote {
                        return;
                    }
                }
            }
        }
    }

    /// Skips a `//` comment up to (not including) the newline.
    fn skip_line_comment(&mut self) {
        while let Some(byte) = self.peek() {
            if byte == b'\n' {
                return;
            }
            self.bump();
        }
    }

    /// Skips a `/* ... */` comment including its terminator.
    fn skip_block_comment(&mut self) {
        self.bump();
        self.bump();
        while let Some(byte) = self.bump() {
            if byte == b'*' && self.peek() == Some(b'/') {
                self.bump();
                return;
            }
        }
    }

    /// Skips whitespace and comments.
    fn skip_trivia(&mut self) {
        while let Some(byte) = self.peek() {
            match byte {
                b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' => {
                    self.bump();
                }
                b'/' if self.peek_at(1) == Some(b'/') => self.skip_line_comment(),
                b'/' if self.peek_at(1) == Some(b'*') => self.skip_block_comment(),
                _ => return,
            }
        }
    }

    /// Skips spaces and tabs only (never crosses a line boundary).
    fn skip_inline_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.bump();
        }
    }

    /// Returns `true` if the cursor sits on a `url(` token that starts an identifier.
    fn at_url_token(&self) -> bool {
        let starts_ident = self.pos == 0 || !is_ident_byte(self.src[self.pos - 1]);
        starts_ident
            && self.src.len() >= self.pos + 4
            && self.src[self.pos..self.pos + 3].eq_ignore_ascii_case(b"url")
            && self.src[self.pos + 3] == b'('
    }

    /// Skips a `url(...)` token, whose unquoted body may legally contain `//` or `@`.
    fn skip_url_token(&mut self) {
        for _ in 0..3 {
            self.bump();
        }
        self.skip_balanced();
    }

    /// Consumes `keyword` case-insensitively if it is followed by a non-identifier byte.
    fn eat_keyword(&mut self, keyword: &[u8]) -> bool {
        let end = self.pos + keyword.len();
        if end > self.src.len() || !self.src[self.pos..end].eq_ignore_ascii_case(keyword) {
            return false;
        }
        if self.src.get(end).copied().is_some_and(is_ident_byte) {
            return false;
        }
        for _ in 0..keyword.len() {
            self.bump();
        }
        true
    }

    /// Reads an identifier (`[A-Za-z0-9_-]` plus any non-ASCII byte).
    fn read_ident(&mut self) -> Option<String> {
        let start = self.pos;
        while self.peek().is_some_and(is_ident_byte) {
            self.bump();
        }
        (self.pos > start).then(|| String::from_utf8_lossy(&self.src[start..self.pos]).into_owned())
    }

    /// Reads a quoted string literal, unescaping `\x` sequences to `x`.
    fn read_string(&mut self) -> Option<String> {
        let quote = self.peek().filter(|b| *b == b'"' || *b == b'\'')?;
        self.bump();

        let mut buf = Vec::new();
        while let Some(byte) = self.peek() {
            match byte {
                b'\\' => {
                    self.bump();
                    buf.push(self.bump()?);
                }
                b'\n' => return None,
                _ if byte == quote => {
                    self.bump();
                    return Some(String::from_utf8_lossy(&buf).into_owned());
                }
                _ => {
                    buf.push(byte);
                    self.bump();
                }
            }
        }

        None
    }

    /// Skips a balanced parenthesised group starting at `(`.
    ///
    /// Nested parentheses, strings and comments are handled, so
    /// `with ($map: (a: ")"))` is consumed as a single unit.
    fn skip_balanced(&mut self) -> bool {
        if self.peek() != Some(b'(') {
            return false;
        }

        let mut depth = 0usize;
        while let Some(byte) = self.peek() {
            match byte {
                b'(' => {
                    depth += 1;
                    self.bump();
                }
                b')' => {
                    self.bump();
                    depth -= 1;
                    if depth == 0 {
                        return true;
                    }
                }
                b'"' | b'\'' => {
                    self.bump();
                    self.skip_string_body(byte);
                }
                b'/' if self.peek_at(1) == Some(b'*') => self.skip_block_comment(),
                _ => {
                    self.bump();
                }
            }
        }

        false
    }

    /// Consumes an optional trailing `;`.
    fn eat_semicolon(&mut self) {
        self.skip_inline_space();
        if self.peek() == Some(b';') {
            self.bump();
        }
    }
}

/// Returns `true` for bytes that may appear in a Sass identifier.
#[inline]
fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte >= 0x80
}

/// Returns `true` if an `@import` target is a plain CSS import (not a Sass dependency).
///
/// Per the Sass spec, imports ending in `.css`, absolute URLs and
/// protocol-relative URLs are emitted as-is and never loaded by Sass.
fn is_plain_css_import(target: &str) -> bool {
    target.ends_with(".css")
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("//")
}

/// Parses the directive following an already-consumed `@`.
fn parse_directive(cursor: &mut Cursor<'_>, location: Location) -> Option<Directive> {
    if cursor.eat_keyword(b"use") {
        parse_use(cursor, location).map(Directive::Use)
    } else if cursor.eat_keyword(b"forward") {
        parse_forward(cursor, location).map(Directive::Forward)
    } else if cursor.eat_keyword(b"import") {
        parse_import(cursor, location).map(Directive::Import)
    } else {
        None
    }
}

/// Parses the remainder of `@use "<path>" [as <ns>|*] [with (...)];`.
fn parse_use(cursor: &mut Cursor<'_>, location: Location) -> Option<UseDirective> {
    cursor.skip_trivia();
    let path = cursor.read_string()?;

    let mut namespace = None;
    let mut configured = false;

    let checkpoint = cursor.clone();
    cursor.skip_trivia();
    if cursor.eat_keyword(b"as") {
        cursor.skip_trivia();
        if cursor.peek() == Some(b'*') {
            cursor.bump();
            namespace = Some(Namespace::Star);
        } else {
            namespace = cursor.read_ident().map(Namespace::Named);
        }
    } else {
        *cursor = checkpoint;
    }

    let checkpoint = cursor.clone();
    cursor.skip_trivia();
    if cursor.eat_keyword(b"with") {
        cursor.skip_trivia();
        configured = cursor.skip_balanced();
    } else {
        *cursor = checkpoint;
    }

    cursor.eat_semicolon();

    Some(UseDirective {
        path,
        namespace,
        configured,
        location,
    })
}

/// Parses the remainder of `@forward "<path>" [as prefix-*] [show|hide ...] [with (...)];`.
fn parse_forward(cursor: &mut Cursor<'_>, location: Location) -> Option<ForwardDirective> {
    cursor.skip_trivia();
    let path = cursor.read_string()?;

    let mut prefix = None;
    let mut visibility = Visibility::All;

    let checkpoint = cursor.clone();
    cursor.skip_trivia();
    if cursor.eat_keyword(b"as") {
        cursor.skip_trivia();
        let ident = cursor.read_ident();
        if cursor.peek() == Some(b'*') {
            cursor.bump();
            prefix = ident;
        }
    } else {
        *cursor = checkpoint;
    }

    let checkpoint = cursor.clone();
    cursor.skip_trivia();
    if cursor.eat_keyword(b"show") {
        visibility = Visibility::Show(read_member_list(cursor));
    } else if cursor.eat_keyword(b"hide") {
        visibility = Visibility::Hide(read_member_list(cursor));
    } else {
        *cursor = checkpoint;
    }

    let checkpoint = cursor.clone();
    cursor.skip_trivia();
    if cursor.eat_keyword(b"with") {
        cursor.skip_trivia();
        cursor.skip_balanced();
    } else {
        *cursor = checkpoint;
    }

    cursor.eat_semicolon();

    Some(ForwardDirective {
        path,
        prefix,
        visibility,
        location,
    })
}

/// Reads a comma-separated list of members (`$var`, `mixin-name`, ...).
fn read_member_list(cursor: &mut Cursor<'_>) -> Vec<String> {
    let mut members = Vec::new();

    loop {
        cursor.skip_trivia();
        let is_variable = cursor.peek() == Some(b'$');
        if is_variable {
            cursor.bump();
        }
        match cursor.read_ident() {
            Some(name) if is_variable => members.push(format!("${name}")),
            Some(name) => members.push(name),
            None => break,
        }

        let checkpoint = cursor.clone();
        cursor.skip_trivia();
        if cursor.peek() == Some(b',') {
            cursor.bump();
        } else {
            *cursor = checkpoint;
            break;
        }
    }

    members
}

/// Parses the remainder of `@import "a", "b" [media-query];`.
///
/// Plain CSS imports (`url(...)`, `*.css`, remote URLs, or any import
/// followed by a media query) are dropped because Sass never loads them.
/// Returns `None` when no Sass dependency remains.
fn parse_import(cursor: &mut Cursor<'_>, location: Location) -> Option<ImportDirective> {
    let mut paths = Vec::new();

    loop {
        cursor.skip_trivia();
        match cursor.peek() {
            Some(b'"' | b'\'') => {
                let target = cursor.read_string()?;
                if !is_plain_css_import(&target) {
                    paths.push(target);
                }
            }
            Some(b'u' | b'U') if cursor.at_url_token() => cursor.skip_url_token(),
            _ => break,
        }

        let checkpoint = cursor.clone();
        cursor.skip_trivia();
        if cursor.peek() == Some(b',') {
            cursor.bump();
        } else {
            *cursor = checkpoint;
            break;
        }
    }

    // Anything other than a terminator on the same line is a media query or
    // `supports()` condition, which turns the whole rule into plain CSS.
    cursor.skip_inline_space();
    let has_condition = !matches!(
        cursor.peek(),
        None | Some(b';' | b'\n' | b'\r' | b'}' | b'/')
    );
    if has_condition {
        return None;
    }

    cursor.eat_semicolon();

    (!paths.is_empty()).then_some(ImportDirective { paths, location })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_use() {
        let input = r#"@use "variables";"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 1);

        if let Directive::Use(use_dir) = &directives[0] {
            assert_eq!(use_dir.path, "variables");
            assert!(use_dir.namespace.is_none());
            assert!(!use_dir.configured);
        } else {
            panic!("Expected Use directive");
        }
    }

    #[test]
    fn parse_use_with_namespace() {
        let input = r#"@use "variables" as vars;"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 1);

        if let Directive::Use(use_dir) = &directives[0] {
            assert_eq!(use_dir.path, "variables");
            assert_eq!(
                use_dir.namespace,
                Some(Namespace::Named("vars".to_string()))
            );
        } else {
            panic!("Expected Use directive");
        }
    }

    #[test]
    fn parse_use_with_star() {
        let input = r#"@use "variables" as *;"#;
        let directives = Parser::parse(input).unwrap();

        if let Directive::Use(use_dir) = &directives[0] {
            assert_eq!(use_dir.namespace, Some(Namespace::Star));
        } else {
            panic!("Expected Use directive");
        }
    }

    #[test]
    fn parse_use_with_configuration() {
        let input = r#"@use "variables" with ($primary: blue);"#;
        let directives = Parser::parse(input).unwrap();

        if let Directive::Use(use_dir) = &directives[0] {
            assert!(use_dir.configured);
        } else {
            panic!("Expected Use directive");
        }
    }

    #[test]
    fn parse_use_with_nested_configuration_and_namespace() {
        let input =
            "@use \"theme\" as t with (\n  $map: (a: 1, b: \")\"),\n  $x: 2\n);\n@use \"next\";";
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 2);

        if let Directive::Use(use_dir) = &directives[0] {
            assert!(use_dir.configured);
            assert_eq!(use_dir.namespace, Some(Namespace::Named("t".to_string())));
        } else {
            panic!("Expected Use directive");
        }

        // Line tracking must survive the multi-line configuration map.
        assert_eq!(directives[1].location(), &Location::new(5, 1));
    }

    #[test]
    fn parse_simple_forward() {
        let input = r#"@forward "mixins";"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 1);

        if let Directive::Forward(fwd_dir) = &directives[0] {
            assert_eq!(fwd_dir.path, "mixins");
            assert!(fwd_dir.prefix.is_none());
            assert_eq!(fwd_dir.visibility, Visibility::All);
        } else {
            panic!("Expected Forward directive");
        }
    }

    #[test]
    fn parse_forward_with_prefix() {
        let input = r#"@forward "functions" as fn-*;"#;
        let directives = Parser::parse(input).unwrap();

        if let Directive::Forward(fwd_dir) = &directives[0] {
            assert_eq!(fwd_dir.prefix, Some("fn-".to_string()));
        } else {
            panic!("Expected Forward directive");
        }
    }

    #[test]
    fn parse_forward_with_prefix_visibility_and_config() {
        let input = r#"@forward "lib" as lib-* hide $a, b with ($c: 1 !default);"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 1);

        if let Directive::Forward(fwd_dir) = &directives[0] {
            assert_eq!(fwd_dir.prefix, Some("lib-".to_string()));
            assert_eq!(
                fwd_dir.visibility,
                Visibility::Hide(vec!["$a".to_string(), "b".to_string()])
            );
        } else {
            panic!("Expected Forward directive");
        }
    }

    #[test]
    fn parse_forward_with_show() {
        let input = r#"@forward "utils" show $var, mixin-name;"#;
        let directives = Parser::parse(input).unwrap();

        if let Directive::Forward(fwd_dir) = &directives[0] {
            assert_eq!(
                fwd_dir.visibility,
                Visibility::Show(vec!["$var".to_string(), "mixin-name".to_string()])
            );
        } else {
            panic!("Expected Forward directive");
        }
    }

    #[test]
    fn parse_forward_with_hide() {
        let input = r#"@forward "utils" hide internal, $private;"#;
        let directives = Parser::parse(input).unwrap();

        if let Directive::Forward(fwd_dir) = &directives[0] {
            assert_eq!(
                fwd_dir.visibility,
                Visibility::Hide(vec!["internal".to_string(), "$private".to_string()])
            );
        } else {
            panic!("Expected Forward directive");
        }
    }

    #[test]
    fn parse_simple_import() {
        let input = r#"@import "legacy";"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 1);

        if let Directive::Import(imp_dir) = &directives[0] {
            assert_eq!(imp_dir.paths, vec!["legacy".to_string()]);
        } else {
            panic!("Expected Import directive");
        }
    }

    #[test]
    fn parse_multiple_imports() {
        let input = r#"@import "a", "b", "c";"#;
        let directives = Parser::parse(input).unwrap();

        if let Directive::Import(imp_dir) = &directives[0] {
            assert_eq!(
                imp_dir.paths,
                vec!["a".to_string(), "b".to_string(), "c".to_string()]
            );
        } else {
            panic!("Expected Import directive");
        }
    }

    #[test]
    fn parse_import_skips_plain_css() {
        let input = r#"
@import url("https://fonts.googleapis.com/css?family=Roboto");
@import "theme.css";
@import "https://cdn.example.com/x";
@import "print" print;
@import "real", "skip.css";
"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 1);

        if let Directive::Import(imp_dir) = &directives[0] {
            assert_eq!(imp_dir.paths, vec!["real".to_string()]);
            assert_eq!(imp_dir.location.line, 6);
        } else {
            panic!("Expected Import directive");
        }
    }

    #[test]
    fn parse_mixed_directives() {
        let input = r#"
@use "variables" as vars;
@forward "mixins";
@import "legacy";
"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 3);

        assert!(matches!(directives[0], Directive::Use(_)));
        assert!(matches!(directives[1], Directive::Forward(_)));
        assert!(matches!(directives[2], Directive::Import(_)));
    }

    #[test]
    fn parse_indented_syntax_without_semicolons() {
        let input = "@use \"a\"\n@forward \"b\"\n@import \"c\"\n";
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 3);
        assert_eq!(directives[2].location().line, 3);
    }

    #[test]
    fn parse_with_comments() {
        let input = r#"
// This is a comment @use "commented";
@use "variables";
/* Multi-line
   comment @use "nope"; */
@forward "mixins";
"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 2);
        assert_eq!(directives[1].location().line, 6);
    }

    #[test]
    fn parse_ignores_other_at_rules() {
        let input = r#"
@use "variables";
@mixin foo { }
@media screen { }
@keyframes slide { }
@include user-styles;
@forward "mixins";
"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 2);
    }

    #[test]
    fn parse_requires_keyword_boundary() {
        let input = r#"@user "x"; @forwarded "y"; @imports "z";"#;
        let directives = Parser::parse(input).unwrap();
        assert!(directives.is_empty());
    }

    #[test]
    fn parse_tracks_location() {
        let input = r#"@use "variables";
@forward "mixins";"#;
        let directives = Parser::parse(input).unwrap();

        assert_eq!(directives[0].location().line, 1);
        assert_eq!(directives[0].location().column, 1);
        assert_eq!(directives[1].location().line, 2);
        assert_eq!(directives[1].location().column, 1);
    }

    #[test]
    fn parse_column_counts_characters() {
        let input = "/* é */ @use \"a\";";
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives[0].location().column, 9);
    }

    #[test]
    fn parse_single_quoted_strings() {
        let input = r#"@use 'variables';"#;
        let directives = Parser::parse(input).unwrap();

        if let Directive::Use(use_dir) = &directives[0] {
            assert_eq!(use_dir.path, "variables");
        } else {
            panic!("Expected Use directive");
        }
    }

    #[test]
    fn parse_string_in_selector_ignored() {
        let input = r#"
.foo[data-attr="@use fake"] {
    content: 'it\'s @import "x"';
}
@use "real";
"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 1);

        if let Directive::Use(use_dir) = &directives[0] {
            assert_eq!(use_dir.path, "real");
        } else {
            panic!("Expected Use directive");
        }
    }

    #[test]
    fn parse_unquoted_url_with_double_slash() {
        let input = ".a { background: url(http://x.test/a.png); } @use \"after\";";
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 1);
    }

    #[test]
    fn parse_unterminated_string_does_not_swallow_file() {
        let input = ".a { content: \"oops\n}\n@use \"ok\";";
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 1);
        assert_eq!(directives[0].location().line, 3);
    }

    #[test]
    fn parse_invalid_utf8_is_tolerated() {
        let mut input = b"@use \"a\";\n/* \xFF\xFE */\n@use \"b\";".to_vec();
        input.push(b'\n');
        let directives = Parser::parse_bytes(&input);
        assert_eq!(directives.len(), 2);
    }
}
