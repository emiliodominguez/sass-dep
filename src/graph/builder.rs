//! Dependency graph builder.
//!
//! The graph is built with a **level-synchronous parallel BFS**: every file
//! in the current frontier is read, parsed and resolved concurrently (with
//! `rayon`), then the results are merged into the graph sequentially in
//! frontier order. Each file is therefore parsed exactly once, and the
//! resulting node/edge order is deterministic regardless of thread timing.
//!
//! Files are identified by their **lexical** absolute path (like Dart Sass),
//! so a file reached through a symlink resolves its own relative imports
//! from the link's location. Within one file, repeated `(target, kind)`
//! pairs are resolved once, and edges are deduplicated in O(1).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::GlobSet;
use indexmap::{IndexMap, IndexSet};
use petgraph::graph::{DiGraph, EdgeIndex};
use rayon::prelude::*;

use super::node::{DependencyEdge, DirectiveType, EdgeMeta, FileNode, NodeFlag, UnresolvedImport};
use super::NodeId;
use crate::parser::{Directive, Location, Namespace, ParseError, Parser, Syntax};
use crate::resolver::{normalize_lexically, ResolveError, ResolveKind, Resolver};

/// A dependency graph representing SCSS file relationships.
///
/// The graph uses `petgraph::DiGraph` for efficient graph operations
/// and `IndexMap` for deterministic node ordering.
pub struct DependencyGraph {
    /// The underlying directed graph.
    graph: DiGraph<FileNode, DependencyEdge>,
    /// Map from file ID to node index.
    node_index: IndexMap<String, NodeId>,
    /// Entry point file IDs, in the order they were added.
    entry_points: IndexSet<String>,
    /// Per-node marker: the file has been scheduled for parsing.
    scheduled: Vec<bool>,
    /// Detected cycles (populated after analysis).
    cycles: Vec<Vec<String>>,
    /// Directives whose targets could not be resolved.
    unresolved: Vec<UnresolvedImport>,
    /// Edge lookup by endpoints, for O(1) deduplication.
    edge_lookup: HashMap<(NodeId, NodeId), EdgeIndex>,
}

/// A dependency discovered while scanning a file.
struct ScannedDependency {
    target: String,
    /// The resolved file, or why it could not be resolved (`None`: not found).
    resolved: Result<PathBuf, Option<Vec<PathBuf>>>,
    directive_type: DirectiveType,
    location: Location,
    meta: EdgeMeta,
}

impl DependencyGraph {
    /// Creates a new empty dependency graph.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_index: IndexMap::new(),
            entry_points: IndexSet::new(),
            scheduled: Vec::new(),
            cycles: Vec::new(),
            unresolved: Vec::new(),
            edge_lookup: HashMap::new(),
        }
    }

    /// Builds the dependency graph starting from an entry point file.
    ///
    /// This method discovers all transitive dependencies and adds them
    /// to the graph. It can be called repeatedly for several entry points;
    /// files shared between entry points are only parsed once.
    ///
    /// # Arguments
    ///
    /// * `entry` - Path to the entry point SCSS file
    /// * `resolver` - Resolver for import paths
    /// * `root` - Project root directory for computing relative paths
    ///
    /// # Errors
    ///
    /// Returns an error if the entry (or any reachable file) cannot be read.
    /// Unresolvable imports are not errors; they are recorded and exposed
    /// through [`DependencyGraph::unresolved`].
    pub fn build_from_entry(
        &mut self,
        entry: &Path,
        resolver: &Resolver,
        root: &Path,
    ) -> Result<NodeId> {
        let idx = self.add_entry(entry, root)?;
        self.expand(vec![idx], resolver, root)?;
        Ok(idx)
    }

    /// Builds the graph from several entry points at once.
    ///
    /// Equivalent to calling [`DependencyGraph::build_from_entry`] for each
    /// entry, but parses all entries' dependency trees in a single parallel
    /// traversal.
    ///
    /// # Errors
    ///
    /// Returns an error if any entry (or reachable file) cannot be read.
    pub fn build_from_entries(
        &mut self,
        entries: &[PathBuf],
        resolver: &Resolver,
        root: &Path,
    ) -> Result<()> {
        let frontier = entries
            .iter()
            .map(|entry| self.add_entry(entry, root))
            .collect::<Result<Vec<_>>>()?;
        self.expand(frontier, resolver, root)
    }

    /// Registers an entry point node and flags it.
    fn add_entry(&mut self, entry: &Path, root: &Path) -> Result<NodeId> {
        let metadata = std::fs::metadata(entry)
            .with_context(|| format!("Failed to resolve entry point: {}", entry.display()))?;
        if !metadata.is_file() {
            anyhow::bail!("Entry point is not a file: {}", entry.display());
        }

        let absolute = if entry.is_absolute() {
            entry.to_path_buf()
        } else {
            std::env::current_dir()?.join(entry)
        };
        let mut entry = normalize_lexically(&absolute);
        // An absolute path spelled through a symlinked prefix (e.g. macOS
        // `/tmp` -> `/private/tmp`) would otherwise get an absolute ID.
        if !entry.starts_with(dunce::simplified(root)) {
            if let Ok(canonical) = dunce::canonicalize(&entry) {
                if canonical.starts_with(dunce::simplified(root)) {
                    entry = canonical;
                }
            }
        }

        let idx = self.add_file(entry, root);
        let id = self.graph[idx].id.clone();
        self.graph[idx].add_flag(NodeFlag::EntryPoint);
        self.entry_points.insert(id);
        Ok(idx)
    }

    /// Parses every unscheduled file reachable from `frontier`.
    fn expand(&mut self, frontier: Vec<NodeId>, resolver: &Resolver, root: &Path) -> Result<()> {
        let mut frontier: Vec<NodeId> = frontier
            .into_iter()
            .filter(|idx| !std::mem::replace(&mut self.scheduled[idx.index()], true))
            .collect();

        while !frontier.is_empty() {
            let scans: Vec<Result<Vec<ScannedDependency>>> = frontier
                .par_iter()
                .map(|&idx| scan_file(&self.graph[idx].absolute_path, resolver))
                .collect();

            let mut next = Vec::new();

            for (&from_idx, scan) in frontier.iter().zip(scans) {
                for dependency in scan? {
                    let resolved = match dependency.resolved {
                        Ok(resolved) => resolved,
                        Err(ambiguous) => {
                            let reason = ambiguous.map(|candidates| {
                                let names: Vec<String> =
                                    candidates.iter().map(|c| file_id(c, root)).collect();
                                format!("ambiguous: {}", names.join(", "))
                            });
                            self.unresolved.push(UnresolvedImport {
                                from: self.graph[from_idx].id.clone(),
                                target: dependency.target,
                                directive_type: dependency.directive_type,
                                location: dependency.location,
                                reason,
                            });
                            continue;
                        }
                    };

                    let to_idx = self.add_file(resolved, root);
                    let edge = DependencyEdge::with_meta(
                        dependency.directive_type,
                        dependency.location,
                        dependency.meta,
                    );
                    match self.edge_lookup.get(&(from_idx, to_idx)) {
                        None => {
                            let index = self.graph.add_edge(from_idx, to_idx, edge);
                            self.edge_lookup.insert((from_idx, to_idx), index);
                        }
                        // One edge per pair; a legacy @import always wins so
                        // `@use "x"; @import "x";` cannot hide it from checks.
                        Some(&index)
                            if edge.directive_type == DirectiveType::Import
                                && self.graph[index].directive_type != DirectiveType::Import =>
                        {
                            self.graph[index] = edge;
                        }
                        Some(_) => {}
                    }

                    if !std::mem::replace(&mut self.scheduled[to_idx.index()], true) {
                        next.push(to_idx);
                    }
                }
            }

            frontier = next;
        }

        Ok(())
    }

    /// Adds a file to the graph if not already present and returns its index.
    fn add_file(&mut self, path: PathBuf, root: &Path) -> NodeId {
        let id = file_id(&path, root);

        if let Some(&idx) = self.node_index.get(&id) {
            return idx;
        }

        let idx = self.graph.add_node(FileNode::new(id.clone(), path));
        self.node_index.insert(id, idx);
        self.scheduled.push(false);
        idx
    }

    /// Discovers orphan files in the project root.
    ///
    /// Orphan files are Sass files that are not reachable from any entry
    /// point. Discovered orphans are parsed as well, so their own
    /// dependencies appear in the graph. The walk honours `.gitignore`,
    /// `.ignore` and hidden-file conventions and always skips
    /// `node_modules`.
    ///
    /// # Errors
    ///
    /// Returns an error if a discovered file cannot be read.
    pub fn discover_orphans(&mut self, root: &Path, resolver: &Resolver) -> Result<()> {
        self.discover_orphans_excluding(root, resolver, &GlobSet::empty())
    }

    /// Like [`DependencyGraph::discover_orphans`], skipping files whose
    /// root-relative ID matches `exclude`.
    ///
    /// # Errors
    ///
    /// Returns an error if a discovered file cannot be read.
    pub fn discover_orphans_excluding(
        &mut self,
        root: &Path,
        resolver: &Resolver,
        exclude: &GlobSet,
    ) -> Result<()> {
        let extensions = resolver.extensions();
        let mut found: Vec<PathBuf> = ignore::WalkBuilder::new(root)
            .require_git(false)
            .filter_entry(|entry| entry.file_name() != "node_modules")
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_some_and(|t| t.is_file()))
            .map(|entry| entry.into_path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| extensions.iter().any(|e| e == ext))
            })
            .collect();
        found.sort();

        let mut frontier = Vec::new();
        for path in found {
            let path = normalize_lexically(&path);
            let id = file_id(&path, root);
            if self.node_index.contains_key(&id) || exclude.is_match(&id) {
                continue;
            }
            let idx = self.add_file(path, root);
            self.graph[idx].add_flag(NodeFlag::Orphan);
            frontier.push(idx);
        }

        self.expand(frontier, resolver, root)
    }

    /// Returns the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Returns an iterator over all node IDs and their data.
    pub fn nodes(&self) -> impl Iterator<Item = (&String, &FileNode)> {
        self.node_index
            .iter()
            .map(|(id, idx)| (id, &self.graph[*idx]))
    }

    /// Returns a mutable reference to a node by ID.
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut FileNode> {
        self.node_index.get(id).map(|idx| &mut self.graph[*idx])
    }

    /// Returns a reference to a node by ID.
    pub fn get_node(&self, id: &str) -> Option<&FileNode> {
        self.node_index.get(id).map(|idx| &self.graph[*idx])
    }

    /// Returns the node index for a file ID.
    pub fn index_of(&self, id: &str) -> Option<NodeId> {
        self.node_index.get(id).copied()
    }

    /// Returns the entry point file IDs, in insertion order.
    pub fn entry_points(&self) -> &IndexSet<String> {
        &self.entry_points
    }

    /// Returns a reference to the underlying petgraph.
    pub fn inner(&self) -> &DiGraph<FileNode, DependencyEdge> {
        &self.graph
    }

    /// Returns a mutable reference to the underlying petgraph.
    pub fn inner_mut(&mut self) -> &mut DiGraph<FileNode, DependencyEdge> {
        &mut self.graph
    }

    /// Returns the node index map.
    pub fn node_index(&self) -> &IndexMap<String, NodeId> {
        &self.node_index
    }

    /// Sets the detected cycles.
    pub fn set_cycles(&mut self, cycles: Vec<Vec<String>>) {
        self.cycles = cycles;
    }

    /// Returns the detected cycles.
    pub fn get_cycles(&self) -> &[Vec<String>] {
        &self.cycles
    }

    /// Returns the directives whose targets could not be resolved.
    pub fn unresolved(&self) -> &[UnresolvedImport] {
        &self.unresolved
    }

    /// Returns all edges as `(from_id, to_id, edge)` tuples.
    pub fn edges(&self) -> impl Iterator<Item = (&str, &str, &DependencyEdge)> {
        self.graph.edge_indices().map(move |idx| {
            let (from, to) = self.graph.edge_endpoints(idx).expect("edge index is valid");
            (
                self.graph[from].id.as_str(),
                self.graph[to].id.as_str(),
                &self.graph[idx],
            )
        })
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Computes the file ID (root-relative path with `/` separators).
///
/// Files outside `root` (e.g. in an external load path) keep their absolute
/// path. IDs are injective: on Unix a literal backslash is written as `\\`
/// and bytes that are not valid UTF-8 as `\xHH`, so distinct files can never
/// share an ID.
pub(crate) fn file_id(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(dunce::simplified(root)).unwrap_or(path);

    #[cfg(windows)]
    {
        relative.to_string_lossy().replace('\\', "/")
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::ffi::OsStrExt;

        let mut id = String::new();
        for chunk in relative.as_os_str().as_bytes().utf8_chunks() {
            for ch in chunk.valid().chars() {
                if ch == '\\' {
                    id.push_str("\\\\");
                } else {
                    id.push(ch);
                }
            }
            for byte in chunk.invalid() {
                id.push_str(&format!("\\x{byte:02X}"));
            }
        }
        id
    }
}

/// Reads, parses and resolves the dependencies of one file.
///
/// This is the unit of parallel work in [`DependencyGraph::expand`].
/// Directives are streamed from the scanner and repeated `(target, kind)`
/// pairs are resolved once, so memory is bounded by the number of distinct
/// targets rather than the number of directives. Plain `.css` files have
/// no Sass dependencies.
fn scan_file(path: &Path, resolver: &Resolver) -> Result<Vec<ScannedDependency>> {
    let syntax = Syntax::from_path(path);
    if syntax == Syntax::Css {
        return Ok(Vec::new());
    }

    let content = std::fs::read(path)
        .map_err(ParseError::Io)
        .with_context(|| format!("Failed to read: {}", path.display()))?;
    let base_dir = path.parent().unwrap_or(path);

    let mut seen: HashSet<(String, ResolveKind)> = HashSet::new();
    let mut dependencies = Vec::new();

    Parser::scan(&content, syntax, |directive| {
        let (directive_type, kind, meta) = match &directive {
            Directive::Use(u) => (
                DirectiveType::Use,
                ResolveKind::Module,
                EdgeMeta {
                    namespace: match &u.namespace {
                        Some(Namespace::Named(n)) => Some(n.clone()),
                        Some(Namespace::Star) => Some("*".to_string()),
                        Some(Namespace::Default) | None => None,
                    },
                    configured: u.configured,
                },
            ),
            Directive::Forward(_) => (
                DirectiveType::Forward,
                ResolveKind::Module,
                EdgeMeta::default(),
            ),
            Directive::Import(_) => (
                DirectiveType::Import,
                ResolveKind::Import,
                EdgeMeta::default(),
            ),
        };

        for target in directive.paths() {
            if Resolver::is_external(target) || !seen.insert((target.to_string(), kind)) {
                continue;
            }
            let resolved = match resolver.resolve_from_dir(base_dir, target, kind) {
                Ok(path) => Ok(path),
                Err(ResolveError::Ambiguous { candidates, .. }) => Err(Some(candidates)),
                Err(_) => Err(None),
            };
            dependencies.push(ScannedDependency {
                target: target.to_string(),
                resolved,
                directive_type,
                location: *directive.location(),
                meta: meta.clone(),
            });
        }
    });

    Ok(dependencies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_simple_project(dir: &Path) {
        fs::write(
            dir.join("main.scss"),
            r#"@use "variables" as vars;
@use "mixins";
@use "sass:math";
"#,
        )
        .unwrap();

        fs::write(dir.join("_variables.scss"), "$primary: blue;\n").unwrap();

        fs::write(
            dir.join("_mixins.scss"),
            r#"@use "variables" as vars;
@mixin test { color: vars.$primary; }
"#,
        )
        .unwrap();
    }

    #[test]
    fn build_simple_graph() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        setup_simple_project(&root);

        let resolver = Resolver::default();
        let mut graph = DependencyGraph::new();

        graph
            .build_from_entry(&root.join("main.scss"), &resolver, &root)
            .unwrap();

        assert_eq!(graph.node_count(), 3);
        // main -> variables, main -> mixins, mixins -> variables
        assert_eq!(graph.edge_count(), 3);
        // sass:math is a built-in module, not an unresolved import
        assert!(graph.unresolved().is_empty());
    }

    #[test]
    fn entry_point_flagged() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        setup_simple_project(&root);

        let resolver = Resolver::default();
        let mut graph = DependencyGraph::new();

        graph
            .build_from_entry(&root.join("main.scss"), &resolver, &root)
            .unwrap();

        assert!(graph
            .get_node("main.scss")
            .unwrap()
            .has_flag(&NodeFlag::EntryPoint));
        assert!(!graph
            .get_node("_variables.scss")
            .unwrap()
            .has_flag(&NodeFlag::EntryPoint));
    }

    #[test]
    fn relative_ids() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("src/components")).unwrap();
        fs::write(root.join("src/main.scss"), r#"@use "components/button";"#).unwrap();
        fs::write(root.join("src/components/_button.scss"), "").unwrap();

        let resolver = Resolver::default();
        let mut graph = DependencyGraph::new();

        graph
            .build_from_entry(&root.join("src/main.scss"), &resolver, &root)
            .unwrap();

        assert!(graph.get_node("src/main.scss").is_some());
        assert!(graph.get_node("src/components/_button.scss").is_some());
    }

    #[test]
    fn unresolved_imports_are_recorded() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        fs::write(root.join("main.scss"), "\n@use \"missing\";\n").unwrap();

        let resolver = Resolver::default();
        let mut graph = DependencyGraph::new();
        graph
            .build_from_entry(&root.join("main.scss"), &resolver, &root)
            .unwrap();

        let unresolved = graph.unresolved();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].from, "main.scss");
        assert_eq!(unresolved[0].target, "missing");
        assert_eq!(unresolved[0].location, Location::new(2, 1));
    }

    #[test]
    fn non_entry_cycle_terminates() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        fs::write(root.join("main.scss"), "@use \"a\";\n").unwrap();
        fs::write(root.join("_a.scss"), "@use \"b\";\n").unwrap();
        fs::write(root.join("_b.scss"), "@use \"a\";\n").unwrap();

        let resolver = Resolver::default();
        let mut graph = DependencyGraph::new();
        graph
            .build_from_entry(&root.join("main.scss"), &resolver, &root)
            .unwrap();

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 3);
    }

    #[test]
    fn orphans_are_parsed_and_skip_node_modules() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        setup_simple_project(&root);
        fs::write(root.join("_orphan.scss"), "@use \"helper\";\n").unwrap();
        fs::write(root.join("_helper.scss"), "").unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/_x.scss"), "").unwrap();

        let resolver = Resolver::default();
        let mut graph = DependencyGraph::new();
        graph
            .build_from_entry(&root.join("main.scss"), &resolver, &root)
            .unwrap();
        graph.discover_orphans(&root, &resolver).unwrap();

        assert!(graph
            .get_node("_orphan.scss")
            .unwrap()
            .has_flag(&NodeFlag::Orphan));
        assert!(graph.get_node("_helper.scss").is_some());
        assert!(graph.get_node("node_modules/pkg/_x.scss").is_none());
        assert_eq!(graph.edge_count(), 4);
    }

    /// Builds and returns the graph for `main.scss` under `root`.
    fn build(root: &Path) -> DependencyGraph {
        let mut graph = DependencyGraph::new();
        graph
            .build_from_entry(&root.join("main.scss"), &Resolver::default(), root)
            .unwrap();
        graph
    }

    #[test]
    fn legacy_import_is_not_hidden_by_use_of_same_target() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        fs::write(
            root.join("main.scss"),
            "@use \"theme\";\n@import \"theme\";\n",
        )
        .unwrap();
        fs::write(root.join("_theme.scss"), "").unwrap();

        let graph = build(&root);
        let edges: Vec<_> = graph.edges().collect();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].2.directive_type, DirectiveType::Import);
        assert_eq!(edges[0].2.location, Location::new(2, 1));
    }

    #[test]
    fn ambiguous_targets_are_unresolved_with_a_reason() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        fs::write(root.join("main.scss"), "@use \"a\";\n").unwrap();
        fs::write(root.join("a.scss"), "").unwrap();
        fs::write(root.join("_a.scss"), "").unwrap();

        let graph = build(&root);
        assert_eq!(graph.node_count(), 1);
        let unresolved = graph.unresolved();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(
            unresolved[0].reason.as_deref(),
            Some("ambiguous: _a.scss, a.scss")
        );
    }

    #[test]
    fn css_files_are_not_scanned() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        fs::write(root.join("main.scss"), "@use \"a\";\n").unwrap();
        fs::write(root.join("a.css"), "@import \"b\";\n").unwrap();
        fs::write(root.join("_b.scss"), "").unwrap();

        let graph = build(&root);
        assert!(graph.get_node("a.css").is_some());
        assert!(graph.get_node("_b.scss").is_none());
        assert!(graph.unresolved().is_empty());
    }

    #[test]
    fn duplicate_targets_in_one_file_resolve_once() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        let mut main = "@import \"a\", \"a\", \"missing\";\n".repeat(1000);
        main.push_str("@use \"a\";\n");
        fs::write(root.join("main.scss"), main).unwrap();
        fs::write(root.join("_a.scss"), "").unwrap();

        let graph = build(&root);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.unresolved().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_files_resolve_imports_from_the_link_location() {
        let temp = TempDir::new().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(root.join("real/sub")).unwrap();
        std::os::unix::fs::symlink(root.join("real/sub"), root.join("link")).unwrap();
        fs::write(root.join("main.scss"), "@use \"link/a\";\n").unwrap();
        fs::write(root.join("real/sub/_a.scss"), "@use \"../b\";\n").unwrap();
        fs::write(root.join("_b.scss"), "").unwrap();
        fs::write(root.join("real/_b.scss"), "").unwrap();

        // Dart Sass resolves URLs lexically: link/../b is <root>/b.
        let graph = build(&root);
        assert!(graph.get_node("link/_a.scss").is_some());
        assert!(graph.get_node("_b.scss").is_some());
        assert!(graph.get_node("real/_b.scss").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn file_ids_are_injective() {
        let root = Path::new("/p");
        assert_eq!(file_id(Path::new("/p/a/_b.scss"), root), "a/_b.scss");
        assert_eq!(file_id(Path::new("/p/a\\_b.scss"), root), "a\\\\_b.scss");

        use std::os::unix::ffi::OsStrExt;
        let ff = file_id(
            Path::new(std::ffi::OsStr::from_bytes(b"/p/_x\xff.scss")),
            root,
        );
        let fe = file_id(
            Path::new(std::ffi::OsStr::from_bytes(b"/p/_x\xfe.scss")),
            root,
        );
        assert_eq!(ff, "_x\\xFF.scss");
        assert_ne!(ff, fe);
    }
}
