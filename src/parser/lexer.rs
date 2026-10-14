//! Hand-written, single-pass Sass scanner.
//!
//! This module extracts only dependency directives (`@use`, `@forward`,
//! `@import`) from Sass source code. It walks the input exactly once at the
//! byte level and skips every construct that may legally contain an `@` or
//! a quote without starting a directive:
//!
//! - strings, including `#{...}` interpolation with nested strings,
//! - silent (`//`) and loud (`/* */`) comments, including indentation-based
//!   comments in the indented (`.sass`) syntax,
//! - unquoted `url(...)` tokens (which may contain `//`, `/*` and escapes),
//! - backslash escapes outside strings,
//! - custom property values (`--x: @import "a"` is raw CSS, not a rule).
//!
//! Directive names are case-sensitive, as in Dart Sass (`@IMPORT` is an
//! unknown CSS at-rule). Every byte is consumed through [`Cursor::bump`],
//! which tracks lines (`\n`, `\r\n` and lone `\r`) so locations stay exact;
//! columns are counted in UTF-16 code units like Dart Sass reports them.
//!
//! All loops are iterative (nesting is tracked with explicit stacks), so
//! hostile input can neither overflow the stack nor make scanning
//! super-linear.

use std::path::Path;

use super::{
    Directive, ForwardDirective, ImportDirective, Location, Namespace, ParseError, UseDirective,
    Visibility,
};

/// Source syntax of a stylesheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    /// SCSS (`.scss`).
    Scss,
    /// Indented syntax (`.sass`).
    Sass,
    /// Plain CSS (`.css`): Sass never follows its `@import`s.
    Css,
}

impl Syntax {
    /// Detects the syntax from a file extension (SCSS for anything unknown).
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("sass") => Syntax::Sass,
            Some(ext) if ext.eq_ignore_ascii_case("css") => Syntax::Css,
            _ => Syntax::Scss,
        }
    }
}

/// Parser for Sass dependency directives.
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

    /// Parses raw SCSS bytes and extracts all dependency directives.
    ///
    /// Invalid UTF-8 is tolerated: paths and identifiers are decoded lossily.
    pub fn parse_bytes(input: &[u8]) -> Vec<Directive> {
        Self::parse_with(input, Syntax::Scss)
    }

    /// Parses raw bytes in the given syntax.
    pub fn parse_with(input: &[u8], syntax: Syntax) -> Vec<Directive> {
        let mut directives = Vec::new();
        Self::scan(input, syntax, |directive| directives.push(directive));
        directives
    }

    /// Streams every dependency directive to `visit`, in source order.
    ///
    /// This avoids materializing all directives of a file at once, which
    /// keeps memory bounded on files with millions of directives.
    pub fn scan(input: &[u8], syntax: Syntax, mut visit: impl FnMut(Directive)) {
        if syntax == Syntax::Css {
            return;
        }

        let mut cursor = Cursor::new(input, syntax);

        while let Some(byte) = cursor.peek() {
            match byte {
                b'"' | b'\'' => {
                    cursor.bump();
                    cursor.skip_string_body(byte);
                }
                b'\\' => {
                    cursor.bump();
                    cursor.bump();
                }
                b'/' if cursor.peek_at(1) == Some(b'/') => cursor.skip_silent_comment(),
                b'/' if cursor.peek_at(1) == Some(b'*') => cursor.skip_loud_comment(),
                b'u' | b'U' if cursor.at_url_token() => {
                    cursor.bump_n(4);
                    cursor.try_unquoted_url();
                }
                b'-' if cursor.at_custom_property() => cursor.skip_custom_property(),
                b'@' => {
                    let location = cursor.location();
                    cursor.bump();
                    if let Some(directive) = parse_directive(&mut cursor, location) {
                        visit(directive);
                    }
                }
                _ => {
                    cursor.bump();
                }
            }
        }
    }

    /// Reads and parses a single file, detecting the syntax from its extension.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Sass file
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::Io`] if the file cannot be read.
    pub fn parse_file(path: &Path) -> Result<Vec<Directive>, ParseError> {
        let content = std::fs::read(path)?;
        Ok(Self::parse_with(&content, Syntax::from_path(path)))
    }
}

/// Byte cursor with exact line tracking.
#[derive(Clone)]
struct Cursor<'a> {
    src: &'a [u8],
    syntax: Syntax,
    pos: usize,
    line: usize,
    line_start: usize,
    /// Byte offset up to which `col_units` has been counted on the current line.
    col_pos: usize,
    /// UTF-16 code units between `line_start` and `col_pos`.
    col_units: usize,
}

impl<'a> Cursor<'a> {
    /// Creates a cursor at the start of `src`.
    fn new(src: &'a [u8], syntax: Syntax) -> Self {
        Self {
            src,
            syntax,
            pos: 0,
            line: 1,
            line_start: 0,
            col_pos: 0,
            col_units: 0,
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

    /// Consumes one byte, updating line tracking (`\n`, `\r\n`, lone `\r`).
    #[inline]
    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        let newline = byte == b'\n' || (byte == b'\r' && self.peek() != Some(b'\n'));
        if newline {
            self.line += 1;
            self.line_start = self.pos;
        }
        Some(byte)
    }

    /// Consumes `n` bytes.
    fn bump_n(&mut self, n: usize) {
        for _ in 0..n {
            self.bump();
        }
    }

    /// Returns the 1-indexed location of the cursor.
    ///
    /// Columns are counted in UTF-16 code units (like Dart Sass) and
    /// incrementally from the previous call on the same line, so minified
    /// single-line files stay linear.
    fn location(&mut self) -> Location {
        if self.col_pos < self.line_start || self.col_pos > self.pos {
            self.col_pos = self.line_start;
            self.col_units = 0;
        }
        self.col_units += self.src[self.col_pos..self.pos]
            .iter()
            .map(|&b| match b {
                0x80..=0xBF => 0,
                0xF0..=0xFF => 2,
                _ => 1,
            })
            .sum::<usize>();
        self.col_pos = self.pos;
        Location::new(self.line, self.col_units + 1)
    }

    /// Returns `true` at a line break byte.
    fn at_newline(&self) -> bool {
        matches!(self.peek(), Some(b'\n' | b'\r' | b'\x0C'))
    }

    /// Skips the rest of a string whose opening `quote` was already consumed.
    ///
    /// `#{...}` interpolation (which may itself contain strings and braces)
    /// is tracked with an explicit stack. An unescaped line break ends an
    /// unterminated string, so a malformed string cannot swallow the file.
    fn skip_string_body(&mut self, quote: u8) {
        self.skip_nested(vec![quote]);
    }

    /// Runs the string/interpolation state machine until `stack` is empty.
    ///
    /// Stack entries are quote bytes (inside a string) or `{` (inside an
    /// interpolation or a brace nested in one).
    fn skip_nested(&mut self, mut stack: Vec<u8>) {
        while let Some(&top) = stack.last() {
            let Some(byte) = self.peek() else {
                return;
            };

            if top == b'{' {
                match byte {
                    b'}' => {
                        self.bump();
                        stack.pop();
                    }
                    b'{' | b'"' | b'\'' => {
                        self.bump();
                        stack.push(byte);
                    }
                    b'\\' => self.bump_n(2),
                    _ => {
                        self.bump();
                    }
                }
                continue;
            }

            match byte {
                b'\\' => self.bump_n(2),
                b'\n' | b'\r' | b'\x0C' => return,
                b'#' if self.peek_at(1) == Some(b'{') => {
                    self.bump_n(2);
                    stack.push(b'{');
                }
                _ if byte == top => {
                    self.bump();
                    stack.pop();
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    /// Skips to the end of the current line (not consuming the line break).
    fn skip_to_line_end(&mut self) {
        while self.peek().is_some() && !self.at_newline() {
            self.bump();
        }
    }

    /// Skips a `//` comment.
    ///
    /// In the indented syntax a comment also owns every following line that
    /// is indented deeper than the line the comment starts on.
    fn skip_silent_comment(&mut self) {
        if self.syntax == Syntax::Sass {
            self.skip_indented_block();
        } else {
            self.skip_to_line_end();
        }
    }

    /// Skips a `/* ... */` comment (indentation-based in the indented syntax).
    fn skip_loud_comment(&mut self) {
        if self.syntax == Syntax::Sass {
            self.skip_indented_block();
            return;
        }

        self.bump_n(2);
        while let Some(byte) = self.bump() {
            if byte == b'*' && self.peek() == Some(b'/') {
                self.bump();
                return;
            }
        }
    }

    /// Skips the current line plus all following blank or deeper-indented lines.
    fn skip_indented_block(&mut self) {
        let base = indentation(&self.src[self.line_start..]);
        self.skip_to_line_end();

        loop {
            let checkpoint = self.clone();
            if !self.at_newline() {
                return;
            }
            self.bump();
            if self.peek() == Some(b'\n') && checkpoint.peek() == Some(b'\r') {
                self.bump();
            }

            let rest = &self.src[self.pos..];
            let indent = indentation(rest);
            let blank = matches!(rest.get(indent), None | Some(b'\n' | b'\r'));
            if blank || indent > base {
                self.skip_to_line_end();
            } else {
                *self = checkpoint;
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
                b'/' if self.peek_at(1) == Some(b'/') => self.skip_silent_comment(),
                b'/' if self.peek_at(1) == Some(b'*') => self.skip_loud_comment(),
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

    /// Returns `true` if the previous byte cannot continue an identifier.
    fn at_ident_start(&self) -> bool {
        self.pos == 0 || !is_ident_byte(self.src[self.pos - 1])
    }

    /// Returns `true` if the cursor sits on a `url(` token that starts an identifier.
    fn at_url_token(&self) -> bool {
        self.at_ident_start()
            && self.src.len() >= self.pos + 4
            && self.src[self.pos..self.pos + 3].eq_ignore_ascii_case(b"url")
            && self.src[self.pos + 3] == b'('
    }

    /// Consumes the body of an unquoted `url(...)` whose `url(` was consumed.
    ///
    /// Mirrors Dart Sass's `_tryUrlContents`: escapes are honoured, `//` and
    /// `/*` are literal, and the token ends at the first unescaped `)`. If
    /// the contents are not a valid unquoted URL (a quote, a nested `(` or
    /// junk after trailing whitespace), the cursor is restored and `false`
    /// is returned so the caller scans the arguments as ordinary code.
    fn try_unquoted_url(&mut self) -> bool {
        let checkpoint = self.clone();
        self.skip_whitespace();

        while let Some(byte) = self.peek() {
            match byte {
                b'\\' => self.bump_n(2),
                b')' => {
                    self.bump();
                    return true;
                }
                b'#' if self.peek_at(1) == Some(b'{') => {
                    self.bump_n(2);
                    self.skip_nested(vec![b'{']);
                }
                b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' => {
                    self.skip_whitespace();
                    if self.peek() == Some(b')') {
                        self.bump();
                        return true;
                    }
                    break;
                }
                b'"' | b'\'' | b'(' => break,
                _ => {
                    self.bump();
                }
            }
        }

        *self = checkpoint;
        false
    }

    /// Skips whitespace including line breaks.
    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0C')) {
            self.bump();
        }
    }

    /// Returns `true` at the start of a custom property declaration (`--name:`).
    fn at_custom_property(&self) -> bool {
        if !self.at_ident_start() || self.peek_at(1) != Some(b'-') {
            return false;
        }
        let mut i = self.pos + 2;
        while self.src.get(i).copied().is_some_and(is_ident_byte) {
            i += 1;
        }
        while matches!(self.src.get(i), Some(b' ' | b'\t')) {
            i += 1;
        }
        self.src.get(i) == Some(&b':')
    }

    /// Skips a custom property's name and raw value.
    ///
    /// The value ends at `;`, at an unmatched `}`/`)`/`]` or at the end of
    /// input (and at a line break in the indented syntax), with strings,
    /// comments and brackets tracked iteratively.
    fn skip_custom_property(&mut self) {
        while self.peek() != Some(b':') {
            self.bump();
        }
        self.bump();

        let mut depth = 0usize;
        while let Some(byte) = self.peek() {
            match byte {
                b'"' | b'\'' => {
                    self.bump();
                    self.skip_string_body(byte);
                }
                b'\\' => self.bump_n(2),
                b'/' if self.peek_at(1) == Some(b'*') => self.skip_loud_comment(),
                b'(' | b'[' | b'{' => {
                    depth += 1;
                    self.bump();
                }
                b')' | b']' | b'}' => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                    self.bump();
                }
                b';' if depth == 0 => return,
                b'\n' | b'\r' if depth == 0 && self.syntax == Syntax::Sass => return,
                _ => {
                    self.bump();
                }
            }
        }
    }

    /// Consumes `keyword` (case-sensitively) if it is followed by a non-identifier byte.
    fn eat_keyword(&mut self, keyword: &[u8]) -> bool {
        let end = self.pos + keyword.len();
        if end > self.src.len() || &self.src[self.pos..end] != keyword {
            return false;
        }
        if self.src.get(end).copied().is_some_and(is_ident_byte) {
            return false;
        }
        self.bump_n(keyword.len());
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

    /// Reads a quoted string literal.
    ///
    /// CSS escapes are decoded: `\66` (1-6 hex digits, plus one optional
    /// whitespace) becomes the code point, an escaped line break is removed
    /// and any other escaped character stands for itself. Returns `None` for
    /// an unterminated string, and [`StringLiteral::Interpolated`] (with the
    /// whole literal skipped) when it contains `#{...}`.
    fn read_string(&mut self) -> Option<StringLiteral> {
        let quote = self.peek().filter(|b| *b == b'"' || *b == b'\'')?;
        self.bump();

        let mut buf = Vec::new();
        while let Some(byte) = self.peek() {
            match byte {
                b'\\' => {
                    self.bump();
                    self.read_escape(&mut buf)?;
                }
                b'#' if self.peek_at(1) == Some(b'{') => {
                    self.bump_n(2);
                    self.skip_nested(vec![quote, b'{']);
                    return Some(StringLiteral::Interpolated);
                }
                b'\n' | b'\r' | b'\x0C' => return None,
                _ if byte == quote => {
                    self.bump();
                    return Some(StringLiteral::Plain(
                        String::from_utf8_lossy(&buf).into_owned(),
                    ));
                }
                _ => {
                    buf.push(byte);
                    self.bump();
                }
            }
        }

        None
    }

    /// Decodes the escape following a consumed backslash into `buf`.
    fn read_escape(&mut self, buf: &mut Vec<u8>) -> Option<()> {
        let first = self.peek()?;

        if first.is_ascii_hexdigit() {
            let mut value: u32 = 0;
            for _ in 0..6 {
                match self.peek() {
                    Some(digit) if digit.is_ascii_hexdigit() => {
                        value = value * 16 + (digit as char).to_digit(16).unwrap_or(0);
                        self.bump();
                    }
                    _ => break,
                }
            }
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\x0C') => {
                    self.bump();
                }
                Some(b'\r') => {
                    self.bump();
                    if self.peek() == Some(b'\n') {
                        self.bump();
                    }
                }
                _ => {}
            }
            let ch = char::from_u32(value)
                .filter(|c| *c != '\0')
                .unwrap_or(char::REPLACEMENT_CHARACTER);
            let mut utf8 = [0; 4];
            buf.extend_from_slice(ch.encode_utf8(&mut utf8).as_bytes());
            return Some(());
        }

        match first {
            b'\n' | b'\x0C' => {
                self.bump();
            }
            b'\r' => {
                self.bump();
                if self.peek() == Some(b'\n') {
                    self.bump();
                }
            }
            _ => {
                buf.push(first);
                self.bump();
            }
        }
        Some(())
    }

    /// Skips a balanced parenthesised group starting at `(`.
    ///
    /// Nested parentheses, strings (with interpolation), escapes and both
    /// comment styles are handled, so `with ($map: (a: ")"), // note (` is
    /// consumed as a single unit.
    fn skip_balanced(&mut self) -> bool {
        if self.peek() != Some(b'(') {
            return false;
        }
        self.bump();
        self.skip_balanced_inside()
    }

    /// Skips to the `)` closing an already-consumed `(`.
    fn skip_balanced_inside(&mut self) -> bool {
        let mut depth = 1usize;
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
                b'\\' => self.bump_n(2),
                b'/' if self.peek_at(1) == Some(b'/') => self.skip_to_line_end(),
                b'/' if self.peek_at(1) == Some(b'*') => self.skip_loud_comment(),
                b'u' | b'U' if self.at_url_token() => {
                    self.bump_n(4);
                    if !self.try_unquoted_url() {
                        depth += 1;
                    }
                }
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

/// A quoted string read by [`Cursor::read_string`].
enum StringLiteral {
    /// A string without interpolation.
    Plain(String),
    /// A string containing `#{...}`; its value is only known at compile time.
    Interpolated,
}

/// Counts the leading spaces and tabs of a line.
fn indentation(line: &[u8]) -> usize {
    line.iter()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count()
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

/// Reads the URL string of `@use`/`@forward` (interpolation is not allowed there).
fn read_module_url(cursor: &mut Cursor<'_>) -> Option<String> {
    cursor.skip_trivia();
    match cursor.read_string()? {
        StringLiteral::Plain(path) => Some(path),
        StringLiteral::Interpolated => None,
    }
}

/// Parses the remainder of `@use "<path>" [as <ns>|*] [with (...)];`.
fn parse_use(cursor: &mut Cursor<'_>, location: Location) -> Option<UseDirective> {
    let path = read_module_url(cursor)?;

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
    let path = read_module_url(cursor)?;

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

/// Reads an unquoted `@import` argument (indented syntax only).
fn read_unquoted_import(cursor: &mut Cursor<'_>) -> Option<String> {
    let start = cursor.pos;
    while cursor
        .peek()
        .is_some_and(|b| !matches!(b, b',' | b';' | b' ' | b'\t' | b'\n' | b'\r' | b'\x0C'))
    {
        cursor.bump();
    }
    let text = String::from_utf8_lossy(&cursor.src[start..cursor.pos]).into_owned();
    (!text.is_empty()).then_some(text)
}

/// Parses the remainder of `@import "a", "b" [modifiers];`.
///
/// Plain CSS imports (`url(...)`, `*.css`, remote URLs, interpolated URLs,
/// or any import followed by modifiers such as a media query, `supports()`
/// or `layer()`) are dropped because Sass never loads them. Returns `None`
/// when no Sass dependency remains.
fn parse_import(cursor: &mut Cursor<'_>, location: Location) -> Option<ImportDirective> {
    let mut paths = Vec::new();

    loop {
        cursor.skip_trivia();
        match cursor.peek() {
            Some(b'"' | b'\'') => match cursor.read_string()? {
                StringLiteral::Plain(target) if !is_plain_css_import(&target) => paths.push(target),
                _ => {}
            },
            Some(b'u' | b'U') if cursor.at_url_token() => {
                cursor.bump_n(4);
                if !cursor.try_unquoted_url() {
                    cursor.skip_balanced_inside();
                }
            }
            Some(_) if cursor.syntax == Syntax::Sass => match read_unquoted_import(cursor) {
                Some(target) if !is_plain_css_import(&target) => paths.push(target),
                Some(_) => {}
                None => break,
            },
            _ => break,
        }

        let checkpoint = cursor.clone();
        if cursor.syntax == Syntax::Sass {
            cursor.skip_inline_space();
        } else {
            cursor.skip_trivia();
        }
        if cursor.peek() == Some(b',') {
            cursor.bump();
        } else {
            *cursor = checkpoint;
            break;
        }
    }

    // Anything but a terminator after the URLs is a modifier (media query,
    // `supports()`, `layer()`), which turns the whole rule into plain CSS.
    // SCSS ends rules with `;`/`}`; the indented syntax ends them at a line break.
    let terminated = if cursor.syntax == Syntax::Sass {
        cursor.skip_inline_space();
        if cursor.peek() == Some(b'/') && cursor.peek_at(1) == Some(b'*') {
            cursor.skip_loud_comment();
        }
        matches!(cursor.peek(), None | Some(b'\n' | b'\r' | b';'))
            || (cursor.peek() == Some(b'/') && cursor.peek_at(1) == Some(b'/'))
    } else {
        cursor.skip_trivia();
        matches!(cursor.peek(), None | Some(b';' | b'}'))
    };
    if !terminated {
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
        let input = b"@use \"a\"\n@forward \"b\"\n@import \"c\"\n";
        let directives = Parser::parse_with(input, Syntax::Sass);
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
    fn parse_columns_on_one_long_line() {
        let input = "@use \"a\"; .é{} @use \"b\"; @import \"c\";";
        let directives = Parser::parse(input).unwrap();
        let columns: Vec<usize> = directives.iter().map(|d| d.location().column).collect();
        assert_eq!(columns, vec![1, 16, 26]);
        assert!(directives.iter().all(|d| d.location().line == 1));
    }

    /// Deterministic fuzzing over the syntax-significant alphabet: the scanner
    /// must never panic, and every location must point inside the input.
    #[test]
    fn fuzz_scanner_never_panics() {
        const PIECES: &[&[u8]] = &[
            b"@",
            b"@use",
            b"@forward",
            b"@import",
            b" ",
            b"\n",
            b"\"",
            b"'",
            b"\\",
            b"/",
            b"*",
            b"(",
            b")",
            b";",
            b",",
            b"as",
            b"with",
            b"show",
            b"hide",
            b"url(",
            b"$x",
            b"-*",
            b"a",
            b"\xC3\xA9",
            b"\xFF",
            b"{",
            b"}",
            b".css",
            b"//",
            b"/*",
            b"*/",
        ];

        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };

        for _ in 0..20_000 {
            let len = (next() % 40) as usize;
            let mut input = Vec::new();
            for _ in 0..len {
                input.extend_from_slice(PIECES[(next() % PIECES.len() as u64) as usize]);
            }

            let lines = input.iter().filter(|&&b| b == b'\n').count() + 1;
            for directive in Parser::parse_bytes(&input) {
                let location = directive.location();
                assert!(location.line >= 1 && location.line <= lines, "{input:?}");
                assert!(
                    location.column >= 1 && location.column <= input.len().max(1),
                    "{input:?}"
                );
            }
        }
    }

    #[test]
    fn parse_invalid_utf8_is_tolerated() {
        let mut input = b"@use \"a\";\n/* \xFF\xFE */\n@use \"b\";".to_vec();
        input.push(b'\n');
        let directives = Parser::parse_bytes(&input);
        assert_eq!(directives.len(), 2);
    }

    /// Parses `input` as SCSS and returns every dependency target.
    fn targets(input: &str) -> Vec<String> {
        targets_with(input, Syntax::Scss)
    }

    /// Parses `input` in `syntax` and returns every dependency target.
    fn targets_with(input: &str, syntax: Syntax) -> Vec<String> {
        Parser::parse_with(input.as_bytes(), syntax)
            .iter()
            .flat_map(|d| d.paths().into_iter().map(str::to_string))
            .collect()
    }

    #[test]
    fn line_comment_inside_with_configuration() {
        let input = "@use \"cfg\" with (\n  $a: 1, // see (docs\n  $b: 2 // tokens/*.scss\n);\n@use \"other\";";
        assert_eq!(targets(input), vec!["cfg", "other"]);
    }

    #[test]
    fn unquoted_url_with_escapes_and_comment_markers() {
        assert_eq!(
            targets("a { b: url(x\\(.png); }\n@import \"foo\";"),
            vec!["foo"]
        );
        assert_eq!(
            targets("a{b:url(/img/*/x.png);}\n@import \"foo\";"),
            vec!["foo"]
        );
        assert_eq!(
            targets("@import url(a\\(b.css);\n@import \"foo\";"),
            vec!["foo"]
        );
        assert_eq!(targets("a{b:url( x.png );}@import \"foo\";"), vec!["foo"]);
        // A quoted url() is an ordinary function call.
        assert_eq!(targets("a{b:url(\"x)\");}@import \"foo\";"), vec!["foo"]);
    }

    #[test]
    fn media_queries_after_trivia_make_imports_plain_css() {
        assert!(targets("@import \"a\"\n  screen;").is_empty());
        assert!(targets("@import \"a\" /* c */ screen;").is_empty());
        assert!(targets("@import \"a\"\r  screen;").is_empty());
        assert!(targets("@import \"a\" supports(display: grid);").is_empty());
        assert_eq!(
            targets("@import \"a\" /* c */ ;\n.x { @import \"b\" }"),
            vec!["a", "b"]
        );
    }

    #[test]
    fn directive_names_are_case_sensitive() {
        assert!(targets("@IMPORT \"a\"; @Use \"b\"; @FORWARD \"c\";").is_empty());
    }

    #[test]
    fn custom_property_values_are_raw() {
        assert_eq!(
            targets(".a { --x: @import \"foo\"; --y: { @use \"z\" }; } @use \"real\";"),
            vec!["real"]
        );
        assert_eq!(
            targets("@include m(--x: 1) { @use \"inner\"; }"),
            vec!["inner"]
        );
    }

    #[test]
    fn escapes_outside_strings() {
        assert_eq!(targets(".a\\\"b { c: d }  @import \"foo\";"), vec!["foo"]);
    }

    #[test]
    fn interpolation_inside_strings() {
        assert_eq!(targets("$x: \"#{'\"'}\"; @import \"foo\";"), vec!["foo"]);
        assert_eq!(
            targets("$x: \"#{map-get((a: \"}\"), a)}\"; @use \"foo\";"),
            vec!["foo"]
        );
        // Interpolated URLs are plain CSS (@import) or invalid (@use).
        assert_eq!(
            targets("@import \"#{$theme}\"; @use \"#{$x}\"; @use \"ok\";"),
            vec!["ok"]
        );
    }

    #[test]
    fn css_escapes_in_urls_are_decoded() {
        assert_eq!(targets("@use \"\\66oo\";"), vec!["foo"]);
        assert_eq!(targets("@use \"\\66 oo\";"), vec!["foo"]);
        assert_eq!(targets("@use \"a\\\"b\";"), vec!["a\"b"]);
        assert_eq!(targets("@use \"a\\\nb\";"), vec!["ab"]);
        assert_eq!(targets("@use \"\\0\";"), vec!["\u{FFFD}"]);
    }

    #[test]
    fn lone_carriage_returns_are_line_breaks() {
        let directives = Parser::parse("/* a */\r/* b */\r@use \"x\";").unwrap();
        assert_eq!(directives[0].location(), &Location::new(3, 1));
        let directives = Parser::parse("/* a */\r\n@use \"x\";").unwrap();
        assert_eq!(directives[0].location(), &Location::new(2, 1));
    }

    #[test]
    fn columns_are_utf16_code_units() {
        let directives = Parser::parse("/* \u{1F389} */ @use \"x\";").unwrap();
        assert_eq!(directives[0].location().column, 10);
    }

    #[test]
    fn indented_syntax_comments_and_unquoted_imports() {
        let input = "/* header\n   continues @use \"no\"\n@use \"a\"\n// note\n  still comment @import \"no\"\n@import b, \"c\"\n@import d.css\n";
        assert_eq!(targets_with(input, Syntax::Sass), vec!["a", "b", "c"]);
        assert!(targets_with("@import \"a\" screen\n", Syntax::Sass).is_empty());
    }

    #[test]
    fn css_files_have_no_dependencies() {
        assert!(targets_with("@import \"b\";", Syntax::Css).is_empty());
        assert_eq!(Syntax::from_path(Path::new("a/b.CSS")), Syntax::Css);
        assert_eq!(Syntax::from_path(Path::new("a/b.sass")), Syntax::Sass);
        assert_eq!(Syntax::from_path(Path::new("a/b")), Syntax::Scss);
    }

    #[test]
    fn deep_nesting_is_iterative() {
        let depth = 1_000_000;
        let mut input = String::from("$x: \"");
        input.push_str(&"#{\"".repeat(depth));
        input.push_str("\n@use \"after\";");
        assert_eq!(targets(&input), vec!["after"]);

        let parens = format!(
            "@use \"a\" with ({}{});\n@use \"b\";",
            "(".repeat(depth),
            ")".repeat(depth)
        );
        assert_eq!(targets(&parens), vec!["a", "b"]);
    }
}
