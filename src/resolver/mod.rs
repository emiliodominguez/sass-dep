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
//! 1. `/project/src/foo.scss`
//! 2. `/project/src/foo.sass`
//! 3. `/project/src/_foo.scss`
//! 4. `/project/src/_foo.sass`
//! 5. `/project/src/foo/index.scss`
//! 6. `/project/src/foo/_index.scss`
//! 7. `/project/src/foo/index.sass`
//! 8. `/project/src/foo/_index.sass`
//! 9. Repeat for each load path
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

pub use path::{ResolveError, Resolver, ResolverConfig};
