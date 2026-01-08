//! SCSS lexer and parser implementation using nom.
//!
//! This module implements a minimal parser that extracts only dependency
//! directives (`@use`, `@forward`, `@import`) from SCSS source code.

use nom::{
    branch::alt,
    bytes::complete::{tag_no_case, take_until, take_while, take_while1},
    character::complete::{char, multispace0, multispace1},
    combinator::{map, opt, peek, recognize, value},
    multi::separated_list1,
    sequence::{delimited, pair, tuple},
    IResult,
};

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
    /// A vector of parsed directives, or an error if parsing fails.
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
        let mut directives = Vec::new();
        let mut remaining = input;
        let mut current_line = 1;
        let mut line_start = 0;

        while !remaining.is_empty() {
            // Skip whitespace and track position
            let (new_remaining, skipped) = skip_to_at_or_end(remaining);

            // Update line tracking
            for (i, c) in skipped.char_indices() {
                if c == '\n' {
                    current_line += 1;
                    line_start = input.len() - remaining.len() + i + 1;
                }
            }

            remaining = new_remaining;

            if remaining.is_empty() {
                break;
            }

            // Check for @ symbol
            if !remaining.starts_with('@') {
                // Skip one character and continue
                let mut chars = remaining.chars();
                if let Some(c) = chars.next() {
                    if c == '\n' {
                        current_line += 1;
                        line_start = input.len() - remaining.len() + 1;
                    }
                    remaining = chars.as_str();
                }
                continue;
            }

            // Calculate column
            let current_pos = input.len() - remaining.len();
            let column = current_pos - line_start + 1;
            let location = Location::new(current_line, column);

            // Try to parse a directive
            if let Ok((new_remaining, directive)) = parse_directive(remaining, &location) {
                directives.push(directive);
                remaining = new_remaining;
            } else {
                // Not a directive we care about, skip the @ and continue
                remaining = &remaining[1..];
            }
        }

        Ok(directives)
    }

    /// Parses a single file and returns its directives.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SCSS file
    ///
    /// # Returns
    ///
    /// A vector of parsed directives, or an error.
    pub fn parse_file(path: &std::path::Path) -> Result<Vec<Directive>, ParseError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }
}

/// Skips characters until an @ symbol or end of input.
fn skip_to_at_or_end(input: &str) -> (&str, &str) {
    let mut in_string = false;
    let mut string_char = '"';
    let mut in_single_comment = false;
    let mut in_multi_comment = false;
    let mut prev_char = '\0';
    let mut end_pos = 0;

    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Handle string literals
        if !in_single_comment && !in_multi_comment {
            if !in_string && (c == '"' || c == '\'') {
                in_string = true;
                string_char = c;
            } else if in_string && c == string_char && prev_char != '\\' {
                in_string = false;
            }
        }

        // Handle comments
        if !in_string && !in_single_comment && !in_multi_comment && c == '/' && i + 1 < chars.len() {
            if chars[i + 1] == '/' {
                in_single_comment = true;
                i += 2;
                continue;
            } else if chars[i + 1] == '*' {
                in_multi_comment = true;
                i += 2;
                continue;
            }
        }

        // End single-line comment on newline
        if in_single_comment && c == '\n' {
            in_single_comment = false;
        }

        // End multi-line comment
        if in_multi_comment && c == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
            in_multi_comment = false;
            i += 2;
            continue;
        }

        // Check for @ outside strings and comments
        if c == '@' && !in_string && !in_single_comment && !in_multi_comment {
            let skipped = &input[..end_pos];
            let remaining = &input[end_pos..];
            return (remaining, skipped);
        }

        prev_char = c;
        end_pos += c.len_utf8();
        i += 1;
    }

    ("", input)
}

/// Parses a directive starting with @.
fn parse_directive<'a>(input: &'a str, location: &Location) -> IResult<&'a str, Directive> {
    alt((
        map(|i| parse_use_directive(i, location), Directive::Use),
        map(|i| parse_forward_directive(i, location), Directive::Forward),
        map(|i| parse_import_directive(i, location), Directive::Import),
    ))(input)
}

/// Parses a @use directive.
fn parse_use_directive<'a>(input: &'a str, location: &Location) -> IResult<&'a str, UseDirective> {
    let (input, _) = tag_no_case("@use")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, path) = parse_string(input)?;
    let (input, _) = multispace0(input)?;

    // Parse optional "as" clause
    let (input, namespace) = opt(parse_as_clause)(input)?;
    let (input, _) = multispace0(input)?;

    // Parse optional "with" clause
    let (input, configured) = map(opt(parse_with_clause), |w| w.is_some())(input)?;
    let (input, _) = multispace0(input)?;

    // Consume semicolon
    let (input, _) = opt(char(';'))(input)?;

    Ok((
        input,
        UseDirective {
            path,
            namespace,
            configured,
            location: location.clone(),
        },
    ))
}

/// Parses the "as" clause in @use.
fn parse_as_clause(input: &str) -> IResult<&str, Namespace> {
    let (input, _) = tag_no_case("as")(input)?;
    let (input, _) = multispace1(input)?;

    alt((
        value(Namespace::Star, char('*')),
        map(parse_identifier, |s| Namespace::Named(s.to_string())),
    ))(input)
}

/// Parses the "with" clause in @use.
fn parse_with_clause(input: &str) -> IResult<&str, ()> {
    let (input, _) = tag_no_case("with")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = delimited(char('('), take_until(")"), char(')'))(input)?;
    Ok((input, ()))
}

/// Parses a @forward directive.
fn parse_forward_directive<'a>(
    input: &'a str,
    location: &Location,
) -> IResult<&'a str, ForwardDirective> {
    let (input, _) = tag_no_case("@forward")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, path) = parse_string(input)?;
    let (input, _) = multispace0(input)?;

    // Parse optional "as prefix-*" clause
    let (input, prefix) = opt(parse_forward_as_clause)(input)?;
    let (input, _) = multispace0(input)?;

    // Parse optional "show" or "hide" clause
    let (input, visibility) = parse_visibility_clause(input)?;
    let (input, _) = multispace0(input)?;

    // Consume semicolon
    let (input, _) = opt(char(';'))(input)?;

    Ok((
        input,
        ForwardDirective {
            path,
            prefix,
            visibility,
            location: location.clone(),
        },
    ))
}

/// Parses the "as prefix-*" clause in @forward.
fn parse_forward_as_clause(input: &str) -> IResult<&str, String> {
    let (input, _) = tag_no_case("as")(input)?;
    let (input, _) = multispace1(input)?;

    // Parse identifier followed by -* pattern
    // We need to be careful: the prefix can contain hyphens, but ends with -*
    // So we look for anything up to and including "-*"
    let (input, prefix_with_star) = take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '*')(input)?;

    // Validate it ends with -* and extract the prefix
    if prefix_with_star.ends_with("-*") {
        let prefix = prefix_with_star.trim_end_matches('*');
        Ok((input, prefix.to_string()))
    } else {
        // Not a valid prefix pattern
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    }
}

/// Parses the visibility clause (show/hide) in @forward.
fn parse_visibility_clause(input: &str) -> IResult<&str, Visibility> {
    alt((
        map(parse_show_clause, Visibility::Show),
        map(parse_hide_clause, Visibility::Hide),
        value(Visibility::All, peek(alt((char(';'), char('\n'))))),
        value(Visibility::All, multispace0),
    ))(input)
}

/// Parses a "show" clause.
fn parse_show_clause(input: &str) -> IResult<&str, Vec<String>> {
    let (input, _) = tag_no_case("show")(input)?;
    let (input, _) = multispace1(input)?;
    parse_member_list(input)
}

/// Parses a "hide" clause.
fn parse_hide_clause(input: &str) -> IResult<&str, Vec<String>> {
    let (input, _) = tag_no_case("hide")(input)?;
    let (input, _) = multispace1(input)?;
    parse_member_list(input)
}

/// Parses a comma-separated list of members.
fn parse_member_list(input: &str) -> IResult<&str, Vec<String>> {
    separated_list1(
        tuple((multispace0, char(','), multispace0)),
        map(parse_member, |s| s.to_string()),
    )(input)
}

/// Parses a member name (variable, mixin, or function).
fn parse_member(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        opt(char('$')),
        take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_'),
    ))(input)
}

/// Parses a @import directive.
fn parse_import_directive<'a>(
    input: &'a str,
    location: &Location,
) -> IResult<&'a str, ImportDirective> {
    let (input, _) = tag_no_case("@import")(input)?;
    let (input, _) = multispace1(input)?;

    // Parse comma-separated list of paths
    let (input, paths) = separated_list1(
        tuple((multispace0, char(','), multispace0)),
        parse_string,
    )(input)?;

    let (input, _) = multispace0(input)?;
    let (input, _) = opt(char(';'))(input)?;

    Ok((
        input,
        ImportDirective {
            paths,
            location: location.clone(),
        },
    ))
}

/// Parses a quoted string.
fn parse_string(input: &str) -> IResult<&str, String> {
    alt((
        map(
            delimited(char('"'), take_while(|c| c != '"'), char('"')),
            |s: &str| s.to_string(),
        ),
        map(
            delimited(char('\''), take_while(|c| c != '\''), char('\'')),
            |s: &str| s.to_string(),
        ),
    ))(input)
}

/// Parses an identifier.
fn parse_identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_')(input)
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
            assert_eq!(use_dir.namespace, Some(Namespace::Named("vars".to_string())));
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
    fn parse_with_comments() {
        let input = r#"
// This is a comment
@use "variables";
/* Multi-line
   comment */
@forward "mixins";
"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 2);
    }

    #[test]
    fn parse_ignores_other_at_rules() {
        let input = r#"
@use "variables";
@mixin foo { }
@media screen { }
@keyframes slide { }
@forward "mixins";
"#;
        let directives = Parser::parse(input).unwrap();
        assert_eq!(directives.len(), 2);
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
    color: red;
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
}
