//! End-to-end integration tests.

use std::fs;
use std::path::Path;

use sass_dep::analyzer::Analyzer;
use sass_dep::graph::{DependencyGraph, NodeFlag};
use sass_dep::output::{OutputSchema, Serializer};
use sass_dep::resolver::Resolver;
use tempfile::TempDir;

/// Tests the full analysis pipeline on the simple fixture.
#[test]
fn analyze_simple_fixture() {
    let fixture_path = Path::new("tests/fixtures/simple").canonicalize().unwrap();
    let entry = fixture_path.join("main.scss");

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&entry, &resolver, &fixture_path)
        .unwrap();

    let analyzer = Analyzer::default();
    analyzer.analyze(&mut graph);

    // Verify node count
    assert_eq!(graph.node_count(), 3);

    // Verify entry point
    let main = graph.get_node("main.scss").unwrap();
    assert!(main.has_flag(&NodeFlag::EntryPoint));
    assert_eq!(main.metrics.depth, 0);
    assert_eq!(main.metrics.fan_out, 2);

    // Verify leaf node
    let vars = graph.get_node("_variables.scss").unwrap();
    assert!(vars.has_flag(&NodeFlag::Leaf));
    assert_eq!(vars.metrics.fan_out, 0);
    assert_eq!(vars.metrics.fan_in, 2); // main and mixins depend on it
}

/// Tests cycle detection on the cycles fixture.
#[test]
fn analyze_cycles_fixture() {
    let fixture_path = Path::new("tests/fixtures/cycles").canonicalize().unwrap();
    let entry = fixture_path.join("_a.scss");

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&entry, &resolver, &fixture_path)
        .unwrap();

    let analyzer = Analyzer::default();
    analyzer.analyze(&mut graph);

    // Should detect cycles
    let cycles = graph.get_cycles();
    assert!(!cycles.is_empty(), "Should detect at least one cycle");

    // All nodes should be marked as in cycle
    for (id, node) in graph.nodes() {
        assert!(
            node.has_flag(&NodeFlag::InCycle),
            "Node {} should be in cycle",
            id
        );
    }
}

/// Tests legacy @import handling.
#[test]
fn analyze_legacy_fixture() {
    let fixture_path = Path::new("tests/fixtures/legacy").canonicalize().unwrap();
    let entry = fixture_path.join("main.scss");

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&entry, &resolver, &fixture_path)
        .unwrap();

    let analyzer = Analyzer::default();
    analyzer.analyze(&mut graph);

    assert_eq!(graph.node_count(), 2);

    // Verify the import edge exists
    let edges: Vec<_> = graph.edges().collect();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].1, "_imported.scss");
}

/// Tests complex nested structure.
#[test]
fn analyze_complex_fixture() {
    let fixture_path = Path::new("tests/fixtures/complex").canonicalize().unwrap();
    let entry = fixture_path.join("main.scss");

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&entry, &resolver, &fixture_path)
        .unwrap();

    let analyzer = Analyzer::default();
    analyzer.analyze(&mut graph);

    // Complex fixture has: main, base/_index, base/_reset, base/_typography,
    // components/_index, components/_button, components/_card
    assert!(graph.node_count() >= 7);

    // Verify main is entry point
    let main = graph.get_node("main.scss").unwrap();
    assert!(main.has_flag(&NodeFlag::EntryPoint));
}

/// Tests JSON output schema generation.
#[test]
fn generate_json_output() {
    let fixture_path = Path::new("tests/fixtures/simple").canonicalize().unwrap();
    let entry = fixture_path.join("main.scss");

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph
        .build_from_entry(&entry, &resolver, &fixture_path)
        .unwrap();

    let analyzer = Analyzer::default();
    analyzer.analyze(&mut graph);

    let schema = OutputSchema::from_graph(&graph, &fixture_path);
    let json = Serializer::to_json(&schema).unwrap();

    // Verify JSON structure
    assert!(json.contains("\"version\": \"1.0.0\""));
    assert!(json.contains("\"nodes\""));
    assert!(json.contains("\"edges\""));
    assert!(json.contains("\"analysis\""));
    assert!(json.contains("\"statistics\""));

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("nodes").is_some());
    assert!(parsed.get("edges").is_some());
}

/// Tests JSON output is deterministic.
#[test]
fn json_output_deterministic() {
    let fixture_path = Path::new("tests/fixtures/simple").canonicalize().unwrap();
    let entry = fixture_path.join("main.scss");
    let resolver = Resolver::default();

    // Generate output twice
    let json1 = {
        let mut graph = DependencyGraph::new();
        graph.build_from_entry(&entry, &resolver, &fixture_path).unwrap();
        Analyzer::default().analyze(&mut graph);
        let schema = OutputSchema::from_graph(&graph, &fixture_path);
        Serializer::to_json(&schema).unwrap()
    };

    let json2 = {
        let mut graph = DependencyGraph::new();
        graph.build_from_entry(&entry, &resolver, &fixture_path).unwrap();
        Analyzer::default().analyze(&mut graph);
        let schema = OutputSchema::from_graph(&graph, &fixture_path);
        Serializer::to_json(&schema).unwrap()
    };

    // Remove timestamps for comparison
    let normalize = |s: &str| {
        s.lines()
            .filter(|l| !l.contains("generated_at"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert_eq!(normalize(&json1), normalize(&json2));
}

/// Tests DOT export format.
#[test]
fn export_dot_format() {
    let fixture_path = Path::new("tests/fixtures/simple").canonicalize().unwrap();
    let entry = fixture_path.join("main.scss");

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph.build_from_entry(&entry, &resolver, &fixture_path).unwrap();
    Analyzer::default().analyze(&mut graph);

    let schema = OutputSchema::from_graph(&graph, &fixture_path);
    let dot = Serializer::to_dot(&schema);

    assert!(dot.starts_with("digraph dependencies {"));
    assert!(dot.ends_with("}\n"));
    assert!(dot.contains("->"));
}

/// Tests Mermaid export format.
#[test]
fn export_mermaid_format() {
    let fixture_path = Path::new("tests/fixtures/simple").canonicalize().unwrap();
    let entry = fixture_path.join("main.scss");

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph.build_from_entry(&entry, &resolver, &fixture_path).unwrap();
    Analyzer::default().analyze(&mut graph);

    let schema = OutputSchema::from_graph(&graph, &fixture_path);
    let mermaid = Serializer::to_mermaid(&schema);

    assert!(mermaid.starts_with("graph LR"));
    assert!(mermaid.contains("classDef"));
}

/// Tests D2 export format.
#[test]
fn export_d2_format() {
    let fixture_path = Path::new("tests/fixtures/simple").canonicalize().unwrap();
    let entry = fixture_path.join("main.scss");

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph.build_from_entry(&entry, &resolver, &fixture_path).unwrap();
    Analyzer::default().analyze(&mut graph);

    let schema = OutputSchema::from_graph(&graph, &fixture_path);
    let d2 = Serializer::to_d2(&schema);

    assert!(d2.starts_with("direction: right"));
    assert!(d2.contains("->"));
}

/// Tests statistics calculation.
#[test]
fn statistics_accuracy() {
    let fixture_path = Path::new("tests/fixtures/simple").canonicalize().unwrap();
    let entry = fixture_path.join("main.scss");

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph.build_from_entry(&entry, &resolver, &fixture_path).unwrap();
    Analyzer::default().analyze(&mut graph);

    let schema = OutputSchema::from_graph(&graph, &fixture_path);
    let stats = &schema.analysis.statistics;

    assert_eq!(stats.total_files, 3);
    assert_eq!(stats.total_dependencies, 3);
    assert_eq!(stats.entry_points, 1);
    assert_eq!(stats.orphan_files, 0);
    assert!(stats.leaf_files >= 1); // At least _variables.scss is a leaf
}

/// Tests depth calculation.
#[test]
fn depth_calculation() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    // Create a chain: a -> b -> c -> d
    fs::write(root.join("a.scss"), "@use \"b\";\n").unwrap();
    fs::write(root.join("_b.scss"), "@use \"c\";\n").unwrap();
    fs::write(root.join("_c.scss"), "@use \"d\";\n").unwrap();
    fs::write(root.join("_d.scss"), "$d: 1;\n").unwrap();

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph.build_from_entry(&root.join("a.scss"), &resolver, &root).unwrap();
    Analyzer::default().analyze(&mut graph);

    assert_eq!(graph.get_node("a.scss").unwrap().metrics.depth, 0);
    assert_eq!(graph.get_node("_b.scss").unwrap().metrics.depth, 1);
    assert_eq!(graph.get_node("_c.scss").unwrap().metrics.depth, 2);
    assert_eq!(graph.get_node("_d.scss").unwrap().metrics.depth, 3);
}

/// Tests transitive dependency calculation.
#[test]
fn transitive_deps_calculation() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    // Create a chain: a -> b -> c -> d
    fs::write(root.join("a.scss"), "@use \"b\";\n").unwrap();
    fs::write(root.join("_b.scss"), "@use \"c\";\n").unwrap();
    fs::write(root.join("_c.scss"), "@use \"d\";\n").unwrap();
    fs::write(root.join("_d.scss"), "$d: 1;\n").unwrap();

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    graph.build_from_entry(&root.join("a.scss"), &resolver, &root).unwrap();
    Analyzer::default().analyze(&mut graph);

    // a depends on b, c, d (3 transitive deps)
    assert_eq!(graph.get_node("a.scss").unwrap().metrics.transitive_deps, 3);

    // b depends on c, d (2 transitive deps)
    assert_eq!(graph.get_node("_b.scss").unwrap().metrics.transitive_deps, 2);

    // d depends on nothing
    assert_eq!(graph.get_node("_d.scss").unwrap().metrics.transitive_deps, 0);
}

/// Tests high fan-in flag assignment.
#[test]
fn high_fan_in_flag() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    // Create a file depended on by many others
    fs::write(root.join("_shared.scss"), "$shared: 1;\n").unwrap();

    for i in 1..=6 {
        fs::write(
            root.join(format!("file{}.scss", i)),
            "@use \"shared\";\n",
        )
        .unwrap();
    }

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    for i in 1..=6 {
        graph
            .build_from_entry(
                &root.join(format!("file{}.scss", i)),
                &resolver,
                &root,
            )
            .unwrap();
    }

    Analyzer::default().analyze(&mut graph);

    let shared = graph.get_node("_shared.scss").unwrap();
    assert_eq!(shared.metrics.fan_in, 6);
    assert!(shared.has_flag(&NodeFlag::HighFanIn));
}
