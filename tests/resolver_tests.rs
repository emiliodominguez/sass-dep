//! Resolver integration tests.

use std::fs;
use std::path::PathBuf;

use sass_dep::resolver::{ResolveError, Resolver, ResolverConfig};
use tempfile::TempDir;

fn create_test_structure(dir: &std::path::Path) {
    // Simple files
    fs::write(dir.join("main.scss"), "").unwrap();
    fs::write(dir.join("_variables.scss"), "").unwrap();
    fs::write(dir.join("mixins.scss"), "").unwrap();

    // Nested structure
    fs::create_dir_all(dir.join("components")).unwrap();
    fs::write(dir.join("components/_index.scss"), "").unwrap();
    fs::write(dir.join("components/_button.scss"), "").unwrap();
    fs::write(dir.join("components/_card.scss"), "").unwrap();

    // Index without underscore
    fs::create_dir_all(dir.join("utils")).unwrap();
    fs::write(dir.join("utils/index.scss"), "").unwrap();

    // Sass extension
    fs::write(dir.join("indented.sass"), "").unwrap();

    // Vendor directory for load paths
    fs::create_dir_all(dir.join("vendor/library")).unwrap();
    fs::write(dir.join("vendor/library/_core.scss"), "").unwrap();
}

#[test]
fn resolve_simple_file_without_underscore() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let resolver = Resolver::default();
    let result = resolver.resolve(&temp.path().join("main.scss"), "mixins");

    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert!(resolved.ends_with("mixins.scss"));
}

#[test]
fn resolve_partial_with_underscore() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let resolver = Resolver::default();
    let result = resolver.resolve(&temp.path().join("main.scss"), "variables");

    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert!(resolved.ends_with("_variables.scss"));
}

#[test]
fn resolve_directory_with_underscore_index() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let resolver = Resolver::default();
    let result = resolver.resolve(&temp.path().join("main.scss"), "components");

    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert!(resolved.to_string_lossy().contains("components"));
    assert!(resolved.ends_with("_index.scss"));
}

#[test]
fn resolve_directory_with_index() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let resolver = Resolver::default();
    let result = resolver.resolve(&temp.path().join("main.scss"), "utils");

    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert!(resolved.to_string_lossy().contains("utils"));
    assert!(resolved.ends_with("index.scss"));
}

#[test]
fn resolve_nested_path() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let resolver = Resolver::default();
    let result = resolver.resolve(&temp.path().join("main.scss"), "components/button");

    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert!(resolved.ends_with("_button.scss"));
}

#[test]
fn resolve_with_sass_extension() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let resolver = Resolver::default();
    let result = resolver.resolve(&temp.path().join("main.scss"), "indented");

    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert!(resolved.ends_with("indented.sass"));
}

#[test]
fn resolve_not_found() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let resolver = Resolver::default();
    let result = resolver.resolve(&temp.path().join("main.scss"), "nonexistent");

    assert!(result.is_err());
    match result.unwrap_err() {
        ResolveError::NotFound { target, .. } => {
            assert_eq!(target, "nonexistent");
        }
        _ => panic!("Expected NotFound error"),
    }
}

#[test]
fn resolve_with_load_path() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let config = ResolverConfig {
        load_paths: vec![PathBuf::from("vendor/library")],
        extensions: vec!["scss".to_string(), "sass".to_string()],
    };
    let resolver = Resolver::new(config);

    let result = resolver.resolve(&temp.path().join("main.scss"), "core");

    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert!(resolved.to_string_lossy().contains("vendor"));
    assert!(resolved.ends_with("_core.scss"));
}

#[test]
fn resolve_prefers_relative_over_load_path() {
    let temp = TempDir::new().unwrap();

    // Create both relative and vendor versions
    fs::write(temp.path().join("_shared.scss"), "// relative").unwrap();
    fs::create_dir_all(temp.path().join("vendor")).unwrap();
    fs::write(temp.path().join("vendor/_shared.scss"), "// vendor").unwrap();
    fs::write(temp.path().join("main.scss"), "").unwrap();

    let config = ResolverConfig {
        load_paths: vec![PathBuf::from("vendor")],
        extensions: vec!["scss".to_string()],
    };
    let resolver = Resolver::new(config);

    let result = resolver.resolve(&temp.path().join("main.scss"), "shared");

    assert!(result.is_ok());
    let resolved = result.unwrap();
    // Should resolve to relative, not vendor
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
    let resolved = result.unwrap();
    assert!(resolved.ends_with("styles.scss"));
}

#[test]
fn resolve_prefers_non_partial_over_partial() {
    let temp = TempDir::new().unwrap();

    fs::write(temp.path().join("styles.scss"), "").unwrap();
    fs::write(temp.path().join("_styles.scss"), "").unwrap();
    fs::write(temp.path().join("main.scss"), "").unwrap();

    let resolver = Resolver::default();
    let result = resolver.resolve(&temp.path().join("main.scss"), "styles");

    assert!(result.is_ok());
    let resolved = result.unwrap();
    // Should prefer non-partial
    assert!(resolved.ends_with("styles.scss"));
    assert!(!resolved.to_string_lossy().contains("_styles"));
}

#[test]
fn resolve_from_directory() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let resolver = Resolver::default();
    // Pass directory instead of file
    let result = resolver.resolve(temp.path(), "variables");

    assert!(result.is_ok());
}

#[test]
fn resolve_relative_path_from_nested() {
    let temp = TempDir::new().unwrap();
    create_test_structure(temp.path());

    let resolver = Resolver::default();
    // Resolve from nested file going up
    let result = resolver.resolve(
        &temp.path().join("components/_button.scss"),
        "../variables",
    );

    assert!(result.is_ok());
    let resolved = result.unwrap();
    assert!(resolved.ends_with("_variables.scss"));
}

#[test]
fn resolver_config_accessors() {
    let config = ResolverConfig {
        load_paths: vec![PathBuf::from("vendor"), PathBuf::from("node_modules")],
        extensions: vec!["scss".to_string()],
    };
    let resolver = Resolver::new(config);

    assert_eq!(resolver.load_paths().len(), 2);
    assert_eq!(resolver.extensions(), &["scss"]);
}
