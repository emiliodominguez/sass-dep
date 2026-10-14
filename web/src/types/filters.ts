import type { NodeFlag } from "./sass-dep";

/** A flag that can be toggled in the toolbar filter: any node flag, or `"no_flags"` for nodes without flags. */
export type FilterFlag = NodeFlag | "no_flags";

/** Numeric range filters on node metrics. `null` means the bound is not set. */
export interface AdvancedFilters {
	/** Minimum depth (inclusive) */
	minDepth: number | null;
	/** Maximum depth (inclusive) */
	maxDepth: number | null;
	/** Minimum fan-in (inclusive) */
	minFanIn: number | null;
	/** Maximum fan-in (inclusive) */
	maxFanIn: number | null;
	/** Minimum fan-out (inclusive) */
	minFanOut: number | null;
	/** Maximum fan-out (inclusive) */
	maxFanOut: number | null;
}
