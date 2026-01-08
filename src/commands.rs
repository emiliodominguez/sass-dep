//! Command implementations.
//!
//! This module contains the business logic for each CLI command.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::analyzer::Analyzer;
use crate::cli::{ExportFormat, OutputFormat};
use crate::graph::DependencyGraph;
use crate::output::{OutputSchema, Serializer};
use crate::resolver::{Resolver, ResolverConfig};

/// Violation found during check command.
#[derive(Debug, Clone)]
pub enum Violation {
    /// Circular dependency detected.
    Cycle { files: Vec<String> },
    /// File exceeds maximum depth.
    MaxDepth { file: String, depth: usize, max: usize },
    /// File exceeds maximum fan-out.
    MaxFanOut { file: String, fan_out: usize, max: usize },
    /// File exceeds maximum fan-in.
    MaxFanIn { file: String, fan_in: usize, max: usize },
}

/// Options for the analyze command.
#[derive(Debug)]
pub struct AnalyzeOptions<'a> {
    pub root: &'a Path,
    pub load_paths: &'a [PathBuf],
    pub entry_points: &'a [PathBuf],
    pub output: Option<&'a Path>,
    pub format: OutputFormat,
    pub include_orphans: bool,
    pub quiet: bool,
    pub verbose: u8,
    pub web: bool,
    pub port: u16,
}

/// Execute the analyze command.
///
/// Builds a dependency graph from the entry points and outputs
/// analysis results in the specified format, or starts a web server
/// for interactive visualization.
pub fn analyze(opts: AnalyzeOptions) -> Result<()> {
    let root = opts.root.canonicalize().context("Failed to resolve root directory")?;

    if opts.verbose > 0 && !opts.quiet {
        eprintln!("Analyzing from root: {}", root.display());
    }

    // Set up resolver
    let config = ResolverConfig {
        load_paths: opts.load_paths.to_vec(),
        extensions: vec!["scss".to_string(), "sass".to_string()],
    };
    let resolver = Resolver::new(config);

    // Build graph
    let mut graph = DependencyGraph::new();
    for entry in opts.entry_points {
        let entry_path = if entry.is_absolute() {
            entry.clone()
        } else {
            root.join(entry)
        };
        let entry_path = entry_path
            .canonicalize()
            .with_context(|| format!("Failed to resolve entry point: {}", entry.display()))?;

        if opts.verbose > 1 && !opts.quiet {
            eprintln!("Processing entry point: {}", entry_path.display());
        }

        graph
            .build_from_entry(&entry_path, &resolver, &root)
            .with_context(|| format!("Failed to build graph from: {}", entry_path.display()))?;
    }

    // Include orphans if requested
    if opts.include_orphans {
        graph.discover_orphans(&root, &resolver)?;
    }

    // Run analysis
    let analyzer = Analyzer::default();
    analyzer.analyze(&mut graph);

    // Generate output schema
    let schema = OutputSchema::from_graph(&graph, &root);

    // Either start web server or output to file/stdout
    if opts.web {
        // Start web visualization server
        let rt = tokio::runtime::Runtime::new()
            .context("Failed to create async runtime")?;
        rt.block_on(crate::web::serve(schema, opts.port))?;
    } else {
        // Generate output
        let output_content = match opts.format {
            OutputFormat::Json => Serializer::to_json(&schema)?,
        };

        // Write output
        match opts.output {
            Some(path) => {
                fs::write(path, &output_content)
                    .with_context(|| format!("Failed to write output to: {}", path.display()))?;
                if !opts.quiet {
                    eprintln!("Output written to: {}", path.display());
                }
            }
            None => {
                io::stdout().write_all(output_content.as_bytes())?;
            }
        }
    }

    Ok(())
}

/// Execute the check command.
///
/// Analyzes the dependency graph and returns any constraint violations.
///
/// # Arguments
///
/// * `root` - Project root directory
/// * `load_paths` - Additional Sass load paths
/// * `entry_points` - Entry point SCSS files
/// * `no_cycles` - Fail if cycles are detected
/// * `max_depth` - Maximum allowed depth
/// * `max_fan_out` - Maximum allowed fan-out
/// * `max_fan_in` - Maximum allowed fan-in
/// * `quiet` - Suppress non-error output
/// * `verbose` - Verbosity level
///
/// # Returns
///
/// A vector of violations found. Empty if all constraints pass.
#[allow(clippy::too_many_arguments)]
pub fn check(
    root: &Path,
    load_paths: &[PathBuf],
    entry_points: &[PathBuf],
    no_cycles: bool,
    max_depth: Option<usize>,
    max_fan_out: Option<usize>,
    max_fan_in: Option<usize>,
    quiet: bool,
    verbose: u8,
) -> Result<Vec<Violation>> {
    let root = root.canonicalize().context("Failed to resolve root directory")?;

    if verbose > 0 && !quiet {
        eprintln!("Checking from root: {}", root.display());
    }

    // Set up resolver
    let config = ResolverConfig {
        load_paths: load_paths.to_vec(),
        extensions: vec!["scss".to_string(), "sass".to_string()],
    };
    let resolver = Resolver::new(config);

    // Build graph
    let mut graph = DependencyGraph::new();
    for entry in entry_points {
        let entry_path = if entry.is_absolute() {
            entry.clone()
        } else {
            root.join(entry)
        };
        let entry_path = entry_path
            .canonicalize()
            .with_context(|| format!("Failed to resolve entry point: {}", entry.display()))?;

        graph
            .build_from_entry(&entry_path, &resolver, &root)
            .with_context(|| format!("Failed to build graph from: {}", entry_path.display()))?;
    }

    // Run analysis
    let analyzer = Analyzer::default();
    analyzer.analyze(&mut graph);

    let mut violations = Vec::new();

    // Check for cycles
    if no_cycles {
        let cycles = graph.get_cycles();
        for cycle in cycles {
            if !quiet {
                eprintln!(
                    "Cycle detected: {}",
                    cycle
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                );
            }
            violations.push(Violation::Cycle { files: cycle.clone() });
        }
    }

    // Check depth constraints
    if let Some(max) = max_depth {
        for (id, node) in graph.nodes() {
            if node.metrics.depth > max {
                if !quiet {
                    eprintln!(
                        "Depth violation: {} has depth {} (max: {})",
                        id, node.metrics.depth, max
                    );
                }
                violations.push(Violation::MaxDepth {
                    file: id.clone(),
                    depth: node.metrics.depth,
                    max,
                });
            }
        }
    }

    // Check fan-out constraints
    if let Some(max) = max_fan_out {
        for (id, node) in graph.nodes() {
            if node.metrics.fan_out > max {
                if !quiet {
                    eprintln!(
                        "Fan-out violation: {} has fan-out {} (max: {})",
                        id, node.metrics.fan_out, max
                    );
                }
                violations.push(Violation::MaxFanOut {
                    file: id.clone(),
                    fan_out: node.metrics.fan_out,
                    max,
                });
            }
        }
    }

    // Check fan-in constraints
    if let Some(max) = max_fan_in {
        for (id, node) in graph.nodes() {
            if node.metrics.fan_in > max {
                if !quiet {
                    eprintln!(
                        "Fan-in violation: {} has fan-in {} (max: {})",
                        id, node.metrics.fan_in, max
                    );
                }
                violations.push(Violation::MaxFanIn {
                    file: id.clone(),
                    fan_in: node.metrics.fan_in,
                    max,
                });
            }
        }
    }

    if violations.is_empty() && !quiet {
        eprintln!("All checks passed.");
    }

    Ok(violations)
}

/// Execute the export command.
///
/// Converts a JSON analysis file to a visualization format.
///
/// # Arguments
///
/// * `input` - Path to the input JSON file
/// * `format` - Export format
pub fn export(input: &Path, format: ExportFormat) -> Result<()> {
    let content = fs::read_to_string(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?;

    let schema: OutputSchema =
        serde_json::from_str(&content).context("Failed to parse input JSON")?;

    let output = match format {
        ExportFormat::Dot => Serializer::to_dot(&schema),
        ExportFormat::Mermaid => Serializer::to_mermaid(&schema),
        ExportFormat::D2 => Serializer::to_d2(&schema),
    };

    print!("{}", output);
    Ok(())
}
