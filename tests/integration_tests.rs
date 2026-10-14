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
    assert_eq!(main.metrics.depth, Some(0));
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
    assert!(json.contains("\"version\": \"1.1.0\""));
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
        graph
            .build_from_entry(&entry, &resolver, &fixture_path)
            .unwrap();
        Analyzer::default().analyze(&mut graph);
        let schema = OutputSchema::from_graph(&graph, &fixture_path);
        Serializer::to_json(&schema).unwrap()
    };

    let json2 = {
        let mut graph = DependencyGraph::new();
        graph
            .build_from_entry(&entry, &resolver, &fixture_path)
            .unwrap();
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

    graph
        .build_from_entry(&entry, &resolver, &fixture_path)
        .unwrap();
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

    graph
        .build_from_entry(&entry, &resolver, &fixture_path)
        .unwrap();
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

    graph
        .build_from_entry(&entry, &resolver, &fixture_path)
        .unwrap();
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

    graph
        .build_from_entry(&entry, &resolver, &fixture_path)
        .unwrap();
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

    graph
        .build_from_entry(&root.join("a.scss"), &resolver, &root)
        .unwrap();
    Analyzer::default().analyze(&mut graph);

    assert_eq!(graph.get_node("a.scss").unwrap().metrics.depth, Some(0));
    assert_eq!(graph.get_node("_b.scss").unwrap().metrics.depth, Some(1));
    assert_eq!(graph.get_node("_c.scss").unwrap().metrics.depth, Some(2));
    assert_eq!(graph.get_node("_d.scss").unwrap().metrics.depth, Some(3));
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

    graph
        .build_from_entry(&root.join("a.scss"), &resolver, &root)
        .unwrap();
    Analyzer::default().analyze(&mut graph);

    // a depends on b, c, d (3 transitive deps)
    assert_eq!(graph.get_node("a.scss").unwrap().metrics.transitive_deps, 3);

    // b depends on c, d (2 transitive deps)
    assert_eq!(
        graph.get_node("_b.scss").unwrap().metrics.transitive_deps,
        2
    );

    // d depends on nothing
    assert_eq!(
        graph.get_node("_d.scss").unwrap().metrics.transitive_deps,
        0
    );
}

/// Tests high fan-in flag assignment.
#[test]
fn high_fan_in_flag() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    // Create a file depended on by many others
    fs::write(root.join("_shared.scss"), "$shared: 1;\n").unwrap();

    for i in 1..=6 {
        fs::write(root.join(format!("file{}.scss", i)), "@use \"shared\";\n").unwrap();
    }

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();

    for i in 1..=6 {
        graph
            .build_from_entry(&root.join(format!("file{}.scss", i)), &resolver, &root)
            .unwrap();
    }

    Analyzer::default().analyze(&mut graph);

    let shared = graph.get_node("_shared.scss").unwrap();
    assert_eq!(shared.metrics.fan_in, 6);
    assert!(shared.has_flag(&NodeFlag::HighFanIn));
}

/// Analyzes a temp project with a single entry point.
fn analyze_project(root: &Path, entry: &str) -> DependencyGraph {
    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();
    graph
        .build_from_entry(&root.join(entry), &resolver, root)
        .unwrap();
    Analyzer::default().analyze(&mut graph);
    graph
}

/// Cycles are ordered closed paths, and every SCC member is covered.
#[test]
fn cycles_are_ordered_and_cover_scc() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    // main -> a -> b -> c -> a, plus c -> d -> b (second cycle in the same SCC)
    fs::write(root.join("main.scss"), "@use \"a\";\n").unwrap();
    fs::write(root.join("_a.scss"), "@use \"b\";\n").unwrap();
    fs::write(root.join("_b.scss"), "@use \"c\";\n").unwrap();
    fs::write(root.join("_c.scss"), "@use \"a\";\n@use \"d\";\n").unwrap();
    fs::write(root.join("_d.scss"), "@use \"b\";\n").unwrap();

    let graph = analyze_project(&root, "main.scss");
    let cycles = graph.get_cycles();

    let edges: std::collections::HashSet<(String, String)> = graph
        .edges()
        .map(|(f, t, _)| (f.to_string(), t.to_string()))
        .collect();
    for cycle in cycles {
        for (i, from) in cycle.iter().enumerate() {
            let to = &cycle[(i + 1) % cycle.len()];
            assert!(
                edges.contains(&(from.clone(), to.clone())),
                "{from} -> {to} is not an edge"
            );
        }
    }

    let covered: std::collections::HashSet<&String> = cycles.iter().flatten().collect();
    for id in ["_a.scss", "_b.scss", "_c.scss", "_d.scss"] {
        assert!(covered.contains(&id.to_string()), "{id} not covered");
        assert!(graph.get_node(id).unwrap().has_flag(&NodeFlag::InCycle));
    }
    assert!(!graph
        .get_node("main.scss")
        .unwrap()
        .has_flag(&NodeFlag::InCycle));
    assert_eq!(cycles[0], vec!["_a.scss", "_b.scss", "_c.scss"]);
}

/// A file importing itself is a one-element cycle.
#[test]
fn self_import_is_a_cycle() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("main.scss"), "@use \"self\";\n").unwrap();
    fs::write(root.join("_self.scss"), "@use \"self\";\n").unwrap();

    let graph = analyze_project(&root, "main.scss");
    assert_eq!(graph.get_cycles(), &[vec!["_self.scss".to_string()]]);
    assert!(graph
        .get_node("_self.scss")
        .unwrap()
        .has_flag(&NodeFlag::InCycle));
}

/// Transitive dependents count everything that reaches a file.
#[test]
fn transitive_dependents_calculation() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    // Diamond: main -> {a, b} -> shared
    fs::write(root.join("main.scss"), "@use \"a\";\n@use \"b\";\n").unwrap();
    fs::write(root.join("_a.scss"), "@use \"shared\";\n").unwrap();
    fs::write(root.join("_b.scss"), "@use \"shared\";\n").unwrap();
    fs::write(root.join("_shared.scss"), "").unwrap();

    let graph = analyze_project(&root, "main.scss");
    let metrics = |id: &str| graph.get_node(id).unwrap().metrics.clone();

    assert_eq!(metrics("_shared.scss").transitive_dependents, 3);
    assert_eq!(metrics("_a.scss").transitive_dependents, 1);
    assert_eq!(metrics("main.scss").transitive_dependents, 0);
    assert_eq!(metrics("main.scss").transitive_deps, 3);
    assert_eq!(metrics("_shared.scss").depth, Some(2));
}

/// Orphans have no depth and serialize it as `null`.
#[test]
fn orphan_depth_is_null() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("main.scss"), "").unwrap();
    fs::write(root.join("_unused.scss"), "@use \"also-unused\";\n").unwrap();
    fs::write(root.join("_also-unused.scss"), "").unwrap();

    let resolver = Resolver::default();
    let mut graph = DependencyGraph::new();
    graph
        .build_from_entry(&root.join("main.scss"), &resolver, &root)
        .unwrap();
    graph.discover_orphans(&root, &resolver).unwrap();
    Analyzer::default().analyze(&mut graph);

    for id in ["_unused.scss", "_also-unused.scss"] {
        let node = graph.get_node(id).unwrap();
        assert_eq!(node.metrics.depth, None);
        assert!(node.has_flag(&NodeFlag::Orphan));
    }

    let schema = OutputSchema::from_graph(&graph, &root);
    assert_eq!(schema.analysis.statistics.orphan_files, 2);
    let json: serde_json::Value =
        serde_json::from_str(&Serializer::to_json(&schema).unwrap()).unwrap();
    assert!(json["nodes"]["_unused.scss"]["metrics"]["depth"].is_null());
}

/// Output is sorted and includes v1.1 fields.
#[test]
fn output_is_sorted_with_new_fields() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(
        root.join("main.scss"),
        "@use \"z\";\n@import \"a\";\n@use \"missing\";\n",
    )
    .unwrap();
    fs::write(root.join("_z.scss"), "").unwrap();
    fs::write(root.join("_a.scss"), "").unwrap();

    let graph = analyze_project(&root, "main.scss");
    let schema = OutputSchema::from_graph(&graph, &root);

    let ids: Vec<&String> = schema.nodes.keys().collect();
    assert_eq!(ids, vec!["_a.scss", "_z.scss", "main.scss"]);
    assert_eq!(schema.edges[0].to, "_a.scss");
    assert_eq!(schema.version, "1.1.0");
    assert_eq!(schema.metadata.entry_points, vec!["main.scss"]);
    assert_eq!(schema.analysis.statistics.import_directives, 1);
    assert_eq!(schema.analysis.statistics.unresolved_imports, 1);
    assert_eq!(schema.analysis.unresolved[0].target, "missing");
}

/// Documents produced by schema 1.0.0 are still readable.
#[test]
fn legacy_schema_is_normalized() {
    let legacy = r#"{
        "$schema": "x", "version": "1.0.0",
        "metadata": { "generated_at": "2024-01-15T10:30:00Z", "root": "/p", "sass_dep_version": "0.1.0" },
        "nodes": {
            "main.scss": { "path": "/p/main.scss", "metrics": { "fan_in": 0, "fan_out": 0, "depth": 0, "transitive_deps": 0 }, "flags": ["entry_point", "leaf"] },
            "_o.scss": { "path": "/p/_o.scss", "metrics": { "fan_in": 0, "fan_out": 0, "depth": 18446744073709551615, "transitive_deps": 0 }, "flags": ["orphan", "leaf"] }
        },
        "edges": [],
        "analysis": { "cycles": [], "statistics": { "total_files": 2, "total_dependencies": 0, "entry_points": 1, "orphan_files": 1, "leaf_files": 2, "max_depth": 18446744073709551615, "max_fan_in": 0, "max_fan_out": 0 } }
    }"#;

    let schema = OutputSchema::from_json(legacy).unwrap();
    assert_eq!(schema.nodes["_o.scss"].metrics.depth, None);
    assert_eq!(schema.analysis.statistics.max_depth, 0);
    assert!(Serializer::to_dot(&schema).contains("_o.scss"));
    assert!(OutputSchema::from_json("{}").is_err());
}

/// The fixture graph must not depend on thread scheduling.
#[test]
fn parallel_build_is_deterministic() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();

    let mut main = String::new();
    for i in 0..200 {
        main.push_str(&format!("@use \"m{i}\";\n"));
        fs::write(
            root.join(format!("_m{i}.scss")),
            format!("@use \"s{}\";\n", i % 7),
        )
        .unwrap();
    }
    for i in 0..7 {
        fs::write(root.join(format!("_s{i}.scss")), "").unwrap();
    }
    fs::write(root.join("main.scss"), main).unwrap();

    let ids = |graph: &DependencyGraph| graph.nodes().map(|(id, _)| id.clone()).collect::<Vec<_>>();
    let first = analyze_project(&root, "main.scss");
    for _ in 0..5 {
        assert_eq!(ids(&first), ids(&analyze_project(&root, "main.scss")));
    }
    assert_eq!(first.node_count(), 208);
    assert_eq!(
        first
            .get_node("_s0.scss")
            .unwrap()
            .metrics
            .transitive_dependents,
        30
    );
}
