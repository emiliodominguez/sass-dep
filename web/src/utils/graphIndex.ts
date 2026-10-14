import type { OutputEdge, SassDepOutput } from "../types/sass-dep";

/** Adjacency index built once per dataset so lookups never scan `data.edges`. */
export interface GraphIndex {
	/** Edges leaving each file (its dependencies), keyed by file ID */
	outgoing: Map<string, OutputEdge[]>;
	/** Edges entering each file (its dependents), keyed by file ID */
	incoming: Map<string, OutputEdge[]>;
	/**
	 * Edges keyed by their stable edge ID. The first edge between two files uses the plain pair key
	 * (see {@link edgePairKey}), so `edgeByKey.get(edgePairKey(a, b))` also works; later edges between
	 * the same pair get a `#n` suffix.
	 */
	edgeByKey: Map<string, OutputEdge>;
	/** Edge ID for each entry of `data.edges`, by array index */
	edgeIds: string[];
	/** Files that contain at least one legacy `@import` directive */
	importerIds: Set<string>;
	/** Files that appear in any cycle */
	cycleNodeIds: Set<string>;
	/** Pair keys (see {@link edgePairKey}) of edges that close or belong to a cycle */
	cycleEdgeKeys: Set<string>;
}

/**
 * Builds the key used to identify the (directed) connection between two files.
 *
 * The key is the JSON encoding of `[from, to]`, which is injective: unlike a plain `from->to`
 * concatenation, files named e.g. `a->b` and `c` can never produce the same key as `a` and `b->c`.
 * Keys always end with `"]`, so `#n` suffixes cannot collide with another pair key either.
 * @param from - Source file ID
 * @param to - Target file ID
 * @returns Unambiguous pair key
 */
export function edgePairKey(from: string, to: string): string {
	return JSON.stringify([from, to]);
}

/**
 * Appends a value to the array stored under `key`, creating it when missing.
 * @param map - Map of arrays
 * @param key - Key to append under
 * @param value - Value to append
 */
function pushTo<V>(map: Map<string, V[]>, key: string, value: V): void {
	const list = map.get(key);
	if (list) {
		list.push(value);
	} else {
		map.set(key, [value]);
	}
}

/**
 * Builds the adjacency index for a dataset in O(N + E).
 * @param data - Normalized sass-dep output
 * @returns The graph index
 */
export function buildGraphIndex(data: SassDepOutput): GraphIndex {
	const outgoing = new Map<string, OutputEdge[]>();
	const incoming = new Map<string, OutputEdge[]>();
	const edgeByKey = new Map<string, OutputEdge>();
	const edgeIds: string[] = [];
	const pairCounts = new Map<string, number>();
	const importerIds = new Set<string>();

	for (const edge of data.edges) {
		pushTo(outgoing, edge.from, edge);
		pushTo(incoming, edge.to, edge);

		// Multiple directives between the same pair of files are possible (e.g. @use + @forward),
		// so subsequent ones get a numeric suffix to keep IDs unique.
		const pairKey = edgePairKey(edge.from, edge.to);
		const count = (pairCounts.get(pairKey) ?? 0) + 1;
		pairCounts.set(pairKey, count);
		const id = count === 1 ? pairKey : `${pairKey}#${count}`;
		edgeByKey.set(id, edge);
		edgeIds.push(id);

		if (edge.directive_type === "import") {
			importerIds.add(edge.from);
		}
	}

	const cycleNodeIds = new Set<string>();
	const cycleEdgeKeys = new Set<string>();
	for (const cycle of data.analysis.cycles) {
		for (const [i, nodeId] of cycle.entries()) {
			cycleNodeIds.add(nodeId);
			const next = cycle[(i + 1) % cycle.length];
			if (next !== undefined) {
				cycleEdgeKeys.add(edgePairKey(nodeId, next));
			}
		}
	}

	return { outgoing, incoming, edgeByKey, edgeIds, importerIds, cycleNodeIds, cycleEdgeKeys };
}

/**
 * Returns the distinct neighbour file IDs of a file, preserving edge order.
 * @param index - Graph index
 * @param nodeId - File ID
 * @param direction - `"dependencies"` for files it loads, `"dependents"` for files that load it
 * @returns Unique neighbour IDs
 */
export function getNeighborIds(index: GraphIndex, nodeId: string, direction: "dependents" | "dependencies"): string[] {
	const edges = direction === "dependencies" ? index.outgoing.get(nodeId) : index.incoming.get(nodeId);
	if (!edges) return [];

	const seen = new Set<string>();
	for (const edge of edges) {
		seen.add(direction === "dependencies" ? edge.to : edge.from);
	}
	return [...seen];
}
