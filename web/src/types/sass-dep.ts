// TypeScript types matching the Rust OutputSchema from src/output/schema.rs

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
	/** Distance from the nearest entry point */
	depth: number;
	/** Total number of transitive dependencies */
	transitive_deps: number;
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
	/** Source file ID */
	from: string;
	/** Target file ID */
	to: string;
	/** Type of directive */
	directive_type: DirectiveType;
	/** Source location in the file */
	location: EdgeLocation;
	/** Namespace for @use directives */
	namespace?: string;
	/** Whether @use has configuration */
	configured?: boolean;
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
}

/** Analysis results */
export interface Analysis {
	/** Detected cycles (arrays of file IDs) */
	cycles: string[][];
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
}

/** Root output schema from sass-dep */
export interface SassDepOutput {
	/** JSON schema reference */
	$schema: string;
	/** Schema version */
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
