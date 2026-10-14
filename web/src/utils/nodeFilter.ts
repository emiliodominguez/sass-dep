import type { AdvancedFilters, FilterFlag } from "../types/filters";
import type { OutputNode, SassDepOutput } from "../types/sass-dep";

/** Everything that decides whether a node is visible. */
export interface NodeFilterCriteria {
	/** Free-text search; matched case-insensitively against the file ID and full path */
	query: string;
	/** Selected flag filters; a node matches if it has any of them. An empty list matches everything. */
	flags: readonly FilterFlag[];
	/** Numeric metric ranges */
	advanced: AdvancedFilters;
}

/** A node prepared for fast repeated filtering. */
export interface SearchableNode {
	/** File ID */
	id: string;
	/** Node data */
	node: OutputNode;
	/** Lower-cased `id` and `path`, joined, for substring search */
	haystack: string;
}

/**
 * Pre-computes lower-cased search text for every node. Run once per dataset.
 * @param data - Normalized sass-dep output
 * @returns Searchable node entries in `data.nodes` order
 */
export function buildSearchableNodes(data: SassDepOutput): SearchableNode[] {
	return Object.entries(data.nodes).map(([id, node]) => ({ id, node, haystack: `${id}\n${node.path}`.toLowerCase() }));
}

/**
 * Checks whether a value lies within optional inclusive bounds.
 * @param value - Value to test
 * @param min - Lower bound, or null for none
 * @param max - Upper bound, or null for none
 * @returns True when the value satisfies both bounds
 */
function inRange(value: number, min: number | null, max: number | null): boolean {
	return (min === null || value >= min) && (max === null || value <= max);
}

/**
 * Creates the single visibility predicate shared by the toolbar count and the graph.
 * The query is lower-cased once here rather than per node.
 * @param criteria - Search, flag and advanced filters
 * @returns Predicate returning true when a node should be visible
 */
export function createNodePredicate(criteria: NodeFilterCriteria): (entry: SearchableNode) => boolean {
	const query = criteria.query.trim().toLowerCase();
	const flagSet = new Set(criteria.flags);
	const matchAnyFlag = flagSet.size === 0;
	const includeUnflagged = flagSet.has("no_flags");
	const { minDepth, maxDepth, minFanIn, maxFanIn, minFanOut, maxFanOut } = criteria.advanced;
	const hasDepthBounds = minDepth !== null || maxDepth !== null;

	return function isNodeVisible({ node, haystack }: SearchableNode): boolean {
		if (query && !haystack.includes(query)) return false;

		if (!matchAnyFlag) {
			const matchesFlag = node.flags.length === 0 ? includeUnflagged : node.flags.some((flag) => flagSet.has(flag));
			if (!matchesFlag) return false;
		}

		const { depth, fan_in, fan_out } = node.metrics;
		// Unreachable files (null depth) only match when no depth bounds are set.
		if (hasDepthBounds && (depth === null || !inRange(depth, minDepth, maxDepth))) return false;

		return inRange(fan_in, minFanIn, maxFanIn) && inRange(fan_out, minFanOut, maxFanOut);
	};
}
