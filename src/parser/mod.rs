//! Parser module for SCSS dependency directives.
//!
//! This module provides functionality to parse SCSS files and extract
//! `@use`, `@forward`, and `@import` directives. It uses a hand-written,
//! single-pass byte scanner (see ADR 001) that skips strings, comments and
//! `url(...)` tokens and tracks exact source locations.
//!
//! # Supported Directives
//!
//! ## @use
//! ```scss
//! @use "path";
//! @use "path" as namespace;
//! @use "path" as *;
//! @use "path" with ($var: value);
//! ```
//!
//! ## @forward
//! ```scss
//! @forward "path";
//! @forward "path" as prefix-*;
//! @forward "path" hide $var, mixin-name;
//! @forward "path" show $var, mixin-name;
//! ```
//!
//! ## @import (legacy)
//! ```scss
//! @import "path";
//! @import "path1", "path2", "path3";
//! ```
//!
//! Plain CSS imports (`url(...)`, `*.css`, remote URLs, imports with media
//! queries) are ignored because Sass never loads them.
//!
//! # Example
//!
//! ```
//! use sass_dep::parser::Parser;
//!
//! let scss = r#"
//! @use "variables" as vars;
//! @use "mixins";
//! "#;
//!
//! let directives = Parser::parse(scss).unwrap();
//! assert_eq!(directives.len(), 2);
//! ```

mod directive;
mod error;
mod lexer;

pub use directive::{
    Directive, ForwardDirective, ImportDirective, Location, Namespace, UseDirective, Visibility,
};
pub use error::ParseError;
pub use lexer::{Parser, Syntax};
