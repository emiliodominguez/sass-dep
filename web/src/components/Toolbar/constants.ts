import type { NodeFlag } from "../../types/sass-dep";

export interface ToolbarHandle {
	focusSearch: () => void;
}

/** Filter options with flag mappings */
export const FILTER_OPTIONS: { flag: NodeFlag | "no_flags"; label: string; className: string }[] = [
	{ flag: "entry_point", label: "Entry Points", className: "entry-point" },
	{ flag: "in_cycle", label: "In Cycle", className: "in-cycle" },
	{ flag: "orphan", label: "Orphans", className: "orphan" },
	{ flag: "leaf", label: "Leaves", className: "leaf" },
	{ flag: "high_fan_in", label: "High Fan-In", className: "high-fan-in" },
	{ flag: "high_fan_out", label: "High Fan-Out", className: "high-fan-out" },
	{ flag: "no_flags", label: "No Flags", className: "no-flags" },
];

export const ALL_FILTER_FLAGS = FILTER_OPTIONS.map((option) => option.flag);

export interface AdvancedFilters {
	minDepth: number | null;
	maxDepth: number | null;
	minFanIn: number | null;
	maxFanIn: number | null;
	minFanOut: number | null;
	maxFanOut: number | null;
}

export const DEFAULT_ADVANCED_FILTERS: AdvancedFilters = {
	minDepth: null,
	maxDepth: null,
	minFanIn: null,
	maxFanIn: null,
	minFanOut: null,
	maxFanOut: null,
};
