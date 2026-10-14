import type { CSSProperties } from "react";

import { type EdgeMarker, MarkerType } from "@xyflow/react";

import type { DirectiveType } from "../../types/sass-dep";

/**
 * Edge colours as CSS variable references. Because they are resolved by the browser,
 * theme switching just works and no `getComputedStyle` calls are needed.
 */
export const EDGE_COLOR_VARS: Readonly<Record<DirectiveType, string>> = {
	use: "var(--color-edge-use)",
	forward: "var(--color-edge-forward)",
	import: "var(--color-edge-import)",
};

/** Dash patterns that distinguish directive types without relying on (expensive) animation. */
const EDGE_DASH: Readonly<Record<DirectiveType, string | undefined>> = {
	use: undefined,
	forward: "6 4",
	import: "2 4",
};

/**
 * Dash patterns for large graphs. Dashes on long paths are surprisingly expensive to paint
 * (hundreds of ms per frame with thousands of edges), so only legacy `@import` keeps a
 * (coarser) dash; `@forward` is distinguished by colour alone.
 */
const LARGE_GRAPH_EDGE_DASH: Readonly<Record<DirectiveType, string | undefined>> = {
	use: undefined,
	forward: undefined,
	import: "10 8",
};

/** Why an edge is visually emphasised, if at all. Path highlighting wins over cycles, cycles over imports. */
export type EdgeHighlight = "none" | "path" | "cycle" | "import";

/** Stroke colour used for each highlight kind (`null` keeps the directive colour). */
const HIGHLIGHT_COLORS: Readonly<Record<EdgeHighlight, string | null>> = {
	none: null,
	path: "var(--color-accent)",
	cycle: "var(--color-flag-in-cycle-border)",
	import: null,
};

/**
 * Builds a lookup table of values for every directive/highlight combination so that
 * edge objects can share the same style and marker instances.
 * @param build - Factory for one combination
 * @returns Nested lookup table
 */
function buildTable<T>(build: (directive: DirectiveType, highlight: EdgeHighlight) => T): Record<DirectiveType, Record<EdgeHighlight, T>> {
	const directives: DirectiveType[] = ["use", "forward", "import"];
	const highlights: EdgeHighlight[] = ["none", "path", "cycle", "import"];
	const table = {} as Record<DirectiveType, Record<EdgeHighlight, T>>;
	for (const directive of directives) {
		table[directive] = {} as Record<EdgeHighlight, T>;
		for (const highlight of highlights) {
			table[directive][highlight] = build(directive, highlight);
		}
	}
	return table;
}

/**
 * Creates the stroke style for an edge.
 * @param directive - Directive type of the edge
 * @param highlight - Highlight state
 * @param dashes - Dash pattern per directive type
 * @returns SVG style for the edge path
 */
function createEdgeStyle(directive: DirectiveType, highlight: EdgeHighlight, dashes: Readonly<Record<DirectiveType, string | undefined>>): CSSProperties {
	const highlightColor = HIGHLIGHT_COLORS[highlight];
	const isEmphasised = highlight !== "none";
	return {
		stroke: highlightColor ?? EDGE_COLOR_VARS[directive],
		strokeWidth: isEmphasised ? 3 : 2,
		strokeDasharray: highlight === "path" || highlight === "cycle" ? undefined : dashes[directive],
	};
}

/**
 * Creates the stroke style for an edge in a normal-sized graph.
 * @param directive - Directive type of the edge
 * @param highlight - Highlight state
 * @returns SVG style for the edge path
 */
function createDefaultEdgeStyle(directive: DirectiveType, highlight: EdgeHighlight): CSSProperties {
	return createEdgeStyle(directive, highlight, EDGE_DASH);
}

/**
 * Creates the stroke style for an edge in a large graph.
 * @param directive - Directive type of the edge
 * @param highlight - Highlight state
 * @returns SVG style for the edge path
 */
function createLargeGraphEdgeStyle(directive: DirectiveType, highlight: EdgeHighlight): CSSProperties {
	return createEdgeStyle(directive, highlight, LARGE_GRAPH_EDGE_DASH);
}

/**
 * Creates the arrowhead marker for an edge.
 * @param directive - Directive type of the edge
 * @param highlight - Highlight state
 * @returns React Flow marker definition
 */
function createEdgeMarker(directive: DirectiveType, highlight: EdgeHighlight): EdgeMarker {
	return { type: MarkerType.ArrowClosed, color: HIGHLIGHT_COLORS[highlight] ?? EDGE_COLOR_VARS[directive] };
}

/** Shared edge styles for normal-sized graphs. */
const EDGE_STYLES = buildTable(createDefaultEdgeStyle);
/** Shared edge styles for large graphs. */
const LARGE_GRAPH_EDGE_STYLES = buildTable(createLargeGraphEdgeStyle);
/** Shared arrowhead markers. */
const EDGE_MARKERS = buildTable(createEdgeMarker);

/**
 * Returns the shared stroke style for an edge.
 * @param directive - Directive type of the edge
 * @param highlight - Highlight state
 * @param isLargeGraph - Whether the cheaper large-graph dash patterns should be used
 * @returns Shared (do not mutate) style object
 */
export function getEdgeStyle(directive: DirectiveType, highlight: EdgeHighlight, isLargeGraph: boolean): CSSProperties {
	return (isLargeGraph ? LARGE_GRAPH_EDGE_STYLES : EDGE_STYLES)[directive][highlight];
}

/**
 * Returns the shared arrowhead marker for an edge.
 * @param directive - Directive type of the edge
 * @param highlight - Highlight state
 * @returns Shared (do not mutate) marker object
 */
export function getEdgeMarker(directive: DirectiveType, highlight: EdgeHighlight): EdgeMarker {
	return EDGE_MARKERS[directive][highlight];
}
