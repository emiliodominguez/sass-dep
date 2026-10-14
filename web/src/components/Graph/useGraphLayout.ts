import { useEffect, useState } from "react";

import { type AppNode, type FileFlowNode, applyLayout, createLayoutInput } from "./utils";
import { runLayout } from "./runLayout";
import type { SassDepOutput } from "../../types/sass-dep";

/** A completed layout and the inputs it was computed for. */
interface LayoutState {
	/** Dataset the layout belongs to */
	data: SassDepOutput;
	/** Grouping mode the layout belongs to */
	grouped: boolean;
	/** Positioned, undecorated nodes */
	nodes: AppNode[];
}

/** A failed layout and the inputs it was attempted for. */
interface LayoutFailure {
	/** Dataset the layout was attempted for */
	data: SassDepOutput;
	/** Grouping mode the layout was attempted for */
	grouped: boolean;
	/** Error message */
	message: string;
}

/** Result of the useGraphLayout hook. */
export interface GraphLayout {
	/** Positioned nodes from the most recent completed layout (may be stale while `isPending`) */
	layoutNodes: AppNode[];
	/** Whether a layout for the current data/grouping is still being computed */
	isPending: boolean;
	/** Error message if the layout failed, or null */
	error: string | null;
}

/** Shared empty node list used before the first layout completes. */
const NO_NODES: AppNode[] = [];

/**
 * Completed layouts per dataset and grouping mode, so toggling grouping back and forth is instant.
 * Keyed weakly by the dataset object so old datasets can be garbage collected.
 */
const layoutCache = new WeakMap<SassDepOutput, Map<boolean, AppNode[]>>();

/**
 * Stores a completed layout in the cache.
 * @param data - Dataset the layout belongs to
 * @param grouped - Grouping mode the layout belongs to
 * @param nodes - Positioned nodes
 */
function cacheLayout(data: SassDepOutput, grouped: boolean, nodes: AppNode[]): void {
	let byMode = layoutCache.get(data);
	if (!byMode) {
		byMode = new Map();
		layoutCache.set(data, byMode);
	}
	byMode.set(grouped, nodes);
}

/**
 * Lays out the graph off the main thread. The layout only re-runs when the dataset or the grouping
 * mode changes (and completed layouts are cached per mode); results for an outdated dataset/grouping
 * are discarded (the worker is terminated).
 * @param data - Normalized sass-dep output
 * @param groupByFolder - Whether to group files by folder
 * @param fileNodes - Undecorated file nodes for `data` (in `data.nodes` order)
 * @returns The latest positioned nodes, a pending flag and any error
 */
export function useGraphLayout(data: SassDepOutput, groupByFolder: boolean, fileNodes: FileFlowNode[]): GraphLayout {
	const [state, setState] = useState<LayoutState | null>(null);
	const [failure, setFailure] = useState<LayoutFailure | null>(null);

	useEffect(() => {
		const cached = layoutCache.get(data)?.get(groupByFolder);
		if (cached) {
			let cancelled = false;
			queueMicrotask(
				/** Applies the cached layout unless the effect was cleaned up meanwhile. */
				function applyCachedLayout() {
					if (!cancelled) setState({ data, grouped: groupByFolder, nodes: cached });
				},
			);
			return () => {
				cancelled = true;
			};
		}

		const task = runLayout(createLayoutInput(data, groupByFolder));

		task.promise.then(
			/**
			 * Stores the positioned nodes of a finished layout.
			 * @param result - Layout computed by the worker (or the fallback)
			 */
			function handleLayoutDone(result) {
				const nodes = applyLayout(fileNodes, result);
				cacheLayout(data, groupByFolder, nodes);
				setState({ data, grouped: groupByFolder, nodes });
			},
			/**
			 * Records a layout failure so the overlay can show it.
			 * @param reason - Rejection reason
			 */
			function handleLayoutFailed(reason: unknown) {
				console.error("Graph layout failed", reason);
				setFailure({ data, grouped: groupByFolder, message: reason instanceof Error ? reason.message : String(reason) });
			},
		);

		return task.cancel;
	}, [data, groupByFolder, fileNodes]);

	const isCurrent = state !== null && state.data === data && state.grouped === groupByFolder;
	const error = failure !== null && failure.data === data && failure.grouped === groupByFolder ? failure.message : null;

	return {
		layoutNodes: state?.nodes ?? NO_NODES,
		isPending: !isCurrent && error === null,
		error,
	};
}
