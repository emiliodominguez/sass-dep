//! Path resolution implementation.
//!
//! This module implements Sass-compliant path resolution following
//! the official Sass specification.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Configuration for the path resolver.
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    /// Additional directories to search for imports.
    ///
    /// These are searched after the relative path from the importing file.
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

/// Sass-compliant path resolver.
///
/// Resolves `@use`, `@forward`, and `@import` paths according to
/// Sass conventions.
#[derive(Debug, Clone)]
pub struct Resolver {
    config: ResolverConfig,
}

impl Resolver {
    /// Creates a new resolver with the given configuration.
    pub fn new(config: ResolverConfig) -> Self {
        Self { config }
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
    /// 2. `/project/src/foo.sass`
    /// 3. `/project/src/_foo.scss`
    /// 4. `/project/src/_foo.sass`
    /// 5. `/project/src/foo/index.scss`
    /// 6. `/project/src/foo/_index.scss`
    /// 7. `/project/src/foo/index.sass`
    /// 8. `/project/src/foo/_index.sass`
    /// 9. Repeat for each load path
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
        // Determine the base directory
        let base_dir = if base.is_file() {
            base.parent().ok_or_else(|| ResolveError::InvalidBasePath(base.to_path_buf()))?
        } else if base.is_dir() {
            base
        } else {
            return Err(ResolveError::InvalidBasePath(base.to_path_buf()));
        };

        // Try relative resolution first
        if let Some(resolved) = self.try_resolve_in_dir(base_dir, target) {
            return Ok(resolved);
        }

        // Try each load path
        for load_path in &self.config.load_paths {
            let load_dir = if load_path.is_absolute() {
                load_path.clone()
            } else {
                base_dir.join(load_path)
            };

            if let Some(resolved) = self.try_resolve_in_dir(&load_dir, target) {
                return Ok(resolved);
            }
        }

        Err(ResolveError::NotFound {
            base: base_dir.to_path_buf(),
            target: target.to_string(),
        })
    }

    /// Attempts to resolve a target in a specific directory.
    ///
    /// Returns `Some(path)` if found, `None` otherwise.
    fn try_resolve_in_dir(&self, dir: &Path, target: &str) -> Option<PathBuf> {
        // Parse the target path
        let target_path = Path::new(target);

        // Get the parent directory and file stem from the target
        let (target_dir, file_stem) = if let Some(parent) = target_path.parent() {
            if parent.as_os_str().is_empty() {
                (None, target_path.to_string_lossy().to_string())
            } else {
                (
                    Some(parent.to_path_buf()),
                    target_path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                )
            }
        } else {
            (None, target.to_string())
        };

        // Build the search directory
        let search_dir = match &target_dir {
            Some(td) => dir.join(td),
            None => dir.to_path_buf(),
        };

        // Try direct file matches
        for ext in &self.config.extensions {
            // Try without underscore prefix
            let path = search_dir.join(format!("{}.{}", file_stem, ext));
            if path.is_file() {
                return path.canonicalize().ok();
            }

            // Try with underscore prefix (partial)
            let path = search_dir.join(format!("_{}.{}", file_stem, ext));
            if path.is_file() {
                return path.canonicalize().ok();
            }
        }

        // Try index file resolution (for directory imports)
        let index_dir = search_dir.join(&file_stem);
        if index_dir.is_dir() {
            for ext in &self.config.extensions {
                // Try index without underscore
                let path = index_dir.join(format!("index.{}", ext));
                if path.is_file() {
                    return path.canonicalize().ok();
                }

                // Try index with underscore
                let path = index_dir.join(format!("_index.{}", ext));
                if path.is_file() {
                    return path.canonicalize().ok();
                }
            }
        }

        None
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_files(dir: &Path) {
        // Create test file structure
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

        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("mixins.scss"));
    }

    #[test]
    fn resolve_partial_file() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "variables");

        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("_variables.scss"));
    }

    #[test]
    fn resolve_directory_with_underscore_index() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "components");

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(resolved.ends_with("_index.scss"));
        assert!(resolved.to_string_lossy().contains("components"));
    }

    #[test]
    fn resolve_directory_with_index() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "utils");

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(resolved.ends_with("index.scss"));
        assert!(resolved.to_string_lossy().contains("utils"));
    }

    #[test]
    fn resolve_nested_path() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "components/button");

        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("_button.scss"));
    }

    #[test]
    fn resolve_not_found() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(&temp.path().join("main.scss"), "nonexistent");

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ResolveError::NotFound { .. }));
    }

    #[test]
    fn resolve_with_load_path() {
        let temp = TempDir::new().unwrap();

        // Create files in a vendor directory
        let vendor_dir = temp.path().join("vendor");
        fs::create_dir_all(&vendor_dir).unwrap();
        fs::write(vendor_dir.join("_library.scss"), "").unwrap();

        // Create main file
        fs::write(temp.path().join("main.scss"), "").unwrap();

        let config = ResolverConfig {
            load_paths: vec![PathBuf::from("vendor")],
            extensions: vec!["scss".to_string()],
        };
        let resolver = Resolver::new(config);

        let result = resolver.resolve(&temp.path().join("main.scss"), "library");

        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("_library.scss"));
    }

    #[test]
    fn resolve_prefers_relative_over_load_path() {
        let temp = TempDir::new().unwrap();

        // Create files in both relative and vendor
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

        let result = resolver.resolve(&temp.path().join("main.scss"), "library");

        assert!(result.is_ok());
        // Should resolve to the relative path, not vendor
        let resolved = result.unwrap();
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

        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("styles.scss"));
    }

    #[test]
    fn resolve_from_directory_base() {
        let temp = TempDir::new().unwrap();
        setup_test_files(temp.path());

        let resolver = Resolver::default();
        let result = resolver.resolve(temp.path(), "variables");

        assert!(result.is_ok());
    }
}
