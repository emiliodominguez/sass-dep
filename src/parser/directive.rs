//! Directive data structures.
//!
//! This module defines the data structures that represent parsed
//! SCSS dependency directives.

use serde::Serialize;

/// A parsed SCSS directive that creates a dependency.
#[derive(Debug, Clone, PartialEq)]
pub enum Directive {
    /// A `@use` directive.
    Use(UseDirective),
    /// A `@forward` directive.
    Forward(ForwardDirective),
    /// A `@import` directive (legacy).
    Import(ImportDirective),
}

impl Directive {
    /// Returns the path(s) referenced by this directive.
    pub fn paths(&self) -> Vec<&str> {
        match self {
            Directive::Use(d) => vec![&d.path],
            Directive::Forward(d) => vec![&d.path],
            Directive::Import(d) => d.paths.iter().map(|s| s.as_str()).collect(),
        }
    }

    /// Returns the location of this directive in the source file.
    pub fn location(&self) -> &Location {
        match self {
            Directive::Use(d) => &d.location,
            Directive::Forward(d) => &d.location,
            Directive::Import(d) => &d.location,
        }
    }
}

/// A parsed `@use` directive.
///
/// The `@use` rule loads mixins, functions, and variables from other
/// Sass stylesheets, and combines CSS from multiple stylesheets together.
///
/// # Examples
///
/// ```scss
/// @use "variables";           // Default namespace: variables
/// @use "variables" as vars;   // Custom namespace: vars
/// @use "variables" as *;      // No namespace (global)
/// @use "variables" with ($x: 1);  // Configured
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct UseDirective {
    /// The path to the imported module.
    pub path: String,
    /// The namespace for accessing module members.
    pub namespace: Option<Namespace>,
    /// Whether the module is configured with `with (...)`.
    pub configured: bool,
    /// Source location of this directive.
    pub location: Location,
}

/// Namespace specification for a `@use` directive.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Namespace {
    /// A named namespace (`@use "x" as name`).
    Named(String),
    /// Global namespace (`@use "x" as *`).
    Star,
    /// Default namespace derived from filename.
    Default,
}

impl Namespace {
    /// Returns the namespace as a string, or None for Star/Default.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Namespace::Named(s) => Some(s),
            Namespace::Star => Some("*"),
            Namespace::Default => None,
        }
    }
}

/// A parsed `@forward` directive.
///
/// The `@forward` rule loads a Sass stylesheet and makes its mixins,
/// functions, and variables available when your stylesheet is loaded
/// with the `@use` rule.
///
/// # Examples
///
/// ```scss
/// @forward "functions";
/// @forward "functions" as fn-*;
/// @forward "functions" hide internal-fn;
/// @forward "functions" show public-fn, $public-var;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ForwardDirective {
    /// The path to the forwarded module.
    pub path: String,
    /// Optional prefix for forwarded members.
    pub prefix: Option<String>,
    /// Visibility rules for forwarded members.
    pub visibility: Visibility,
    /// Source location of this directive.
    pub location: Location,
}

/// Visibility specification for a `@forward` directive.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// All members are forwarded.
    All,
    /// Only specified members are forwarded.
    Show(Vec<String>),
    /// All members except specified are forwarded.
    Hide(Vec<String>),
}

/// A parsed `@import` directive (legacy).
///
/// The `@import` rule is a legacy feature that loads styles from other
/// stylesheets. It has been deprecated in favor of `@use` and `@forward`.
///
/// # Examples
///
/// ```scss
/// @import "legacy";
/// @import "file1", "file2", "file3";
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDirective {
    /// The paths to import.
    pub paths: Vec<String>,
    /// Source location of this directive.
    pub location: Location,
}

/// Source location of a directive.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Location {
    /// Line number (1-indexed).
    pub line: usize,
    /// Column number (1-indexed).
    pub column: usize,
}

impl Location {
    /// Creates a new location.
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directive_paths_use() {
        let directive = Directive::Use(UseDirective {
            path: "variables".to_string(),
            namespace: None,
            configured: false,
            location: Location::default(),
        });
        assert_eq!(directive.paths(), vec!["variables"]);
    }

    #[test]
    fn directive_paths_import() {
        let directive = Directive::Import(ImportDirective {
            paths: vec!["a".to_string(), "b".to_string()],
            location: Location::default(),
        });
        assert_eq!(directive.paths(), vec!["a", "b"]);
    }

    #[test]
    fn namespace_as_str() {
        assert_eq!(Namespace::Named("foo".to_string()).as_str(), Some("foo"));
        assert_eq!(Namespace::Star.as_str(), Some("*"));
        assert_eq!(Namespace::Default.as_str(), None);
    }
}
