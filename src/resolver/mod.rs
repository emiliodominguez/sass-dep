//! Path resolver module for Sass imports.
//!
//! This module implements Sass-compliant path resolution for `@use`,
//! `@forward`, and `@import` directives.
//!
//! # Resolution Algorithm
//!
//! For a path like `@use "foo"` from `/project/src/main.scss`, the resolver
//! searches in the following order:
//!
//! 1. `/project/src/foo.scss`, then `/project/src/_foo.scss`
//! 2. `/project/src/foo.sass`, then `/project/src/_foo.sass`
//! 3. `/project/src/foo.css`, then `/project/src/_foo.css` (`@use`/`@forward` only)
//! 4. `/project/src/foo/index.*`, then `/project/src/foo/_index.*` (same extension order)
//! 5. Repeat for each load path
//!
//! `@import` additionally prefers import-only files (`foo.import.scss`).
//! Targets with an explicit extension (`"foo.scss"`) only match that extension.
//! Webpack-style `~package/path` targets are looked up in the load paths and
//! in `node_modules` directories walking up from the importing file.
//!
//! # Example
//!
//! ```
//! use sass_dep::resolver::{Resolver, ResolverConfig};
//! use std::path::PathBuf;
//!
//! let config = ResolverConfig {
//!     load_paths: vec![PathBuf::from("node_modules")],
//!     extensions: vec!["scss".to_string(), "sass".to_string()],
//! };
//!
//! let resolver = Resolver::new(config);
//! // resolver.resolve(&PathBuf::from("src"), "variables")
//! ```

mod path;

pub(crate) use path::normalize_lexically;
pub use path::{ResolveError, ResolveKind, Resolver, ResolverConfig};
