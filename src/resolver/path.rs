//! Path resolution implementation.
//!
//! This module mirrors Dart Sass's filesystem importer
//! (`resolveImportPath` in `lib/src/importer/utils.dart`):
//!
//! - The target is a URL: `?query` and `#fragment` are dropped and the path
//!   is percent-decoded.
//! - `.sass` and `.scss` candidates (plain and `_partial`) are collected
//!   together and must match **exactly one** file; several matches are an
//!   ambiguity error, exactly like Dart. `.css` is only tried when neither
//!   matches, for `@use`, `@forward` and `@import` alike.
//! - `@import` first tries import-only files (`foo.import.scss`).
//! - Directories resolve to `index` files; targets that name a directory
//!   explicitly (`"."`, `".."`, `"foo/"`) only resolve as directories.
//! - Paths are kept **lexical** (symlinks are not resolved), so relative
//!   imports inside a symlinked file resolve from the link's location.
//!
//! Lookups are backed by a thread-safe **directory listing cache**: each
//! directory is read once, after which every candidate check is a hash
//! lookup, so the resolver can be shared by the parallel graph builder.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};

use thiserror::Error;

/// Configuration for the path resolver.
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    /// Additional directories to search for imports.
    ///
    /// These are searched after the relative path from the importing file.
    /// Relative load paths are resolved against the importing file's
    /// directory; the CLI always passes absolute paths (resolved against
    /// `--root`).
    pub load_paths: Vec<PathBuf>,

    /// Sass file extensions to try (`.css` is always the final fallback).
    ///
    /// Defaults to `["scss", "sass"]`. All of them are tried together, so a
    /// target matching files with two different extensions is ambiguous.
    pub extensions: Vec<String>,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            load_paths: Vec::new(),
            extensions: vec!["scss".to_string(), "sass".to_string()],
        }
    }
}

/// The kind of rule a path is being resolved for.
///
/// `@import` additionally considers import-only files (`foo.import.scss`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolveKind {
    /// `@use` or `@forward`.
    Module,
    /// Legacy `@import`.
    Import,
}

/// Errors that can occur during path resolution.
#[derive(Debug, Error)]
pub enum ResolveError {
    /// The target file could not be found.
    #[error("Could not resolve '{target}' from '{base}'")]
    NotFound {
        /// The base directory from which resolution was attempted.
        base: PathBuf,
        /// The target path that could not be resolved.
        target: String,
    },

    /// Several files match the target (Dart Sass refuses to pick one).
    #[error("It's not clear which file to import for '{target}'; found: {}", .candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))]
    Ambiguous {
        /// The target path.
        target: String,
        /// Every matching file.
        candidates: Vec<PathBuf>,
    },

    /// The base path is invalid (not a file or directory).
    #[error("Invalid base path: {0}")]
    InvalidBasePath(PathBuf),

    /// IO error during resolution.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of a lookup step: `Ok(None)` means "not here, keep looking";
/// `Err` carries the candidates of an ambiguous match.
type Lookup = Result<Option<PathBuf>, Vec<PathBuf>>;

/// Cached listing of a directory.
#[derive(Debug, Default)]
struct DirListing {
    /// Regular files (symlinks are followed).
    files: HashSet<OsString>,
    /// Sub-directories (symlinks are followed).
    dirs: HashSet<OsString>,
}

/// Shared caches behind a [`Resolver`].
#[derive(Debug, Default)]
struct Caches {
    listings: RwLock<HashMap<PathBuf, Arc<DirListing>>>,
}

/// Sass-compliant path resolver.
///
/// Resolves `@use`, `@forward`, and `@import` paths according to
/// Sass conventions. Cloning a resolver shares its caches.
#[derive(Debug, Clone)]
pub struct Resolver {
    config: ResolverConfig,
    caches: Arc<Caches>,
}

impl Resolver {
    /// Creates a new resolver with the given configuration.
    pub fn new(config: ResolverConfig) -> Self {
        Self {
            config,
            caches: Arc::default(),
        }
    }

    /// Returns `true` for URLs that never map to a file on disk.
    ///
    /// This covers Sass built-in modules (`sass:math`), the Node.js package
    /// importer (`pkg:`) and any other URL scheme (`https:`, `data:`, ...).
    /// Single-letter schemes are treated as Windows drive letters.
    pub fn is_external(target: &str) -> bool {
        match target.split_once(':') {
            Some((scheme, _)) => {
                scheme.len() > 1
                    && scheme
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
            }
            None => false,
        }
    }

    /// Resolves a `@use`/`@forward`/`@import` path to an absolute file path.
    ///
    /// # Arguments
    ///
    /// * `base` - The path of the file containing the directive (or its directory)
    /// * `target` - The URL from the directive (e.g., `"variables"`)
    ///
    /// # Returns
    ///
    /// The absolute, lexically normalized path of the resolved file.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use sass_dep::resolver::{Resolver, ResolverConfig};
    /// use std::path::PathBuf;
    ///
    /// let resolver = Resolver::new(ResolverConfig::default());
    /// let result = resolver.resolve(
    ///     &PathBuf::from("/project/src/main.scss"),
    ///     "variables"
    /// );
    /// ```
    pub fn resolve(&self, base: &Path, target: &str) -> Result<PathBuf, ResolveError> {
        let base_dir = if base.is_file() {
            base.parent()
                .ok_or_else(|| ResolveError::InvalidBasePath(base.to_path_buf()))?
        } else if base.is_dir() {
            base
        } else {
            return Err(ResolveError::InvalidBasePath(base.to_path_buf()));
        };

        self.resolve_from_dir(base_dir, target, ResolveKind::Module)
    }

    /// Resolves `target` relative to `base_dir` for the given rule kind.
    ///
    /// Search order: relative to `base_dir`, then each load path. Webpack
    /// style `~pkg/path` targets are looked up in the load paths and then in
    /// `node_modules` directories walking up from `base_dir`.
    ///
    /// # Errors
    ///
    /// [`ResolveError::NotFound`] if nothing matches, or
    /// [`ResolveError::Ambiguous`] if several files match in the first
    /// location that has any match.
    pub fn resolve_from_dir(
        &self,
        base_dir: &Path,
        target: &str,
        kind: ResolveKind,
    ) -> Result<PathBuf, ResolveError> {
        let not_found = || ResolveError::NotFound {
            base: base_dir.to_path_buf(),
            target: target.to_string(),
        };

        let path = url_to_path(target).ok_or_else(not_found)?;
        if path.is_empty() {
            return Err(not_found());
        }

        let lookup = match path.strip_prefix('~') {
            Some(package) => match self.in_load_paths(base_dir, package, kind) {
                Ok(None) => self.in_node_modules(base_dir, package, kind),
                other => other,
            },
            None => match self.in_dir(base_dir, &path, kind) {
                Ok(None) => self.in_load_paths(base_dir, &path, kind),
                other => other,
            },
        };

        match lookup {
            Ok(Some(found)) => Ok(found),
            Ok(None) => Err(not_found()),
            Err(candidates) => Err(ResolveError::Ambiguous {
                target: target.to_string(),
                candidates,
            }),
        }
    }

    /// Tries every configured load path in order.
    fn in_load_paths(&self, base_dir: &Path, path: &str, kind: ResolveKind) -> Lookup {
        for load_path in &self.config.load_paths {
            let load_dir = if load_path.is_absolute() {
                load_path.clone()
            } else {
                base_dir.join(load_path)
            };
            if let Some(found) = self.in_dir(&load_dir, path, kind)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Tries `node_modules/<path>` in `base_dir` and each of its ancestors.
    fn in_node_modules(&self, base_dir: &Path, path: &str, kind: ResolveKind) -> Lookup {
        for dir in base_dir.ancestors() {
            if let Some(found) = self.in_dir(&dir.join("node_modules"), path, kind)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Resolves a decoded URL path against one directory.
    fn in_dir(&self, dir: &Path, path: &str, kind: ResolveKind) -> Lookup {
        // URLs always use `/`; push segments one by one so this also works
        // under Windows verbatim (`\\?\`) prefixes.
        let mut joined = if path.starts_with('/') {
            PathBuf::from("/")
        } else {
            dir.to_path_buf()
        };
        joined.extend(path.split('/').filter(|segment| !segment.is_empty()));
        let joined = normalize_lexically(&joined);

        let names_directory =
            path.ends_with('/') || matches!(path.rsplit('/').next(), Some("." | ".."));
        if names_directory {
            return self.as_directory(&joined, kind);
        }

        self.import_path(&joined, kind)
    }

    /// Dart's `resolveImportPath` for a path that may name a file or directory.
    fn import_path(&self, path: &Path, kind: ResolveKind) -> Lookup {
        let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(OsStr::to_str))
        else {
            return Ok(None);
        };

        if let Some((stem, ext)) = name.rsplit_once('.') {
            if self.is_known_extension(ext) {
                if kind == ResolveKind::Import {
                    let import_only = format!("{stem}.import.{ext}");
                    if let Some(found) = exactly_one(self.try_path(dir, &import_only))? {
                        return Ok(Some(found));
                    }
                }
                return exactly_one(self.try_path(dir, name));
            }
        }

        if kind == ResolveKind::Import {
            let import_only = format!("{name}.import");
            if let Some(found) = exactly_one(self.with_extensions(dir, &import_only))? {
                return Ok(Some(found));
            }
        }
        if let Some(found) = exactly_one(self.with_extensions(dir, name))? {
            return Ok(Some(found));
        }

        self.as_directory(path, kind)
    }

    /// Resolves `path` as a directory containing an `index` file.
    fn as_directory(&self, path: &Path, kind: ResolveKind) -> Lookup {
        let is_dir = match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) => self.listing(parent).dirs.contains(name),
            _ => path.is_dir(),
        };
        if !is_dir {
            return Ok(None);
        }

        if kind == ResolveKind::Import {
            if let Some(found) = exactly_one(self.with_extensions(path, "index.import"))? {
                return Ok(Some(found));
            }
        }
        exactly_one(self.with_extensions(path, "index"))
    }

    /// Candidates for `base` with every Sass extension, else with `.css`.
    fn with_extensions(&self, dir: &Path, base: &str) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = self
            .config
            .extensions
            .iter()
            .filter(|ext| ext.as_str() != "css")
            .flat_map(|ext| self.try_path(dir, &format!("{base}.{ext}")))
            .collect();
        if found.is_empty() {
            found = self.try_path(dir, &format!("{base}.css"));
        }
        found
    }

    /// Existing files among `_name` and `name` in `dir`.
    fn try_path(&self, dir: &Path, name: &str) -> Vec<PathBuf> {
        let listing = self.listing(dir);
        [format!("_{name}"), name.to_string()]
            .into_iter()
            .filter(|candidate| listing.files.contains(OsStr::new(candidate)))
            .map(|candidate| dir.join(candidate))
            .collect()
    }

    /// Returns `true` for extensions that are loaded as-is when written explicitly.
    fn is_known_extension(&self, ext: &str) -> bool {
        ext == "css" || self.config.extensions.iter().any(|e| e == ext)
    }

    /// Returns the cached listing of `dir`, reading it on first access.
    ///
    /// Unreadable or missing directories yield an empty listing.
    fn listing(&self, dir: &Path) -> Arc<DirListing> {
        if let Some(listing) = self
            .caches
            .listings
            .read()
            .expect("listing cache poisoned")
            .get(dir)
        {
            return Arc::clone(listing);
        }

        let listing = Arc::new(read_listing(dir));
        self.caches
            .listings
            .write()
            .expect("listing cache poisoned")
            .entry(dir.to_path_buf())
            .or_insert(listing)
            .clone()
    }

    /// Returns the configured load paths.
    pub fn load_paths(&self) -> &[PathBuf] {
        &self.config.load_paths
    }

    /// Returns the configured extensions.
    pub fn extensions(&self) -> &[String] {
        &self.config.extensions
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new(ResolverConfig::default())
    }
}

/// Dart's `_exactlyOne`: no match, one match, or an ambiguity.
fn exactly_one(mut candidates: Vec<PathBuf>) -> Lookup {
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.pop()),
        _ => Err(candidates),
    }
}

/// Converts a Sass URL into a decoded path.
///
/// Drops `?query` and `#fragment` and percent-decodes the rest. Returns
/// `None` for invalid percent-encoding or non-UTF-8 results.
fn url_to_path(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    if !path.contains('%') {
        return Some(path.to_string());
    }

    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// Reads a directory into a [`DirListing`], following symlinks.
fn read_listing(dir: &Path) -> DirListing {
    let mut listing = DirListing::default();

    let Ok(entries) = fs::read_dir(dir) else {
        return listing;
    };

    for entry in entries.flatten() {
        let Ok(mut file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            match fs::metadata(entry.path()) {
                Ok(meta) => file_type = meta.file_type(),
                Err(_) => continue,
            }
        }
        if file_type.is_dir() {
            listing.dirs.insert(entry.file_name());
        } else if file_type.is_file() {
            listing.files.insert(entry.file_name());
        }
    }

    listing
}

/// Lexically resolves `.` and `..` components without touching the filesystem.
pub(crate) fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_files(dir: &Path) {
        fs::write(dir.join("main.scss"), "").unwrap();
        fs::write(dir.join("_variables.scss"), "").unwrap();
        fs::write(dir.join("mixins.scss"), "").unwrap();

        fs::create_dir_all(dir.join("components")).unwrap();
        fs::write(dir.join("components/_index.scss"), "").unwrap();
        fs::write(dir.join("components/_button.scss"), "").unwrap();

        fs::create_dir_all(dir.join("utils")).unwrap();
        fs::write(dir.join("utils/index.scss"), "").unwrap();
    }

    #[test]
    fn resolve_simple_file() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "mixins");

        assert!(result.unwrap().ends_with("mixins.scss"));
    }

    #[test]
    fn resolve_partial_file() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "variables");

        assert!(result.unwrap().ends_with("_variables.scss"));
    }

    #[test]
    fn resolve_directory_with_underscore_index() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let resolved = resolver
            .resolve(&temp.path().join("main.scss"), "components")
            .unwrap();

        assert!(resolved.ends_with("components/_index.scss"));
    }

    #[test]
    fn resolve_directory_with_index() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let resolved = resolver
            .resolve(&temp.path().join("main.scss"), "utils")
            .unwrap();

        assert!(resolved.ends_with("utils/index.scss"));
    }

    #[test]
    fn resolve_nested_path() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "components/button");

        assert!(result.unwrap().ends_with("_button.scss"));
    }

    #[test]
    fn resolve_parent_relative_path() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve_from_dir(
            &temp.path().join("components"),
            "../utils/../variables",
            ResolveKind::Module,
        );

        assert!(result.unwrap().ends_with("_variables.scss"));
    }

    #[test]
    fn resolve_not_found() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "nonexistent");

        assert!(matches!(result.unwrap_err(), ResolveError::NotFound { .. }));
    }

    #[test]
    fn resolve_explicit_extension() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "variables.scss");

        assert!(result.unwrap().ends_with("_variables.scss"));
    }

    #[test]
    fn resolve_css_fallback_for_every_rule() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("normalize.css"), "").unwrap();

        let resolver = Resolver::default();
        for kind in [ResolveKind::Module, ResolveKind::Import] {
            assert!(resolver
                .resolve_from_dir(temp.path(), "normalize", kind)
                .unwrap()
                .ends_with("normalize.css"));
        }
    }

    #[test]
    fn css_file_wins_over_directory_index() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("foo.css"), "").unwrap();
        fs::create_dir_all(temp.path().join("foo")).unwrap();
        fs::write(temp.path().join("foo/_index.scss"), "").unwrap();

        let resolver = Resolver::default();
        assert!(resolver
            .resolve_from_dir(temp.path(), "foo", ResolveKind::Import)
            .unwrap()
            .ends_with("foo.css"));
    }

    #[test]
    fn explicit_extension_prefers_import_only_file_for_imports() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("_a.scss"), "").unwrap();
        fs::write(temp.path().join("_a.import.scss"), "").unwrap();
        fs::write(temp.path().join("a.import.css"), "").unwrap();

        let resolver = Resolver::default();
        assert!(resolver
            .resolve_from_dir(temp.path(), "a.scss", ResolveKind::Import)
            .unwrap()
            .ends_with("_a.import.scss"));
        assert!(resolver
            .resolve_from_dir(temp.path(), "a.scss", ResolveKind::Module)
            .unwrap()
            .ends_with("_a.scss"));
    }

    #[test]
    fn ambiguous_candidates_are_an_error() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("a.scss"), "").unwrap();
        fs::write(temp.path().join("_a.scss"), "").unwrap();
        fs::write(temp.path().join("_b.scss"), "").unwrap();
        fs::write(temp.path().join("b.sass"), "").unwrap();

        let resolver = Resolver::default();
        for target in ["a", "b", "a.scss"] {
            let error = resolver
                .resolve_from_dir(temp.path(), target, ResolveKind::Module)
                .unwrap_err();
            assert!(
                matches!(error, ResolveError::Ambiguous { ref candidates, .. } if candidates.len() == 2),
                "{target}"
            );
        }
    }

    #[test]
    fn directory_targets_only_resolve_index_files() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("src/foo")).unwrap();
        fs::write(temp.path().join("_src.scss"), "").unwrap();
        fs::write(temp.path().join("src/_index.scss"), "").unwrap();
        fs::write(temp.path().join("src/_foo.scss"), "").unwrap();
        fs::write(temp.path().join("src/foo/_index.scss"), "").unwrap();
        fs::write(temp.path().join("src/_bar.scss"), "").unwrap();

        let resolver = Resolver::default();
        let src = temp.path().join("src");
        let resolve = |target: &str| resolver.resolve_from_dir(&src, target, ResolveKind::Module);

        assert!(resolve(".").unwrap().ends_with("src/_index.scss"));
        assert!(resolve("foo/").unwrap().ends_with("foo/_index.scss"));
        assert!(resolve("foo").unwrap().ends_with("src/_foo.scss"));
        assert!(resolve("bar/").is_err());
        assert!(resolve("").is_err());
        assert!(resolver
            .resolve_from_dir(&src.join("foo"), "..", ResolveKind::Module)
            .unwrap()
            .ends_with("src/_index.scss"));
    }

    #[test]
    fn targets_are_urls() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("_a b.scss"), "").unwrap();
        fs::write(temp.path().join("_ñ.scss"), "").unwrap();
        fs::write(temp.path().join("_c.scss"), "").unwrap();

        let resolver = Resolver::default();
        let resolve =
            |target: &str| resolver.resolve_from_dir(temp.path(), target, ResolveKind::Module);

        assert!(resolve("a%20b").unwrap().ends_with("_a b.scss"));
        assert!(resolve("%C3%B1").unwrap().ends_with("_ñ.scss"));
        assert!(resolve("c?query").unwrap().ends_with("_c.scss"));
        assert!(resolve("c#fragment").unwrap().ends_with("_c.scss"));
        assert!(resolve("c%zz").is_err());
        assert!(resolve("%FF").is_err());
    }

    #[test]
    fn resolve_import_only_files_first() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("_theme.scss"), "").unwrap();
        fs::write(temp.path().join("_theme.import.scss"), "").unwrap();

        let resolver = Resolver::default();
        assert!(resolver
            .resolve_from_dir(temp.path(), "theme", ResolveKind::Import)
            .unwrap()
            .ends_with("_theme.import.scss"));
        assert!(resolver
            .resolve_from_dir(temp.path(), "theme", ResolveKind::Module)
            .unwrap()
            .ends_with("_theme.scss"));
    }

    #[test]
    fn resolve_tilde_from_node_modules() {
        let temp = TempDir::new().unwrap();
        let pkg = temp.path().join("node_modules/lib/scss");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("_core.scss"), "").unwrap();
        fs::create_dir_all(temp.path().join("src/deep")).unwrap();

        let resolver = Resolver::default();
        let resolved = resolver
            .resolve_from_dir(
                &temp.path().join("src/deep"),
                "~lib/scss/core",
                ResolveKind::Import,
            )
            .unwrap();

        assert!(resolved.ends_with("node_modules/lib/scss/_core.scss"));
    }

    #[test]
    fn resolve_with_load_path() {
        let temp = TempDir::new().unwrap();

        let vendor_dir = temp.path().join("vendor");
        fs::create_dir_all(&vendor_dir).unwrap();
        fs::write(vendor_dir.join("_library.scss"), "").unwrap();
        fs::write(temp.path().join("main.scss"), "").unwrap();

        let config = ResolverConfig {
            load_paths: vec![PathBuf::from("vendor")],
            extensions: vec!["scss".to_string()],
        };
        let resolver = Resolver::new(config);

        let result = resolver.resolve(&temp.path().join("main.scss"), "library");

        assert!(result.unwrap().ends_with("_library.scss"));
    }

    #[test]
    fn resolve_prefers_relative_over_load_path() {
        let temp = TempDir::new().unwrap();

        fs::write(temp.path().join("_library.scss"), "relative").unwrap();

        let vendor_dir = temp.path().join("vendor");
        fs::create_dir_all(&vendor_dir).unwrap();
        fs::write(vendor_dir.join("_library.scss"), "vendor").unwrap();

        fs::write(temp.path().join("main.scss"), "").unwrap();

        let config = ResolverConfig {
            load_paths: vec![PathBuf::from("vendor")],
            extensions: vec!["scss".to_string()],
        };
        let resolver = Resolver::new(config);

        let resolved = resolver
            .resolve(&temp.path().join("main.scss"), "library")
            .unwrap();
        assert!(!resolved.to_string_lossy().contains("vendor"));
    }

    #[test]
    fn resolve_from_directory_base() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        assert!(resolver.resolve(temp.path(), "variables").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_symlinks_lexically_like_dart_sass() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("shared")).unwrap();
        fs::write(temp.path().join("shared/_real.scss"), "").unwrap();
        std::os::unix::fs::symlink(
            temp.path().join("shared/_real.scss"),
            temp.path().join("_link.scss"),
        )
        .unwrap();

        let resolver = Resolver::default();
        let resolved = resolver
            .resolve_from_dir(temp.path(), "link", ResolveKind::Module)
            .unwrap();

        // The link path is kept, so the linked file's own relative imports
        // resolve from the link's directory (Dart Sass semantics).
        assert_eq!(resolved, temp.path().join("_link.scss"));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_absolute_target() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("_abs.scss"), "").unwrap();
        let target = temp.path().join("abs").to_string_lossy().into_owned();

        let resolver = Resolver::default();
        let resolved = resolver
            .resolve_from_dir(Path::new("/nonexistent"), &target, ResolveKind::Module)
            .unwrap();

        assert!(resolved.ends_with("_abs.scss"));
    }

    #[test]
    fn external_urls() {
        assert!(Resolver::is_external("sass:math"));
        assert!(Resolver::is_external("pkg:bootstrap"));
        assert!(Resolver::is_external("https://example.com/a"));
        assert!(!Resolver::is_external("variables"));
        assert!(!Resolver::is_external("C:/styles/a"));
        assert!(!Resolver::is_external("../a"));
    }

    #[test]
    fn normalize_lexically_resolves_dots() {
        assert_eq!(
            normalize_lexically(Path::new("/a/b/./c/../d")),
            PathBuf::from("/a/b/d")
        );
    }
}
