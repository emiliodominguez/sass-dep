import type { AdvancedFilters, FilterFlag } from "../../types/filters";

export type { AdvancedFilters, FilterFlag };

/** Imperative handle exposed by the Toolbar component. */
export interface ToolbarHandle {
	/** Focuses and selects the search input. */
	focusSearch: () => void;
}

/** A selectable entry in the flag filter dropdown. */
export interface FilterOption {
	/** The flag this option toggles */
	flag: FilterFlag;
	/** Human-readable label */
	label: string;
	/** CSS module class name used for the colour swatch */
	className: string;
}

/** Filter options with flag mappings */
export const FILTER_OPTIONS: readonly FilterOption[] = [
	{ flag: "entry_point", label: "Entry Points", className: "entry-point" },
	{ flag: "in_cycle", label: "In Cycle", className: "in-cycle" },
	{ flag: "orphan", label: "Orphans", className: "orphan" },
	{ flag: "leaf", label: "Leaves", className: "leaf" },
	{ flag: "high_fan_in", label: "High Fan-In", className: "high-fan-in" },
	{ flag: "high_fan_out", label: "High Fan-Out", className: "high-fan-out" },
	{ flag: "no_flags", label: "No Flags", className: "no-flags" },
];

/** Every filter flag; the default selection (shows all nodes). */
export const ALL_FILTER_FLAGS: FilterFlag[] = FILTER_OPTIONS.map((option) => option.flag);

/** Advanced filters with no bounds set. */
export const DEFAULT_ADVANCED_FILTERS: AdvancedFilters = {
	minDepth: null,
	maxDepth: null,
	minFanIn: null,
	maxFanIn: null,
	minFanOut: null,
	maxFanOut: null,
};
