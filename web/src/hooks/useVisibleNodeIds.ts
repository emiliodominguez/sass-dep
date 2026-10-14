import { useDeferredValue, useMemo } from "react";

import { buildSearchableNodes, createNodePredicate } from "../utils/nodeFilter";
import type { AdvancedFilters, FilterFlag } from "../types/filters";
import type { SassDepOutput } from "../types/sass-dep";

/**
 * Computes the set of visible file IDs from the dataset and the current filters.
 *
 * This is the single source of truth for visibility, shared by the toolbar count and the graph.
 * It is computed from `data.nodes` (never from React Flow node state) and the search query is
 * deferred with `useDeferredValue`, so typing stays responsive on large graphs.
 * @param data - Normalized sass-dep output
 * @param searchQuery - Raw search text from the toolbar
 * @param activeFilters - Selected flag filters
 * @param advancedFilters - Numeric metric range filters
 * @returns A memoized Set of visible file IDs; its identity only changes when an input changes
 */
export function useVisibleNodeIds(data: SassDepOutput, searchQuery: string, activeFilters: readonly FilterFlag[], advancedFilters: AdvancedFilters): Set<string> {
	const deferredQuery = useDeferredValue(searchQuery);
	const searchable = useMemo(() => buildSearchableNodes(data), [data]);

	return useMemo(() => {
		const isVisible = createNodePredicate({ query: deferredQuery, flags: activeFilters, advanced: advancedFilters });
		const visible = new Set<string>();
		for (const entry of searchable) {
			if (isVisible(entry)) visible.add(entry.id);
		}
		return visible;
	}, [searchable, deferredQuery, activeFilters, advancedFilters]);
}
