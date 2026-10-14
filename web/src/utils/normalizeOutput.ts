import type {
	Analysis,
	DirectiveType,
	EdgeLocation,
	Metadata,
	NodeFlag,
	NodeMetrics,
	OutputEdge,
	OutputNode,
	SassDepOutput,
	Statistics,
	UnresolvedImport,
} from "../types/sass-dep";

/** Schema version this visualizer was written against. */
export const SUPPORTED_SCHEMA_VERSION = "1.1.0";

/** Error thrown when a JSON document is not valid sass-dep output. */
export class SassDepFormatError extends Error {
	/**
	 * Creates a new format error.
	 * @param message - Human-readable description of what is wrong with the input
	 */
	constructor(message: string) {
		super(message);
		this.name = "SassDepFormatError";
	}
}

/** Generic JSON object shape used while validating untrusted input. */
type JsonObject = Record<string, unknown>;

/** Valid directive type strings. */
const DIRECTIVE_TYPES: ReadonlySet<string> = new Set<DirectiveType>(["use", "forward", "import"]);
/** Valid node flag strings (unknown flags are dropped). */
const NODE_FLAGS: ReadonlySet<string> = new Set<NodeFlag>(["entry_point", "leaf", "orphan", "high_fan_in", "high_fan_out", "in_cycle"]);

/**
 * Checks whether a value is a plain (non-array) object.
 * @param value - Value to check
 * @returns True when the value is a non-null, non-array object
 */
function isObject(value: unknown): value is JsonObject {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Reads a non-negative count, falling back when the value is missing or not a finite number.
 * @param value - Raw value
 * @param fallback - Value used when `value` is not a usable number
 * @returns A finite number
 */
function toCount(value: unknown, fallback = 0): number {
	return typeof value === "number" && Number.isFinite(value) && value < Number.MAX_SAFE_INTEGER ? value : fallback;
}

/**
 * Normalizes a depth value. Older (v1.0.0) output encoded "unreachable" as `usize::MAX`,
 * which JSON.parse turns into a number above `Number.MAX_SAFE_INTEGER`; v1.1.0 uses `null`.
 * @param value - Raw depth value
 * @returns The depth, or null when the file is unreachable from any entry point
 */
function toDepth(value: unknown): number | null {
	if (typeof value !== "number" || !Number.isFinite(value) || value >= Number.MAX_SAFE_INTEGER) {
		return null;
	}
	return value;
}

/**
 * Normalizes a source location.
 * @param value - Raw location object
 * @returns Location with numeric line and column (0 when unknown)
 */
function toLocation(value: unknown): EdgeLocation {
	if (!isObject(value)) return { line: 0, column: 0 };
	return { line: toCount(value["line"]), column: toCount(value["column"]) };
}

/**
 * Validates a directive type string.
 * @param value - Raw directive type
 * @param where - Description of the field location, used in error messages
 * @returns The validated directive type
 * @throws SassDepFormatError when the value is not a known directive type
 */
function toDirectiveType(value: unknown, where: string): DirectiveType {
	if (typeof value !== "string" || !DIRECTIVE_TYPES.has(value)) {
		throw new SassDepFormatError(`${where}.directive_type must be one of "use", "forward" or "import" (got ${JSON.stringify(value)}).`);
	}
	return value as DirectiveType;
}

/**
 * Normalizes a single node entry.
 * @param id - Node ID (key in the `nodes` map)
 * @param raw - Raw node value
 * @returns The normalized node
 * @throws SassDepFormatError when the node is not an object
 */
function normalizeNode(id: string, raw: unknown): OutputNode {
	if (!isObject(raw)) {
		throw new SassDepFormatError(`nodes["${id}"] must be an object.`);
	}

	const rawMetrics = isObject(raw["metrics"]) ? raw["metrics"] : {};
	const metrics: NodeMetrics = {
		fan_in: toCount(rawMetrics["fan_in"]),
		fan_out: toCount(rawMetrics["fan_out"]),
		depth: toDepth(rawMetrics["depth"]),
		transitive_deps: toCount(rawMetrics["transitive_deps"]),
		transitive_dependents: toCount(rawMetrics["transitive_dependents"]),
	};

	const rawFlags = Array.isArray(raw["flags"]) ? raw["flags"] : [];
	const flags = rawFlags.filter((flag): flag is NodeFlag => typeof flag === "string" && NODE_FLAGS.has(flag));

	return {
		path: typeof raw["path"] === "string" ? raw["path"] : id,
		metrics,
		flags,
	};
}

/**
 * Normalizes a single edge entry.
 * @param raw - Raw edge value
 * @param index - Index in the `edges` array, used in error messages
 * @returns The normalized edge
 * @throws SassDepFormatError when required fields are missing or invalid
 */
function normalizeEdge(raw: unknown, index: number): OutputEdge {
	const where = `edges[${index}]`;
	if (!isObject(raw)) {
		throw new SassDepFormatError(`${where} must be an object.`);
	}
	if (typeof raw["from"] !== "string" || typeof raw["to"] !== "string") {
		throw new SassDepFormatError(`${where} must have string "from" and "to" fields.`);
	}

	const edge: OutputEdge = {
		from: raw["from"],
		to: raw["to"],
		directive_type: toDirectiveType(raw["directive_type"], where),
		location: toLocation(raw["location"]),
		configured: raw["configured"] === true,
	};
	if (typeof raw["namespace"] === "string") {
		edge.namespace = raw["namespace"];
	}
	return edge;
}

/**
 * Normalizes an unresolved import entry.
 * @param raw - Raw entry
 * @param index - Index in the `unresolved` array, used in error messages
 * @returns The normalized entry
 * @throws SassDepFormatError when required fields are missing or invalid
 */
function normalizeUnresolved(raw: unknown, index: number): UnresolvedImport {
	const where = `analysis.unresolved[${index}]`;
	if (!isObject(raw) || typeof raw["from"] !== "string" || typeof raw["target"] !== "string") {
		throw new SassDepFormatError(`${where} must be an object with string "from" and "target" fields.`);
	}
	return {
		from: raw["from"],
		target: raw["target"],
		directive_type: toDirectiveType(raw["directive_type"], where),
		location: toLocation(raw["location"]),
	};
}

/**
 * Normalizes the cycle list, dropping malformed entries.
 * @param raw - Raw `analysis.cycles` value
 * @returns Array of cycles, each an array of node IDs
 */
function normalizeCycles(raw: unknown): string[][] {
	if (!Array.isArray(raw)) return [];
	const cycles: string[][] = [];
	for (const cycle of raw) {
		if (Array.isArray(cycle)) {
			const ids = cycle.filter((id): id is string => typeof id === "string");
			if (ids.length > 0) cycles.push(ids);
		}
	}
	return cycles;
}

/**
 * Normalizes summary statistics. Fields introduced in v1.1.0 are derived from the
 * rest of the document when absent, so older files still show meaningful numbers.
 * @param raw - Raw `analysis.statistics` value
 * @param derived - Values derived from the document used as fallbacks for missing fields
 * @returns Complete statistics object
 */
function normalizeStatistics(raw: unknown, derived: Statistics): Statistics {
	const stats = isObject(raw) ? raw : {};
	return {
		total_files: toCount(stats["total_files"], derived.total_files),
		total_dependencies: toCount(stats["total_dependencies"], derived.total_dependencies),
		entry_points: toCount(stats["entry_points"], derived.entry_points),
		orphan_files: toCount(stats["orphan_files"], derived.orphan_files),
		leaf_files: toCount(stats["leaf_files"], derived.leaf_files),
		max_depth: toCount(stats["max_depth"], derived.max_depth),
		max_fan_in: toCount(stats["max_fan_in"], derived.max_fan_in),
		max_fan_out: toCount(stats["max_fan_out"], derived.max_fan_out),
		cycle_count: toCount(stats["cycle_count"], derived.cycle_count),
		unresolved_imports: toCount(stats["unresolved_imports"], derived.unresolved_imports),
		import_directives: toCount(stats["import_directives"], derived.import_directives),
	};
}

/**
 * Computes statistics from the normalized nodes and edges; used as fallbacks for missing fields.
 * @param nodes - Normalized node map
 * @param edges - Normalized edges
 * @param cycles - Normalized cycles
 * @param unresolved - Normalized unresolved imports
 * @returns Derived statistics
 */
function deriveStatistics(nodes: Record<string, OutputNode>, edges: OutputEdge[], cycles: string[][], unresolved: UnresolvedImport[]): Statistics {
	const stats: Statistics = {
		total_files: 0,
		total_dependencies: edges.length,
		entry_points: 0,
		orphan_files: 0,
		leaf_files: 0,
		max_depth: 0,
		max_fan_in: 0,
		max_fan_out: 0,
		cycle_count: cycles.length,
		unresolved_imports: unresolved.length,
		import_directives: 0,
	};

	for (const node of Object.values(nodes)) {
		stats.total_files++;
		if (node.flags.includes("entry_point")) stats.entry_points++;
		if (node.flags.includes("orphan")) stats.orphan_files++;
		if (node.flags.includes("leaf")) stats.leaf_files++;
		stats.max_depth = Math.max(stats.max_depth, node.metrics.depth ?? 0);
		stats.max_fan_in = Math.max(stats.max_fan_in, node.metrics.fan_in);
		stats.max_fan_out = Math.max(stats.max_fan_out, node.metrics.fan_out);
	}

	for (const edge of edges) {
		if (edge.directive_type === "import") stats.import_directives++;
	}

	return stats;
}

/**
 * Normalizes analysis metadata.
 * @param raw - Raw `metadata` value
 * @returns Metadata with string fields (empty when missing)
 */
function normalizeMetadata(raw: unknown): Metadata {
	const meta = isObject(raw) ? raw : {};
	const metadata: Metadata = {
		generated_at: typeof meta["generated_at"] === "string" ? meta["generated_at"] : "",
		root: typeof meta["root"] === "string" ? meta["root"] : "",
		sass_dep_version: typeof meta["sass_dep_version"] === "string" ? meta["sass_dep_version"] : "",
	};
	if (Array.isArray(meta["entry_points"])) {
		metadata.entry_points = meta["entry_points"].filter((id): id is string => typeof id === "string");
	}
	return metadata;
}

/**
 * Validates and normalizes a parsed sass-dep JSON document into the current (v1.1.0) shape.
 *
 * This is the single entry point for all data coming into the app (API fetch and file upload).
 * It tolerates v1.0.0 documents: a missing `unresolved` list becomes `[]`, missing metrics become 0,
 * missing v1.1.0 statistics are derived from the document, and a depth of `usize::MAX` becomes `null`.
 * Edges that reference unknown files are dropped (they cannot be drawn) with a console warning.
 * @param raw - Parsed JSON value of unknown shape
 * @returns A fully-populated SassDepOutput
 * @throws SassDepFormatError with a human-readable message when the document is not sass-dep output
 */
export function normalizeOutput(raw: unknown): SassDepOutput {
	if (!isObject(raw)) {
		throw new SassDepFormatError("Expected a JSON object produced by `sass-dep analyze`.");
	}

	const missing = ["nodes", "edges", "analysis"].filter((key) => !(key in raw));
	if (missing.length > 0) {
		throw new SassDepFormatError(`Not a sass-dep output file: missing required field${missing.length > 1 ? "s" : ""} ${missing.map((key) => `"${key}"`).join(", ")}.`);
	}
	if (!isObject(raw["nodes"])) {
		throw new SassDepFormatError('"nodes" must be an object mapping file IDs to node data.');
	}
	if (!Array.isArray(raw["edges"])) {
		throw new SassDepFormatError('"edges" must be an array.');
	}
	if (!isObject(raw["analysis"])) {
		throw new SassDepFormatError('"analysis" must be an object.');
	}

	const version = typeof raw["version"] === "string" ? raw["version"] : "";
	if (!version.startsWith("1.")) {
		console.warn(`sass-dep schema version "${version}" may not be fully compatible with this visualizer (expects ${SUPPORTED_SCHEMA_VERSION}).`);
	}

	const nodes: Record<string, OutputNode> = {};
	for (const [id, node] of Object.entries(raw["nodes"])) {
		nodes[id] = normalizeNode(id, node);
	}

	const edges: OutputEdge[] = [];
	let droppedEdges = 0;
	for (const [index, rawEdge] of raw["edges"].entries()) {
		const edge = normalizeEdge(rawEdge, index);
		if (edge.from in nodes && edge.to in nodes) {
			edges.push(edge);
		} else {
			droppedEdges++;
		}
	}
	if (droppedEdges > 0) {
		console.warn(`Ignored ${droppedEdges} edge(s) referencing files that are not in "nodes".`);
	}

	const rawAnalysis = raw["analysis"];
	const cycles = normalizeCycles(rawAnalysis["cycles"]);
	const unresolved = Array.isArray(rawAnalysis["unresolved"]) ? rawAnalysis["unresolved"].map(normalizeUnresolved) : [];
	const statistics = normalizeStatistics(rawAnalysis["statistics"], deriveStatistics(nodes, edges, cycles, unresolved));
	const analysis: Analysis = { cycles, unresolved, statistics };

	return {
		$schema: typeof raw["$schema"] === "string" ? raw["$schema"] : "",
		version,
		metadata: normalizeMetadata(raw["metadata"]),
		nodes,
		edges,
		analysis,
	};
}

/**
 * Returns a human-readable message for an error thrown while loading data.
 * @param error - The caught error
 * @param fallback - Message used when the error has no message
 * @returns Error message suitable for display
 */
export function describeLoadError(error: unknown, fallback: string): string {
	if (error instanceof SyntaxError) {
		return `The file is not valid JSON: ${error.message}`;
	}
	if (error instanceof Error && error.message) {
		return error.message;
	}
	return fallback;
}
