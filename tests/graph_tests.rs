//! Graph integration tests.

use std::fs;
use std::path::Path;

use sass_dep::graph::{DependencyGraph, NodeFlag};
use sass_dep::resolver::Resolver;
use tempfile::TempDir;

fn create_simple_project(dir: &Path) {
    fs::write(
        dir.join("main.scss"),
        r#"@use "variables" as vars;
@use "mixins";

.container { color: vars.$primary; }
"#,
    )
    .unwrap();

    fs::write(dir.join("_variables.scss"), "$primary: blue;\n").unwrap();

    fs::write(
        dir.join("_mixins.scss"),
        r#"@use "variables" as vars;

@mixin center { display: flex; }
"#,
    )
    .unwrap();
}

fn create_cycle_project(dir: &Path) {
    fs::write(dir.join("_a.scss"), "@use \"b\";\n$a: 1;\n").unwrap();
    fs::write(dir.join("_b.scss"), "@use \"c\";\n$b: 2;\n").unwrap();
    fs::write(dir.join("_c.scss"), "@use \"a\";\n$c: 3;\n").unwrap();
}

fn create_legacy_project(dir: &Path) {
    fs::write(
        dir.join("main.scss"),
        "@import \"imported\";\n.legacy { color: $old-var; }\n",
    )
    .unwrap();
    fs::write(dir.join("_imported.scss"), "$old-var: red;\n").unwrap();
}

#[test]
fn build_simple_graph() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    create_simple_project(&root);

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    let result = graph.build_from_entry(&root.join("main.scss"), &resolver, &root);

    assert!(result.is_ok());
    assert_eq!(graph.node_count(), 3); // main, variables, mixins
    assert_eq!(graph.edge_count(), 3); // main->vars, main->mixins, mixins->vars
}

#[test]
fn entry_point_flagged() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    create_simple_project(&root);

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&root.join("main.scss"), &resolver, &root)
        .unwrap();

    let main = graph.get_node("main.scss").unwrap();
    assert!(main.has_flag(&NodeFlag::EntryPoint));

    let vars = graph.get_node("_variables.scss").unwrap();
    assert!(!vars.has_flag(&NodeFlag::EntryPoint));
}

#[test]
fn multiple_entry_points() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    // Create two separate entry points
    fs::write(root.join("app.scss"), "@use \"shared\";\n").unwrap();
    fs::write(root.join("admin.scss"), "@use \"shared\";\n").unwrap();
    fs::write(root.join("_shared.scss"), "$color: red;\n").unwrap();

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&root.join("app.scss"), &resolver, &root)
        .unwrap();
    graph
        .build_from_entry(&root.join("admin.scss"), &resolver, &root)
        .unwrap();

    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.entry_points().len(), 2);

    let app = graph.get_node("app.scss").unwrap();
    let admin = graph.get_node("admin.scss").unwrap();
    assert!(app.has_flag(&NodeFlag::EntryPoint));
    assert!(admin.has_flag(&NodeFlag::EntryPoint));
}

#[test]
fn graph_with_cycle() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    create_cycle_project(&root);

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    // This should still work - cycles are detected but don't cause infinite loops
    let result = graph.build_from_entry(&root.join("_a.scss"), &resolver, &root);

    assert!(result.is_ok());
    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.edge_count(), 3);
}

#[test]
fn graph_with_legacy_import() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    create_legacy_project(&root);

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&root.join("main.scss"), &resolver, &root)
        .unwrap();

    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn relative_file_ids() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    fs::create_dir_all(root.join("src/components")).unwrap();
    fs::write(root.join("src/main.scss"), "@use \"components/button\";\n").unwrap();
    fs::write(root.join("src/components/_button.scss"), "").unwrap();

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&root.join("src/main.scss"), &resolver, &root)
        .unwrap();

    // IDs should be relative paths from root
    assert!(graph.get_node("src/main.scss").is_some());
    assert!(graph.get_node("src/components/_button.scss").is_some());
}

#[test]
fn graph_edges() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    create_simple_project(&root);

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&root.join("main.scss"), &resolver, &root)
        .unwrap();

    let edges: Vec<_> = graph.edges().collect();
    assert_eq!(edges.len(), 3);

    // Check that all edges have valid from/to
    for (from, to, _edge) in edges {
        assert!(graph.get_node(from).is_some());
        assert!(graph.get_node(to).is_some());
    }
}

#[test]
fn discover_orphans() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    create_simple_project(&root);

    // Add an orphan file
    fs::write(root.join("_orphan.scss"), "$orphan: true;\n").unwrap();

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&root.join("main.scss"), &resolver, &root)
        .unwrap();

    // Before discovering orphans
    assert_eq!(graph.node_count(), 3);
    assert!(graph.get_node("_orphan.scss").is_none());

    // Discover orphans
    graph.discover_orphans(&root, &resolver).unwrap();

    // After discovering orphans
    assert_eq!(graph.node_count(), 4);

    let orphan = graph.get_node("_orphan.scss").unwrap();
    assert!(orphan.has_flag(&NodeFlag::Orphan));
}

#[test]
fn graph_with_forward_directives() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    fs::create_dir_all(root.join("base")).unwrap();
    fs::write(
        root.join("main.scss"),
        "@use \"base\";\n",
    )
    .unwrap();
    fs::write(
        root.join("base/_index.scss"),
        "@forward \"reset\";\n@forward \"typography\";\n",
    )
    .unwrap();
    fs::write(root.join("base/_reset.scss"), "@mixin reset {}\n").unwrap();
    fs::write(root.join("base/_typography.scss"), "@mixin type {}\n").unwrap();

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&root.join("main.scss"), &resolver, &root)
        .unwrap();

    // main -> base/_index.scss -> reset, typography
    assert_eq!(graph.node_count(), 4);
    assert_eq!(graph.edge_count(), 3);
}

#[test]
fn graph_node_iteration_order() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    create_simple_project(&root);

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&root.join("main.scss"), &resolver, &root)
        .unwrap();

    // Collect node IDs
    let ids1: Vec<_> = graph.nodes().map(|(id, _)| id.clone()).collect();

    // Rebuild graph
    let mut graph2 = DependencyGraph::new();
    graph2
        .build_from_entry(&root.join("main.scss"), &resolver, &root)
        .unwrap();

    let ids2: Vec<_> = graph2.nodes().map(|(id, _)| id.clone()).collect();

    // Order should be deterministic
    assert_eq!(ids1, ids2);
}
