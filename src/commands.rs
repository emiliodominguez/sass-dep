//! Command implementations.
//!
//! This module contains the business logic for each CLI command. All
//! commands share the same pipeline: load the project ([`Project::open`]),
//! build and analyze the graph ([`Project::build`]), then report.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::Serialize;
use thiserror::Error;

use crate::analyzer::{Analyzer, AnalyzerConfig};
use crate::cli::{CheckFormat, ExportFormat, GraphArgs, OutputFormat, ReportFormat};
use crate::config::{starter_config, CompiledRule, Config, ConfigError, DEFAULT_CONFIG_FILE};
use crate::graph::{DependencyGraph, DirectiveType, NodeFlag};
use crate::output::{OutputSchema, Serializer};
use crate::parser::ParseError;
use crate::resolver::{Resolver, ResolverConfig};

/// Maximum number of unresolved-import warnings printed without `-v`.
const MAX_UNRESOLVED_WARNINGS: usize = 10;

/// Invalid command-line usage (exit code 2).
#[derive(Debug, Error)]
#[error("{0}")]
pub struct UsageError(pub String);

/// Maps an error to the documented process exit code.
///
/// | Code | Meaning                            |
/// | ---- | ---------------------------------- |
/// | 2    | Invalid arguments or configuration |
/// | 3    | File not found or IO error         |
/// | 4    | Parse error                        |
pub fn exit_code(error: &anyhow::Error) -> i32 {
    for cause in error.chain() {
        if cause.is::<UsageError>() || cause.is::<ConfigError>() {
            return 2;
        }
        if cause.is::<ParseError>() {
            return 4;
        }
        if cause.is::<io::Error>() {
            return 3;
        }
    }
    2
}

/// Options shared by every command.
#[derive(Debug, Clone)]
pub struct GlobalOptions {
    /// Project root directory.
    pub root: PathBuf,
    /// Explicit config file.
    pub config: Option<PathBuf>,
    /// Extra load paths from the command line.
    pub load_paths: Vec<PathBuf>,
    /// Suppress non-error output.
    pub quiet: bool,
    /// Verbosity level.
    pub verbose: u8,
}

/// Stderr logger honouring `--quiet` and `--verbose`.
#[derive(Debug, Clone, Copy)]
struct Log {
    quiet: bool,
    verbose: u8,
}

impl Log {
    /// Prints a warning unless `--quiet`.
    fn warn(&self, message: impl AsRef<str>) {
        if !self.quiet {
            eprintln!("warning: {}", message.as_ref());
        }
    }

    /// Prints a message at verbosity `level` or above.
    fn info(&self, level: u8, message: impl AsRef<str>) {
        if !self.quiet && self.verbose >= level {
            eprintln!("{}", message.as_ref());
        }
    }

    /// Prints a status message unless `--quiet`.
    fn status(&self, message: impl AsRef<str>) {
        if !self.quiet {
            eprintln!("{}", message.as_ref());
        }
    }
}

/// A loaded project: canonical root, configuration and resolver.
pub struct Project {
    root: PathBuf,
    config: Config,
    resolver: Resolver,
    log: Log,
}

impl Project {
    /// Canonicalizes the root, loads the config file and sets up the resolver.
    ///
    /// # Errors
    ///
    /// Fails if the root does not exist or the config file is invalid.
    pub fn open(global: &GlobalOptions) -> Result<Self> {
        let root = dunce::canonicalize(&global.root).with_context(|| {
            format!(
                "Failed to resolve root directory: {}",
                global.root.display()
            )
        })?;
        let log = Log {
            quiet: global.quiet,
            verbose: global.verbose,
        };

        let (config, config_path) = Config::load(&root, global.config.as_deref())?;
        if let Some(path) = &config_path {
            log.info(1, format!("Using config: {}", path.display()));
        }

        // CLI load paths first, then config load paths; all absolute.
        let load_paths = global
            .load_paths
            .iter()
            .chain(&config.load_paths)
            .map(|p| absolutize(&root, p))
            .collect();

        let resolver = Resolver::new(ResolverConfig {
            load_paths,
            extensions: config
                .extensions
                .clone()
                .unwrap_or_else(|| ResolverConfig::default().extensions),
        });

        Ok(Self {
            root,
            config,
            resolver,
            log,
        })
    }

    /// Returns the canonical project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the loaded configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Picks entry points from the CLI, falling back to the config file.
    fn entry_points(&self, cli: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let entries = if cli.is_empty() {
            &self.config.entry_points
        } else {
            cli
        };
        if entries.is_empty() {
            return Err(UsageError(format!(
                "No entry points given. Pass them as arguments or set `entry_points` in {DEFAULT_CONFIG_FILE} (see `sass-dep init`)."
            ))
            .into());
        }
        Ok(entries.iter().map(|e| absolutize(&self.root, e)).collect())
    }

    /// Builds and analyzes the dependency graph.
    ///
    /// # Errors
    ///
    /// Fails if an entry point is missing or a reachable file cannot be read.
    pub fn build(&self, cli_entries: &[PathBuf], include_orphans: bool) -> Result<DependencyGraph> {
        let entries = self.entry_points(cli_entries)?;
        self.log
            .info(1, format!("Analyzing from root: {}", self.root.display()));

        let started = Instant::now();
        let mut graph = DependencyGraph::new();
        graph.build_from_entries(&entries, &self.resolver, &self.root)?;

        if include_orphans || self.config.include_orphans {
            let exclude = self.config.exclude_set()?;
            graph.discover_orphans_excluding(&self.root, &self.resolver, &exclude)?;
        }
        let built = started.elapsed();

        let analyzer = Analyzer::new(AnalyzerConfig {
            thresholds: self.config.thresholds.clone(),
        });
        let analysis_started = Instant::now();
        analyzer.analyze(&mut graph);

        self.log.info(
            1,
            format!(
                "Parsed {} files and {} dependencies in {:.1?}; analysis took {:.1?}",
                graph.node_count(),
                graph.edge_count(),
                built,
                analysis_started.elapsed()
            ),
        );

        self.report_unresolved(&graph);
        Ok(graph)
    }

    /// Prints unresolved-import warnings (capped unless `-v`).
    fn report_unresolved(&self, graph: &DependencyGraph) {
        let unresolved = graph.unresolved();
        let limit = if self.log.verbose > 0 {
            usize::MAX
        } else {
            MAX_UNRESOLVED_WARNINGS
        };

        for item in unresolved.iter().take(limit) {
            self.log.warn(format!(
                "cannot resolve @{} \"{}\" in {}:{}:{}",
                item.directive_type,
                item.target,
                item.from,
                item.location.line,
                item.location.column
            ));
        }
        if unresolved.len() > limit {
            self.log.warn(format!(
                "... and {} more unresolved imports (use -v to list all, -I to add load paths)",
                unresolved.len() - limit
            ));
        }
    }

    /// Maps a user-supplied path or file ID to a node ID in `graph`.
    fn file_id(&self, graph: &DependencyGraph, arg: &Path) -> Option<String> {
        let as_id = arg.to_string_lossy().replace('\\', "/");
        if graph.get_node(&as_id).is_some() {
            return Some(as_id);
        }

        let candidates = [
            absolutize(&self.root, arg),
            std::env::current_dir().ok()?.join(arg),
        ];
        candidates.iter().find_map(|candidate| {
            let canonical = dunce::canonicalize(candidate).ok()?;
            let id = canonical
                .strip_prefix(&self.root)
                .unwrap_or(&canonical)
                .to_string_lossy()
                .replace('\\', "/");
            graph.get_node(&id).map(|_| id)
        })
    }
}

/// Resolves `path` against `root` unless it is absolute.
fn absolutize(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Runs `write` against the output file, or a buffered stdout.
fn with_output(
    output: Option<&Path>,
    write: impl FnOnce(&mut dyn Write) -> Result<()>,
) -> Result<()> {
    match output {
        Some(path) => {
            let file = fs::File::create(path)
                .with_context(|| format!("Failed to create output file: {}", path.display()))?;
            let mut writer = BufWriter::new(file);
            write(&mut writer)?;
            writer.flush()?;
        }
        None => {
            let stdout = io::stdout();
            let mut writer = BufWriter::new(stdout.lock());
            write(&mut writer)?;
            writer.flush()?;
        }
    }
    Ok(())
}

/// Options for the analyze command.
#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    /// Entry points and orphan discovery.
    pub graph: GraphArgs,
    /// Output file (stdout when `None`).
    pub output: Option<PathBuf>,
    /// Output format.
    pub format: OutputFormat,
    /// Start the web visualizer instead of writing output.
    pub web: bool,
    /// Preferred web server port.
    pub port: u16,
    /// Open the browser when the server starts.
    pub open_browser: bool,
}

/// Execute the analyze command.
///
/// Builds a dependency graph from the entry points and outputs
/// analysis results in the specified format, or starts a web server
/// for interactive visualization.
///
/// # Errors
///
/// Fails on invalid configuration, unreadable files or output errors.
pub fn analyze(global: &GlobalOptions, opts: AnalyzeOptions) -> Result<()> {
    let project = Project::open(global)?;
    let graph = project.build(&opts.graph.entry_points, opts.graph.include_orphans)?;
    let schema = OutputSchema::from_graph(&graph, project.root());
    drop(graph);

    if opts.web {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to create async runtime")?;
        return runtime.block_on(crate::web::serve(
            schema,
            crate::web::ServeOptions {
                port: opts.port,
                open_browser: opts.open_browser,
                quiet: global.quiet,
            },
        ));
    }

    with_output(opts.output.as_deref(), |w| match opts.format {
        OutputFormat::Json => Serializer::write_json(&schema, w),
        OutputFormat::Dot => Ok(w.write_all(Serializer::to_dot(&schema).as_bytes())?),
        OutputFormat::Mermaid => Ok(w.write_all(Serializer::to_mermaid(&schema).as_bytes())?),
        OutputFormat::D2 => Ok(w.write_all(Serializer::to_d2(&schema).as_bytes())?),
    })?;

    if let Some(path) = &opts.output {
        project
            .log
            .status(format!("Output written to: {}", path.display()));
    }
    Ok(())
}

/// A constraint violation found by the check command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Violation {
    /// Circular dependency detected.
    Cycle {
        /// Files in import order (the last one imports the first).
        files: Vec<String>,
    },
    /// File exceeds maximum depth.
    MaxDepth {
        /// Offending file.
        file: String,
        /// Actual depth.
        depth: usize,
        /// Allowed maximum.
        max: usize,
    },
    /// File exceeds maximum fan-out.
    MaxFanOut {
        /// Offending file.
        file: String,
        /// Actual fan-out.
        fan_out: usize,
        /// Allowed maximum.
        max: usize,
    },
    /// File exceeds maximum fan-in.
    MaxFanIn {
        /// Offending file.
        file: String,
        /// Actual fan-in.
        fan_in: usize,
        /// Allowed maximum.
        max: usize,
    },
    /// File is unreachable from every entry point.
    Orphan {
        /// Offending file.
        file: String,
    },
    /// Import target could not be resolved.
    Unresolved {
        /// Importing file.
        file: String,
        /// Raw target.
        target: String,
        /// Directive line.
        line: usize,
        /// Directive column.
        column: usize,
    },
    /// Legacy `@import` usage.
    LegacyImport {
        /// Importing file.
        file: String,
        /// Imported file.
        target: String,
        /// Directive line.
        line: usize,
        /// Directive column.
        column: usize,
    },
    /// Forbidden dependency per an architecture rule.
    Rule {
        /// Rule name.
        rule: String,
        /// Importing file.
        file: String,
        /// Forbidden import.
        target: String,
        /// Directive line.
        line: usize,
        /// Directive column.
        column: usize,
        /// Optional explanation from the config.
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}

impl Violation {
    /// Short machine-friendly kind.
    pub fn kind(&self) -> &'static str {
        match self {
            Violation::Cycle { .. } => "cycle",
            Violation::MaxDepth { .. } => "max-depth",
            Violation::MaxFanOut { .. } => "max-fan-out",
            Violation::MaxFanIn { .. } => "max-fan-in",
            Violation::Orphan { .. } => "orphan",
            Violation::Unresolved { .. } => "unresolved",
            Violation::LegacyImport { .. } => "legacy-import",
            Violation::Rule { .. } => "rule",
        }
    }

    /// Human-readable description.
    pub fn message(&self) -> String {
        match self {
            Violation::Cycle { files } => {
                let mut chain = files.clone();
                chain.extend(files.first().cloned());
                format!("circular dependency: {}", chain.join(" -> "))
            }
            Violation::MaxDepth { file, depth, max } => {
                format!("{file} has depth {depth} (max {max})")
            }
            Violation::MaxFanOut { file, fan_out, max } => {
                format!("{file} has fan-out {fan_out} (max {max})")
            }
            Violation::MaxFanIn { file, fan_in, max } => {
                format!("{file} has fan-in {fan_in} (max {max})")
            }
            Violation::Orphan { file } => format!("{file} is not reachable from any entry point"),
            Violation::Unresolved { file, target, .. } => {
                format!("{file} imports \"{target}\" which cannot be resolved")
            }
            Violation::LegacyImport { file, target, .. } => {
                format!("{file} uses @import for {target}; migrate to @use/@forward")
            }
            Violation::Rule {
                rule,
                file,
                target,
                message,
                ..
            } => {
                let mut text = format!("{file} must not depend on {target} [{rule}]");
                if let Some(message) = message {
                    text.push_str(": ");
                    text.push_str(message);
                }
                text
            }
        }
    }

    /// File and optional `(line, column)` the violation points at.
    pub fn location(&self) -> (&str, Option<(usize, usize)>) {
        match self {
            Violation::Cycle { files } => (files.first().map_or("", String::as_str), None),
            Violation::MaxDepth { file, .. }
            | Violation::MaxFanOut { file, .. }
            | Violation::MaxFanIn { file, .. }
            | Violation::Orphan { file } => (file, None),
            Violation::Unresolved {
                file, line, column, ..
            }
            | Violation::LegacyImport {
                file, line, column, ..
            }
            | Violation::Rule {
                file, line, column, ..
            } => (file, Some((*line, *column))),
        }
    }
}

/// Constraints evaluated by [`collect_violations`].
#[derive(Debug, Clone, Default)]
pub struct CheckSettings {
    /// Fail on cycles.
    pub no_cycles: bool,
    /// Maximum depth.
    pub max_depth: Option<usize>,
    /// Maximum fan-out.
    pub max_fan_out: Option<usize>,
    /// Maximum fan-in.
    pub max_fan_in: Option<usize>,
    /// Fail on orphans.
    pub no_orphans: bool,
    /// Fail on unresolved imports.
    pub no_unresolved: bool,
    /// Fail on legacy `@import`.
    pub no_legacy_imports: bool,
}

/// Evaluates all constraints and rules against an analyzed graph.
///
/// Violations are ordered by kind, then by file ID, for stable reports.
pub fn collect_violations(
    graph: &DependencyGraph,
    settings: &CheckSettings,
    rules: &[CompiledRule],
) -> Vec<Violation> {
    let mut violations = Vec::new();

    if settings.no_cycles {
        violations.extend(graph.get_cycles().iter().map(|files| Violation::Cycle {
            files: files.clone(),
        }));
    }

    let mut nodes: Vec<_> = graph.nodes().collect();
    nodes.sort_unstable_by(|a, b| a.0.cmp(b.0));

    if let Some(max) = settings.max_depth {
        for (id, node) in &nodes {
            if let Some(depth) = node.metrics.depth.filter(|d| *d > max) {
                violations.push(Violation::MaxDepth {
                    file: id.to_string(),
                    depth,
                    max,
                });
            }
        }
    }

    if let Some(max) = settings.max_fan_out {
        for (id, node) in &nodes {
            if node.metrics.fan_out > max {
                violations.push(Violation::MaxFanOut {
                    file: id.to_string(),
                    fan_out: node.metrics.fan_out,
                    max,
                });
            }
        }
    }

    if let Some(max) = settings.max_fan_in {
        for (id, node) in &nodes {
            if node.metrics.fan_in > max {
                violations.push(Violation::MaxFanIn {
                    file: id.to_string(),
                    fan_in: node.metrics.fan_in,
                    max,
                });
            }
        }
    }

    if settings.no_orphans {
        for (id, node) in &nodes {
            if node.has_flag(&NodeFlag::Orphan) {
                violations.push(Violation::Orphan {
                    file: id.to_string(),
                });
            }
        }
    }

    if settings.no_unresolved {
        violations.extend(graph.unresolved().iter().map(|u| Violation::Unresolved {
            file: u.from.clone(),
            target: u.target.clone(),
            line: u.location.line,
            column: u.location.column,
        }));
    }

    let mut edges: Vec<_> = graph.edges().collect();
    edges.sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));

    if settings.no_legacy_imports {
        for (from, to, edge) in &edges {
            if edge.directive_type == DirectiveType::Import {
                violations.push(Violation::LegacyImport {
                    file: from.to_string(),
                    target: to.to_string(),
                    line: edge.location.line,
                    column: edge.location.column,
                });
            }
        }
    }

    for rule in rules {
        for (from, to, edge) in &edges {
            if rule.is_violated_by(from, to) {
                violations.push(Violation::Rule {
                    rule: rule.name.clone(),
                    file: from.to_string(),
                    target: to.to_string(),
                    line: edge.location.line,
                    column: edge.location.column,
                    message: rule.message.clone(),
                });
            }
        }
    }

    violations
}

/// Options for the check command.
#[derive(Debug, Clone)]
pub struct CheckOptions {
    /// Entry points and orphan discovery.
    pub graph: GraphArgs,
    /// Constraints from the command line (merged with the config).
    pub settings: CheckSettings,
    /// Report format.
    pub format: CheckFormat,
}

/// Execute the check command.
///
/// Command-line constraints are merged with the `[check]` config section
/// (flags enable checks; numeric limits from the CLI win) and every
/// `[[rules]]` entry is enforced.
///
/// # Returns
///
/// The violations found. Empty if all constraints pass.
///
/// # Errors
///
/// Fails on invalid configuration or unreadable files.
pub fn check(global: &GlobalOptions, opts: CheckOptions) -> Result<Vec<Violation>> {
    let project = Project::open(global)?;
    let config = &project.config().check;
    let cli = &opts.settings;

    let settings = CheckSettings {
        no_cycles: cli.no_cycles || config.no_cycles,
        max_depth: cli.max_depth.or(config.max_depth),
        max_fan_out: cli.max_fan_out.or(config.max_fan_out),
        max_fan_in: cli.max_fan_in.or(config.max_fan_in),
        no_orphans: cli.no_orphans || config.no_orphans,
        no_unresolved: cli.no_unresolved || config.no_unresolved,
        no_legacy_imports: cli.no_legacy_imports || config.no_legacy_imports,
    };
    let rules = project.config().compiled_rules()?;

    let graph = project.build(
        &opts.graph.entry_points,
        opts.graph.include_orphans || settings.no_orphans,
    )?;
    let violations = collect_violations(&graph, &settings, &rules);

    match opts.format {
        CheckFormat::Text => {
            for violation in &violations {
                let (file, position) = violation.location();
                let at = match position {
                    Some((line, column)) => format!(" ({file}:{line}:{column})"),
                    None => String::new(),
                };
                eprintln!("error[{}]: {}{at}", violation.kind(), violation.message());
            }
            if violations.is_empty() {
                project
                    .log
                    .status(format!("All checks passed ({} files).", graph.node_count()));
            } else {
                eprintln!(
                    "\n{} violation(s) found in {} files.",
                    violations.len(),
                    graph.node_count()
                );
            }
        }
        CheckFormat::Json => {
            #[derive(Serialize)]
            struct Report<'a> {
                passed: bool,
                files: usize,
                violations: &'a [Violation],
            }
            with_output(None, |w| {
                serde_json::to_writer_pretty(
                    &mut *w,
                    &Report {
                        passed: violations.is_empty(),
                        files: graph.node_count(),
                        violations: &violations,
                    },
                )?;
                Ok(writeln!(w)?)
            })?;
        }
        CheckFormat::Github => {
            with_output(None, |w| {
                for violation in &violations {
                    let (file, position) = violation.location();
                    let mut props = format!("file={}", github_property(file));
                    if let Some((line, column)) = position {
                        props.push_str(&format!(",line={line},col={column}"));
                    }
                    props.push_str(&format!(
                        ",title={}",
                        github_property(&format!("sass-dep {}", violation.kind()))
                    ));
                    writeln!(w, "::error {props}::{}", github_data(&violation.message()))?;
                }
                Ok(())
            })?;
        }
    }

    Ok(violations)
}

/// Escapes workflow command data.
fn github_data(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Escapes a workflow command property value.
fn github_property(value: &str) -> String {
    github_data(value).replace(':', "%3A").replace(',', "%2C")
}

/// Execute the export command.
///
/// Converts a JSON analysis file (any 1.x schema) to a visualization format.
///
/// # Errors
///
/// Fails if the input cannot be read or is not a sass-dep document.
pub fn export(input: &Path, format: ExportFormat, output: Option<&Path>) -> Result<()> {
    let content = if input == Path::new("-") {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read stdin")?;
        buf
    } else {
        fs::read_to_string(input)
            .with_context(|| format!("Failed to read input file: {}", input.display()))?
    };

    let schema = OutputSchema::from_json(&content).map_err(|e| UsageError(format!("{e:#}")))?;

    let rendered = match format {
        ExportFormat::Dot => Serializer::to_dot(&schema),
        ExportFormat::Mermaid => Serializer::to_mermaid(&schema),
        ExportFormat::D2 => Serializer::to_d2(&schema),
    };

    with_output(output, |w| Ok(w.write_all(rendered.as_bytes())?))
}

/// One step of an import chain.
#[derive(Debug, Clone, Serialize)]
pub struct ChainStep {
    /// File ID.
    pub file: String,
    /// Directive in the previous file that imports this one (absent for the entry point).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directive_type: Option<DirectiveType>,
    /// Line of that directive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

/// Finds the shortest import chain from any entry point to `target`.
pub fn import_chain(graph: &DependencyGraph, target: &str) -> Option<Vec<ChainStep>> {
    let inner = graph.inner();
    let target = graph.index_of(target)?;

    let mut parent: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut seen: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<NodeIndex> = graph
        .entry_points()
        .iter()
        .filter_map(|id| graph.index_of(id))
        .filter(|idx| seen.insert(*idx))
        .collect();

    while let Some(current) = queue.pop_front() {
        if current == target {
            let mut nodes = vec![current];
            let mut node = current;
            while let Some(&prev) = parent.get(&node) {
                nodes.push(prev);
                node = prev;
            }
            nodes.reverse();

            return Some(
                nodes
                    .iter()
                    .enumerate()
                    .map(|(i, &idx)| {
                        let edge = (i > 0)
                            .then(|| inner.find_edge(nodes[i - 1], idx))
                            .flatten()
                            .map(|e| &inner[e]);
                        ChainStep {
                            file: inner[idx].id.clone(),
                            directive_type: edge.map(|e| e.directive_type),
                            line: edge.map(|e| e.location.line),
                        }
                    })
                    .collect(),
            );
        }

        for next in inner.neighbors_directed(current, Direction::Outgoing) {
            if seen.insert(next) {
                parent.insert(next, current);
                queue.push_back(next);
            }
        }
    }

    None
}

/// Execute the why command.
///
/// # Returns
///
/// `true` if the file is reachable from an entry point.
///
/// # Errors
///
/// Fails if the file is not part of the graph.
pub fn why(
    global: &GlobalOptions,
    file: &Path,
    entry_points: &[PathBuf],
    format: ReportFormat,
) -> Result<bool> {
    let project = Project::open(global)?;
    let graph = project.build(entry_points, true)?;
    let id = project.file_id(&graph, file).ok_or_else(|| {
        UsageError(format!(
            "{} is not a Sass file in this project",
            file.display()
        ))
    })?;
    let chain = import_chain(&graph, &id);

    match format {
        ReportFormat::Json => {
            #[derive(Serialize)]
            struct Report<'a> {
                file: &'a str,
                reachable: bool,
                chain: &'a [ChainStep],
            }
            with_output(None, |w| {
                serde_json::to_writer_pretty(
                    &mut *w,
                    &Report {
                        file: &id,
                        reachable: chain.is_some(),
                        chain: chain.as_deref().unwrap_or_default(),
                    },
                )?;
                Ok(writeln!(w)?)
            })?;
        }
        ReportFormat::Text => with_output(None, |w| {
            match &chain {
                None => writeln!(w, "{id} is not reachable from any entry point (orphan).")?,
                Some(steps) => {
                    for (i, step) in steps.iter().enumerate() {
                        match (step.directive_type, step.line) {
                            (Some(directive), Some(line)) => writeln!(
                                w,
                                "{}└─ @{directive} (line {line}) → {}",
                                "   ".repeat(i - 1),
                                step.file
                            )?,
                            _ => writeln!(w, "{}", step.file)?,
                        }
                    }
                }
            }
            Ok(())
        })?,
    }

    Ok(chain.is_some())
}

/// Result of an impact analysis.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImpactReport {
    /// Changed files that are part of the graph.
    pub changed: Vec<String>,
    /// Arguments that are not Sass files of this project.
    pub ignored: Vec<String>,
    /// Files that (transitively) depend on a changed file.
    pub affected: Vec<String>,
    /// Entry points that must be rebuilt.
    pub entry_points: Vec<String>,
}

/// Computes every file transitively depending on `changed`.
pub fn impact_of(graph: &DependencyGraph, changed: &[String]) -> ImpactReport {
    let inner = graph.inner();
    let mut seen: HashSet<NodeIndex> = changed.iter().filter_map(|id| graph.index_of(id)).collect();
    let mut queue: VecDeque<NodeIndex> = seen.iter().copied().collect();
    let mut affected = Vec::new();

    while let Some(current) = queue.pop_front() {
        for edge in inner.edges_directed(current, Direction::Incoming) {
            let dependent = edge.source();
            if seen.insert(dependent) {
                affected.push(inner[dependent].id.clone());
                queue.push_back(dependent);
            }
        }
    }
    affected.sort_unstable();

    let mut entry_points: Vec<String> = seen
        .iter()
        .map(|&idx| &inner[idx])
        .filter(|node| node.has_flag(&NodeFlag::EntryPoint))
        .map(|node| node.id.clone())
        .collect();
    entry_points.sort_unstable();

    ImpactReport {
        changed: changed.to_vec(),
        ignored: Vec::new(),
        affected,
        entry_points,
    }
}

/// Execute the impact command.
///
/// # Errors
///
/// Fails on invalid configuration or unreadable files.
pub fn impact(
    global: &GlobalOptions,
    files: &[PathBuf],
    entry_points: &[PathBuf],
    format: ReportFormat,
) -> Result<ImpactReport> {
    let project = Project::open(global)?;
    let graph = project.build(entry_points, true)?;

    let mut changed = Vec::new();
    let mut ignored = Vec::new();
    for file in files {
        match project.file_id(&graph, file) {
            Some(id) => changed.push(id),
            None => ignored.push(file.to_string_lossy().into_owned()),
        }
    }
    changed.sort_unstable();
    changed.dedup();

    let mut report = impact_of(&graph, &changed);
    report.ignored = ignored;

    with_output(None, |w| {
        match format {
            ReportFormat::Json => {
                serde_json::to_writer_pretty(&mut *w, &report)?;
                writeln!(w)?;
            }
            ReportFormat::Text => {
                writeln!(w, "Changed ({}):", report.changed.len())?;
                for id in &report.changed {
                    writeln!(w, "  {id}")?;
                }
                writeln!(w, "Affected files ({}):", report.affected.len())?;
                for id in &report.affected {
                    writeln!(w, "  {id}")?;
                }
                writeln!(w, "Affected entry points ({}):", report.entry_points.len())?;
                for id in &report.entry_points {
                    writeln!(w, "  {id}")?;
                }
            }
        }
        Ok(())
    })?;

    for file in &report.ignored {
        project
            .log
            .info(1, format!("ignored (not a Sass file in the graph): {file}"));
    }

    Ok(report)
}

/// Execute the init command: writes a starter `.sass-dep.toml`.
///
/// Entry points are detected as non-partial Sass files that no other file imports.
///
/// # Errors
///
/// Fails if the config already exists (without `force`) or cannot be written.
pub fn init(global: &GlobalOptions, force: bool) -> Result<PathBuf> {
    let root = dunce::canonicalize(&global.root).with_context(|| {
        format!(
            "Failed to resolve root directory: {}",
            global.root.display()
        )
    })?;
    let path = root.join(DEFAULT_CONFIG_FILE);
    if path.exists() && !force {
        return Err(UsageError(format!(
            "{} already exists (use --force to overwrite)",
            path.display()
        ))
        .into());
    }

    let resolver = Resolver::new(ResolverConfig {
        load_paths: global
            .load_paths
            .iter()
            .map(|p| absolutize(&root, p))
            .collect(),
        ..ResolverConfig::default()
    });
    let mut graph = DependencyGraph::new();
    graph.discover_orphans(&root, &resolver)?;
    Analyzer::default().analyze(&mut graph);

    let mut entries: Vec<String> = graph
        .nodes()
        .filter(|(id, node)| {
            let is_partial = id
                .rsplit('/')
                .next()
                .is_some_and(|name| name.starts_with('_'));
            !is_partial && node.metrics.fan_in == 0
        })
        .map(|(id, _)| id.clone())
        .collect();
    entries.sort_unstable();

    fs::write(&path, starter_config(&entries))
        .with_context(|| format!("Failed to write {}", path.display()))?;

    if !global.quiet {
        eprintln!(
            "Created {} with {} detected entry point(s).",
            path.display(),
            entries.len()
        );
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes() {
        assert_eq!(exit_code(&UsageError("x".into()).into()), 2);
        assert_eq!(exit_code(&ConfigError("x".into()).into()), 2);
        let io_error: anyhow::Error = io::Error::new(io::ErrorKind::NotFound, "x").into();
        assert_eq!(exit_code(&io_error.context("reading")), 3);
    }

    #[test]
    fn github_escaping() {
        assert_eq!(github_property("a:b,c"), "a%3Ab%2Cc");
        assert_eq!(github_data("50%\nx"), "50%25%0Ax");
    }

    #[test]
    fn cycle_message_closes_loop() {
        let violation = Violation::Cycle {
            files: vec!["a".into(), "b".into()],
        };
        assert_eq!(violation.message(), "circular dependency: a -> b -> a");
    }
}
