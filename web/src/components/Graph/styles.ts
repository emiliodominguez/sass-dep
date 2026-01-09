/** Get CSS variable value from the document */
function getCSSVariable(name: string): string {
	if (typeof window === "undefined") return "";
	return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Node flag colors - fallbacks when CSS variables aren't available */
const FLAG_COLORS = {
	entry_point: "#dcfce7",
	orphan: "#fee2e2",
	in_cycle: "#fef3c7",
	leaf: "#f1f5f9",
	high_fan_in: "#dbeafe",
	high_fan_out: "#f3e8ff",
	default: "#ffffff",
} as const;

/** Edge colors */
export const EDGE_COLORS = {
	use: "#3b82f6",
	forward: "#22c55e",
	import: "#f97316",
} as const;

/** Get node background color based on primary flag */
export function getNodeColor(primaryFlag: string | null): string {
	const varName = primaryFlag ? `--color-flag-${primaryFlag.replace(/_/g, "-")}` : "--color-flag-default";
	const cssValue = getCSSVariable(varName);
	if (cssValue) return cssValue;

	if (!primaryFlag) return FLAG_COLORS.default;
	return FLAG_COLORS[primaryFlag as keyof typeof FLAG_COLORS] || FLAG_COLORS.default;
}

/** Get edge color based on directive type */
export function getEdgeColor(directiveType: string): string {
	const varName = `--color-edge-${directiveType}`;
	const cssValue = getCSSVariable(varName);
	if (cssValue) return cssValue;

	return EDGE_COLORS[directiveType as keyof typeof EDGE_COLORS] || "#64748b";
}

/** Get edge style based on directive type */
export function getEdgeStyle(directiveType: string): "default" | "dashed" | "dotted" {
	switch (directiveType) {
		case "forward":
			return "dashed";
		case "import":
			return "dotted";
		default:
			return "default";
	}
}
