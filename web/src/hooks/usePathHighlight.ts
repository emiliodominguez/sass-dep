import { useMemo } from "react";

import type { SassDepOutput } from "../types/sass-dep";

/** Result returned by the usePathHighlight hook. */
interface PathHighlightResult {
	/** Set of node IDs in the highlighted path. */
	pathNodeIds: Set<string>;
	/** Set of edge keys in the highlighted path (format: "source->target"). */
	pathEdgeKeys: Set<string>;
	/** Whether a valid path exists between source and target. */
	hasPath: boolean;
}

/**
 * Finds the shortest path between two nodes using BFS.
 * @param from - Source node ID
 * @param to - Target node ID
 * @param edges - All edges in the graph
 * @returns Array of node IDs in the path, or null if no path exists
 */
function findShortestPath(from: string, to: string, edges: SassDepOutput["edges"]): string[] | null {
	if (from === to) return [from];

	const forward = new Map<string, string[]>();
	const backward = new Map<string, string[]>();

	for (const edge of edges) {
		if (!forward.has(edge.from)) forward.set(edge.from, []);
		forward.get(edge.from)!.push(edge.to);

		if (!backward.has(edge.to)) backward.set(edge.to, []);
		backward.get(edge.to)!.push(edge.from);
	}

	const visited = new Set<string>();
	const parent = new Map<string, string | null>();
	const queue: string[] = [from];
	visited.add(from);
	parent.set(from, null);

	while (queue.length > 0) {
		const current = queue.shift()!;

		if (current === to) {
			const path: string[] = [];
			let node: string | null = to;
			while (node !== null) {
				path.unshift(node);
				node = parent.get(node) ?? null;
			}
			return path;
		}

		const forwardNeighbors = forward.get(current) || [];
		const backwardNeighbors = backward.get(current) || [];
		const neighbors = [...forwardNeighbors, ...backwardNeighbors];

		for (const neighbor of neighbors) {
			if (!visited.has(neighbor)) {
				visited.add(neighbor);
				parent.set(neighbor, current);
				queue.push(neighbor);
			}
		}
	}

	return null;
}

/**
 * Creates edge keys for path edges (checks both directions).
 * @param path - Array of node IDs in the path
 * @param edges - All edges in the graph
 * @returns Set of edge keys in the path
 */
function createPathEdgeKeys(path: string[], edges: SassDepOutput["edges"]): Set<string> {
	const keys = new Set<string>();

	for (let i = 0; i < path.length - 1; i++) {
		const a = path[i];
		const b = path[i + 1];

		for (const edge of edges) {
			if ((edge.from === a && edge.to === b) || (edge.from === b && edge.to === a)) {
				keys.add(`${edge.from}->${edge.to}`);
			}
		}
	}

	return keys;
}

/**
 * Hook for computing path highlighting data between two selected nodes.
 * @param sourceNodeId - The source node ID, or null
 * @param targetNodeId - The target node ID, or null
 * @param edges - All edges in the graph
 * @returns Path highlighting result with node IDs, edge keys, and hasPath flag
 */
export function usePathHighlight(sourceNodeId: string | null, targetNodeId: string | null, edges: SassDepOutput["edges"]): PathHighlightResult {
	return useMemo(() => {
		if (!sourceNodeId || !targetNodeId || sourceNodeId === targetNodeId) {
			return {
				pathNodeIds: new Set<string>(),
				pathEdgeKeys: new Set<string>(),
				hasPath: false,
			};
		}

		const path = findShortestPath(sourceNodeId, targetNodeId, edges);

		if (!path) {
			return {
				pathNodeIds: new Set<string>(),
				pathEdgeKeys: new Set<string>(),
				hasPath: false,
			};
		}

		return {
			pathNodeIds: new Set(path),
			pathEdgeKeys: createPathEdgeKeys(path, edges),
			hasPath: true,
		};
	}, [sourceNodeId, targetNodeId, edges]);
}
