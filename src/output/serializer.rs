//! Rendering of [`OutputSchema`] documents.
//!
//! All text formats are produced into a single pre-sized `String` with
//! `fmt::Write`, avoiding per-line allocations on large graphs.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Write;

use anyhow::Result;

use super::schema::OutputSchema;
use crate::graph::{DirectiveType, NodeFlag};

/// Visual style for each primary flag: `(name, fill, stroke)`.
const FLAG_STYLES: [(NodeFlag, &str, &str, &str); 6] = [
    (NodeFlag::EntryPoint, "entry", "#dcfce7", "#16a34a"),
    (NodeFlag::InCycle, "cycle", "#fef3c7", "#d97706"),
    (NodeFlag::Orphan, "orphan", "#fee2e2", "#dc2626"),
    (NodeFlag::HighFanIn, "fanin", "#dbeafe", "#2563eb"),
    (NodeFlag::HighFanOut, "fanout", "#f3e8ff", "#9333ea"),
    (NodeFlag::Leaf, "leaf", "#f1f5f9", "#64748b"),
];

/// Color used for edges that close a cycle.
const CYCLE_EDGE_COLOR: &str = "#dc2626";

/// Serializer for the supported output formats.
pub struct Serializer;

impl Serializer {
    /// Serializes the schema as pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(schema: &OutputSchema) -> Result<String> {
        Ok(serde_json::to_string_pretty(schema)?)
    }

    /// Streams the schema as pretty-printed JSON into `writer`.
    ///
    /// Prefer this over [`Serializer::to_json`] for large graphs: it never
    /// materializes the whole document in memory.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or writing fails.
    pub fn write_json<W: Write>(schema: &OutputSchema, writer: W) -> Result<()> {
        serde_json::to_writer_pretty(writer, schema)?;
        Ok(())
    }

    /// Renders the graph in Graphviz DOT format.
    pub fn to_dot(schema: &OutputSchema) -> String {
        let cycle_edges = cycle_edges(schema);
        let mut out = String::with_capacity(64 * (schema.nodes.len() + schema.edges.len()) + 256);

        out.push_str("digraph dependencies {\n");
        out.push_str("  rankdir=LR;\n");
        out.push_str("  node [shape=box, style=\"rounded,filled\", fontname=\"Helvetica\", fillcolor=\"#ffffff\", color=\"#94a3b8\"];\n");
        out.push_str("  edge [fontname=\"Helvetica\", fontsize=10, color=\"#64748b\"];\n\n");

        for (id, node) in &schema.nodes {
            let _ = write!(
                out,
                "  \"{}\" [label=\"{}\"",
                dot_escape(id),
                dot_escape(id)
            );
            if let Some((_, _, fill, stroke)) = primary_style(&node.flags) {
                let _ = write!(out, ", fillcolor=\"{fill}\", color=\"{stroke}\"");
            }
            out.push_str("];\n");
        }

        if !schema.edges.is_empty() {
            out.push('\n');
        }

        for edge in &schema.edges {
            let _ = write!(
                out,
                "  \"{}\" -> \"{}\"",
                dot_escape(&edge.from),
                dot_escape(&edge.to)
            );
            let mut attrs: Vec<String> = Vec::new();
            match edge.directive_type {
                DirectiveType::Use => {}
                DirectiveType::Forward => attrs.push("style=dashed".to_string()),
                DirectiveType::Import => attrs.push("style=dotted".to_string()),
            }
            if cycle_edges.contains(&(edge.from.as_str(), edge.to.as_str())) {
                attrs.push(format!("color=\"{CYCLE_EDGE_COLOR}\", penwidth=2"));
            }
            if !attrs.is_empty() {
                let _ = write!(out, " [{}]", attrs.join(", "));
            }
            out.push_str(";\n");
        }

        out.push_str("}\n");
        out
    }

    /// Renders the graph as a Mermaid flowchart.
    ///
    /// Node IDs are replaced by short synthetic identifiers (`n0`, `n1`, ...)
    /// because Mermaid IDs cannot contain path characters.
    pub fn to_mermaid(schema: &OutputSchema) -> String {
        let ids: HashMap<&str, usize> = schema
            .nodes
            .keys()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let cycle_edges = cycle_edges(schema);
        let mut out = String::with_capacity(48 * (schema.nodes.len() + schema.edges.len()) + 512);

        out.push_str("graph LR\n");

        for (i, id) in schema.nodes.keys().enumerate() {
            let _ = writeln!(out, "  n{i}[\"{}\"]", mermaid_escape(id));
        }

        let mut cycle_links = Vec::new();
        for (link, edge) in schema.edges.iter().enumerate() {
            let (Some(from), Some(to)) = (ids.get(edge.from.as_str()), ids.get(edge.to.as_str()))
            else {
                continue;
            };
            let arrow = match edge.directive_type {
                DirectiveType::Use => "-->",
                DirectiveType::Forward => "-.->",
                DirectiveType::Import => "==>",
            };
            let _ = writeln!(out, "  n{from} {arrow} n{to}");
            if cycle_edges.contains(&(edge.from.as_str(), edge.to.as_str())) {
                cycle_links.push(link.to_string());
            }
        }

        out.push('\n');
        for (_, class, fill, stroke) in FLAG_STYLES {
            let _ = writeln!(
                out,
                "  classDef {class} fill:{fill},stroke:{stroke},color:#0f172a"
            );
        }

        for (i, node) in schema.nodes.values().enumerate() {
            if let Some((_, class, _, _)) = primary_style(&node.flags) {
                let _ = writeln!(out, "  class n{i} {class}");
            }
        }

        if !cycle_links.is_empty() {
            let _ = writeln!(
                out,
                "  linkStyle {} stroke:{CYCLE_EDGE_COLOR},stroke-width:2px",
                cycle_links.join(",")
            );
        }

        out
    }

    /// Renders the graph as a D2 diagram.
    pub fn to_d2(schema: &OutputSchema) -> String {
        let cycle_edges = cycle_edges(schema);
        let mut out = String::with_capacity(64 * (schema.nodes.len() + schema.edges.len()) + 64);

        out.push_str("direction: right\n\n");

        for (id, node) in &schema.nodes {
            let key = d2_escape(id);
            match primary_style(&node.flags) {
                Some((_, _, fill, stroke)) => {
                    let _ = writeln!(
                        out,
                        "\"{key}\": {{\n  style.fill: \"{fill}\"\n  style.stroke: \"{stroke}\"\n}}"
                    );
                }
                None => {
                    let _ = writeln!(out, "\"{key}\"");
                }
            }
        }

        if !schema.edges.is_empty() {
            out.push('\n');
        }

        for edge in &schema.edges {
            let _ = write!(
                out,
                "\"{}\" -> \"{}\"",
                d2_escape(&edge.from),
                d2_escape(&edge.to)
            );
            let dash = match edge.directive_type {
                DirectiveType::Use => None,
                DirectiveType::Forward => Some(5),
                DirectiveType::Import => Some(2),
            };
            let in_cycle = cycle_edges.contains(&(edge.from.as_str(), edge.to.as_str()));
            if dash.is_some() || in_cycle {
                out.push_str(": {\n");
                if let Some(dash) = dash {
                    let _ = writeln!(out, "  style.stroke-dash: {dash}");
                }
                if in_cycle {
                    let _ = writeln!(out, "  style.stroke: \"{CYCLE_EDGE_COLOR}\"");
                }
                out.push('}');
            }
            out.push('\n');
        }

        out
    }
}

/// Returns the style of the highest-priority flag on a node.
fn primary_style(
    flags: &[NodeFlag],
) -> Option<(NodeFlag, &'static str, &'static str, &'static str)> {
    FLAG_STYLES
        .into_iter()
        .find(|(flag, ..)| flags.contains(flag))
}

/// Collects `(from, to)` pairs of edges that belong to a reported cycle.
fn cycle_edges(schema: &OutputSchema) -> HashSet<(&str, &str)> {
    let mut edges = HashSet::new();
    for cycle in &schema.analysis.cycles {
        for (i, from) in cycle.iter().enumerate() {
            let to = &cycle[(i + 1) % cycle.len()];
            edges.insert((from.as_str(), to.as_str()));
        }
    }
    edges
}

/// Escapes a string for use inside a double-quoted DOT ID.
fn dot_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Escapes a string for use inside a quoted Mermaid label.
fn mermaid_escape(value: &str) -> String {
    value.replace('"', "#quot;")
}

/// Escapes a string for use inside a double-quoted D2 key.
fn d2_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_quotes() {
        assert_eq!(dot_escape(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(mermaid_escape(r#"a"b"#), "a#quot;b");
    }

    #[test]
    fn primary_style_priority() {
        let style = primary_style(&[NodeFlag::Leaf, NodeFlag::EntryPoint]).unwrap();
        assert_eq!(style.0, NodeFlag::EntryPoint);
        assert!(primary_style(&[]).is_none());
    }
}
