// TypeScript types matching the Rust OutputSchema from src/output/schema.rs (schema v1.1.0)

/** Type of SCSS directive that created a dependency */
export type DirectiveType = "use" | "forward" | "import";

/** Flags assigned to nodes based on analysis */
export type NodeFlag = "entry_point" | "leaf" | "orphan" | "high_fan_in" | "high_fan_out" | "in_cycle";

/** Computed metrics for a file node */
export interface NodeMetrics {
	/** Number of files that depend on this file (in-degree) */
	fan_in: number;
	/** Number of files this file depends on (out-degree) */
	fan_out: number;
	/** Distance from the nearest entry point, or `null` when the file is unreachable from any entry point (orphans) */
	depth: number | null;
	/** Total number of transitive dependencies */
	transitive_deps: number;
	/** Number of files that (transitively) depend on this file, i.e. its "impact" or blast radius */
	transitive_dependents: number;
}

/** Node data in the output schema */
export interface OutputNode {
	/** Absolute file path */
	path: string;
	/** Computed metrics */
	metrics: NodeMetrics;
	/** Assigned flags */
	flags: NodeFlag[];
}

/** Source location of a directive */
export interface EdgeLocation {
	/** Line number (1-indexed) */
	line: number;
	/** Column number (1-indexed) */
	column: number;
}

/** Edge data in the output schema */
export interface OutputEdge {
	/** Source file ID (the file containing the directive) */
	from: string;
	/** Target file ID (the file being loaded) */
	to: string;
	/** Type of directive */
	directive_type: DirectiveType;
	/** Source location of the directive in the `from` file */
	location: EdgeLocation;
	/** Namespace for @use directives */
	namespace?: string;
	/** Whether @use/@forward has a `with(...)` configuration */
	configured: boolean;
}

/** An import directive whose target could not be resolved to a file */
export interface UnresolvedImport {
	/** File ID containing the directive */
	from: string;
	/** The raw target string as written in the directive */
	target: string;
	/** Type of directive */
	directive_type: DirectiveType;
	/** Source location of the directive in the `from` file */
	location: EdgeLocation;
	/** Why resolution failed when it is not simply "not found" (e.g. `ambiguous: a.scss, _a.scss`) */
	reason?: string;
}

/** Summary statistics */
export interface Statistics {
	/** Total number of files analyzed */
	total_files: number;
	/** Total number of dependencies */
	total_dependencies: number;
	/** Number of entry points */
	entry_points: number;
	/** Number of orphan files */
	orphan_files: number;
	/** Number of leaf files */
	leaf_files: number;
	/** Maximum depth in dependency tree */
	max_depth: number;
	/** Maximum fan-in value */
	max_fan_in: number;
	/** Maximum fan-out value */
	max_fan_out: number;
	/** Number of detected dependency cycles */
	cycle_count: number;
	/** Number of import directives that could not be resolved */
	unresolved_imports: number;
	/** Number of legacy `@import` edges (deprecated in Dart Sass, removed in 3.0) */
	import_directives: number;
}

/** Analysis results */
export interface Analysis {
	/**
	 * Detected cycles. Each cycle is an ordered closed path: `cycle[i] -> cycle[i + 1]` is a real
	 * edge and `last -> first` closes it. Self-imports appear as a 1-element cycle.
	 */
	cycles: string[][];
	/** Import directives that could not be resolved */
	unresolved: UnresolvedImport[];
	/** Summary statistics */
	statistics: Statistics;
}

/** Metadata about the analysis run */
export interface Metadata {
	/** ISO 8601 timestamp */
	generated_at: string;
	/** Project root directory */
	root: string;
	/** sass-dep version */
	sass_dep_version: string;
	/** Entry point file IDs (emitted by newer CLI versions; absent in older files) */
	entry_points?: string[];
}

/** Root output schema from sass-dep */
export interface SassDepOutput {
	/** JSON schema reference */
	$schema: string;
	/** Schema version (currently "1.1.0") */
	version: string;
	/** Analysis metadata */
	metadata: Metadata;
	/** Map of file ID to node data */
	nodes: Record<string, OutputNode>;
	/** Array of dependency edges */
	edges: OutputEdge[];
	/** Analysis results */
	analysis: Analysis;
}
