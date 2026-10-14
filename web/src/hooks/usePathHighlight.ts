import { useMemo } from "react";

import { type DependencyPath, findDependencyPath } from "../utils/pathFinding";
import { type GraphIndex, edgePairKey } from "../utils/graphIndex";

/** Result returned by the usePathHighlight hook. */
export interface PathHighlightResult {
	/** The found path, or null when there is none (or fewer than two endpoints are selected) */
	path: DependencyPath | null;
	/** Set of node IDs in the highlighted path */
	pathNodeIds: Set<string>;
	/** Set of edge pair keys in the highlighted path (format: "from->to") */
	pathEdgeKeys: Set<string>;
	/** Whether both endpoints are selected and a path was searched for */
	isComplete: boolean;
}

/** Result used when no complete pair of endpoints is selected. */
const EMPTY_RESULT: PathHighlightResult = {
	path: null,
	pathNodeIds: new Set<string>(),
	pathEdgeKeys: new Set<string>(),
	isComplete: false,
};

/**
 * Computes the dependency path between two shift-clicked files using the prebuilt adjacency index.
 * Searches the directed graph `source → target` first, then `target → source`.
 * @param sourceNodeId - First selected node ID, or null
 * @param targetNodeId - Second selected node ID, or null
 * @param index - Graph adjacency index
 * @returns Path, its direction, and the node/edge sets to highlight
 */
export function usePathHighlight(sourceNodeId: string | null, targetNodeId: string | null, index: GraphIndex): PathHighlightResult {
	return useMemo(() => {
		if (!sourceNodeId || !targetNodeId || sourceNodeId === targetNodeId) {
			return EMPTY_RESULT;
		}

		const path = findDependencyPath(sourceNodeId, targetNodeId, index.outgoing);
		if (!path) {
			return { ...EMPTY_RESULT, isComplete: true };
		}

		const pathEdgeKeys = new Set<string>();
		for (let i = 0; i < path.nodes.length - 1; i++) {
			pathEdgeKeys.add(edgePairKey(path.nodes[i]!, path.nodes[i + 1]!));
		}

		return { path, pathNodeIds: new Set(path.nodes), pathEdgeKeys, isComplete: true };
	}, [sourceNodeId, targetNodeId, index]);
}
