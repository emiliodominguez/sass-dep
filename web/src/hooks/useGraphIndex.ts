import { useMemo } from "react";

import { type GraphIndex, buildGraphIndex } from "../utils/graphIndex";
import type { SassDepOutput } from "../types/sass-dep";

/**
 * Builds the adjacency index for a dataset once and memoizes it for the lifetime of that dataset.
 * @param data - Normalized sass-dep output
 * @returns Graph index with outgoing/incoming adjacency, edge lookup and cycle/import sets
 */
export function useGraphIndex(data: SassDepOutput): GraphIndex {
	return useMemo(() => buildGraphIndex(data), [data]);
}
