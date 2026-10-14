//! Path resolution implementation.
//!
//! This module implements Sass-compliant path resolution following
//! the official Sass specification, backed by a thread-safe **directory
//! listing cache** (`dir -> {canonical dir, files, dirs}`). Large projects
//! need one `read_dir` + one `realpath` per directory instead of a `stat`
//! per candidate file and a `realpath` per resolved import, and a single
//! [`Resolver`] can be shared by the parallel graph builder.

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

    /// File extensions to try, in order.
    ///
    /// Defaults to `["scss", "sass"]`.
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
/// `@import` has slightly different rules than the module system: it
/// prefers `*.import.scss` files and never loads plain `.css` files.
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

    /// The base path is invalid (not a file or directory).
    #[error("Invalid base path: {0}")]
    InvalidBasePath(PathBuf),

    /// IO error during resolution.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Cached listing of a directory.
#[derive(Debug, Default)]
struct DirListing {
    /// Canonical path of the directory (`None` if it does not exist).
    canonical: Option<PathBuf>,
    /// File names, mapped to whether the entry is a symlink.
    files: HashMap<OsString, bool>,
    /// Sub-directory names.
    dirs: HashSet<OsString>,
}

impl DirListing {
    /// Returns the canonical path of file `name` if it exists in this directory.
    ///
    /// Plain files reuse the directory's canonical path; only symlinked
    /// files pay for a `realpath`.
    fn file(&self, dir: &Path, name: &str) -> Option<PathBuf> {
        match self.files.get(OsStr::new(name))? {
            true => dunce::canonicalize(dir.join(name)).ok(),
            false => self
                .canonical
                .as_ref()
                .map(|canonical| canonical.join(name)),
        }
    }
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
    /// * `target` - The path string from the directive (e.g., `"variables"`)
    ///
    /// # Returns
    ///
    /// The canonical absolute path to the resolved file.
    ///
    /// # Resolution Order
    ///
    /// For `@use "foo"` from `/project/src/main.scss`:
    ///
    /// 1. `/project/src/foo.scss`
    /// 2. `/project/src/_foo.scss`
    /// 3. `/project/src/foo.sass`
    /// 4. `/project/src/_foo.sass`
    /// 5. `/project/src/foo.css` / `_foo.css` (module rules only)
    /// 6. `/project/src/foo/index.scss`, `/project/src/foo/_index.scss`, ...
    /// 7. Repeat for each load path
    ///
    /// Webpack-style `~pkg/path` targets are resolved against the load paths
    /// and then against `node_modules` directories walking up from `base`.
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
    /// Unlike [`Resolver::resolve`], this performs no `stat` on the base and
    /// is the entry point used by the graph builder.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError::NotFound`] if no candidate exists.
    pub fn resolve_from_dir(
        &self,
        base_dir: &Path,
        target: &str,
        kind: ResolveKind,
    ) -> Result<PathBuf, ResolveError> {
        let resolved = self.resolve_uncached(base_dir, target, kind);

        resolved.ok_or_else(|| ResolveError::NotFound {
            base: base_dir.to_path_buf(),
            target: target.to_string(),
        })
    }

    /// Performs resolution (directory listings are cached, results are not).
    fn resolve_uncached(
        &self,
        base_dir: &Path,
        target: &str,
        kind: ResolveKind,
    ) -> Option<PathBuf> {
        if let Some(package_path) = target.strip_prefix('~') {
            return self
                .resolve_in_load_paths(base_dir, package_path, kind)
                .or_else(|| self.resolve_in_node_modules(base_dir, package_path, kind));
        }

        self.try_resolve_in_dir(base_dir, target, kind)
            .or_else(|| self.resolve_in_load_paths(base_dir, target, kind))
    }

    /// Tries every configured load path in order.
    fn resolve_in_load_paths(
        &self,
        base_dir: &Path,
        target: &str,
        kind: ResolveKind,
    ) -> Option<PathBuf> {
        self.config.load_paths.iter().find_map(|load_path| {
            let load_dir = if load_path.is_absolute() {
                load_path.clone()
            } else {
                base_dir.join(load_path)
            };
            self.try_resolve_in_dir(&load_dir, target, kind)
        })
    }

    /// Tries `node_modules/<target>` in `base_dir` and each of its ancestors.
    fn resolve_in_node_modules(
        &self,
        base_dir: &Path,
        target: &str,
        kind: ResolveKind,
    ) -> Option<PathBuf> {
        base_dir
            .ancestors()
            .find_map(|dir| self.try_resolve_in_dir(&dir.join("node_modules"), target, kind))
    }

    /// Attempts to resolve a target in a specific directory.
    ///
    /// Returns `Some(path)` if found, `None` otherwise.
    fn try_resolve_in_dir(&self, dir: &Path, target: &str, kind: ResolveKind) -> Option<PathBuf> {
        // Sass URLs always use `/`; push segments one by one so this also
        // works under Windows verbatim (`\\?\`) prefixes.
        let mut joined = if target.starts_with('/') {
            Path::new("/").to_path_buf()
        } else {
            dir.to_path_buf()
        };
        joined.extend(target.split('/').filter(|segment| !segment.is_empty()));
        let joined = normalize_lexically(&joined);
        let search_dir = joined.parent()?;
        let file_name = joined.file_name()?.to_str()?;

        let (stem, explicit_ext) = match file_name.rsplit_once('.') {
            Some((stem, ext))
                if self.config.extensions.iter().any(|e| e == ext)
                    || (ext == "css" && kind == ResolveKind::Module) =>
            {
                (stem, Some(ext))
            }
            _ => (file_name, None),
        };

        let listing = self.listing(search_dir);

        // Explicit extension: only `name.ext` and `_name.ext` are candidates.
        if let Some(ext) = explicit_ext {
            return [format!("{stem}.{ext}"), format!("_{stem}.{ext}")]
                .into_iter()
                .find_map(|candidate| listing.file(search_dir, &candidate));
        }

        if let Some(found) = self.find_candidate(&listing, search_dir, stem, kind) {
            return Some(found);
        }

        // Directory import: `name/index.*` and `name/_index.*`.
        if listing.dirs.contains(&OsString::from(stem)) {
            let index_dir = search_dir.join(stem);
            let index_listing = self.listing(&index_dir);
            return self.find_candidate(&index_listing, &index_dir, "index", kind);
        }

        None
    }

    /// Finds the first existing candidate file for `stem` in a listed directory.
    fn find_candidate(
        &self,
        listing: &DirListing,
        dir: &Path,
        stem: &str,
        kind: ResolveKind,
    ) -> Option<PathBuf> {
        if listing.files.is_empty() {
            return None;
        }

        let try_name = |name: String| listing.file(dir, &name);

        if kind == ResolveKind::Import {
            for ext in &self.config.extensions {
                if let Some(found) = try_name(format!("{stem}.import.{ext}"))
                    .or_else(|| try_name(format!("_{stem}.import.{ext}")))
                {
                    return Some(found);
                }
            }
        }

        for ext in &self.config.extensions {
            if let Some(found) =
                try_name(format!("{stem}.{ext}")).or_else(|| try_name(format!("_{stem}.{ext}")))
            {
                return Some(found);
            }
        }

        if kind == ResolveKind::Module {
            return try_name(format!("{stem}.css")).or_else(|| try_name(format!("_{stem}.css")));
        }

        None
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

/// Reads a directory into a [`DirListing`], following symlinks.
fn read_listing(dir: &Path) -> DirListing {
    let mut listing = DirListing::default();

    let Ok(entries) = fs::read_dir(dir) else {
        return listing;
    };

    listing.canonical = dunce::canonicalize(dir).ok();

    for entry in entries.flatten() {
        let Ok(mut file_type) = entry.file_type() else {
            continue;
        };
        let is_symlink = file_type.is_symlink();
        if is_symlink {
            match fs::metadata(entry.path()) {
                Ok(meta) => file_type = meta.file_type(),
                Err(_) => continue,
            }
        }
        if file_type.is_dir() {
            listing.dirs.insert(entry.file_name());
        } else if file_type.is_file() {
            listing.files.insert(entry.file_name(), is_symlink);
        }
    }

    listing
}

/// Lexically resolves `.` and `..` components without touching the filesystem.
fn normalize_lexically(path: &Path) -> PathBuf {
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
    fn resolve_css_only_for_module_rules() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("normalize.css"), "").unwrap();

        let resolver = Resolver::default();
        assert!(resolver
            .resolve_from_dir(temp.path(), "normalize", ResolveKind::Module)
            .unwrap()
            .ends_with("normalize.css"));
        assert!(resolver
            .resolve_from_dir(temp.path(), "normalize", ResolveKind::Import)
            .is_err());
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
    fn resolve_prefers_scss_over_sass() {
        let temp = TempDir::new().unwrap();

        fs::write(temp.path().join("styles.scss"), "").unwrap();
        fs::write(temp.path().join("styles.sass"), "").unwrap();
        fs::write(temp.path().join("main.scss"), "").unwrap();

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "styles");

        assert!(result.unwrap().ends_with("styles.scss"));
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
    fn resolve_symlinked_file_to_its_target() {
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

        assert!(resolved.ends_with("shared/_real.scss"));
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
