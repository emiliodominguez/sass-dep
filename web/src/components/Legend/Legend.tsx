import { useState } from "react";

import styles from "./Legend.module.scss";

/**
 * Node legend items with their associated style class and description
 */
const NODE_ITEMS = [
	{ className: "entry-point", label: "Entry", title: "Entry Point - Files not imported by any other file" },
	{ className: "in-cycle", label: "Cycle", title: "In Cycle - Files involved in circular dependencies" },
	{ className: "orphan", label: "Orphan", title: "Orphan - Files not connected to the graph" },
	{ className: "leaf", label: "Leaf", title: "Leaf - Files with no dependencies" },
	{ className: "high-fan-in", label: "Fan-In", title: "High Fan-In - Files imported by many others" },
	{ className: "high-fan-out", label: "Fan-Out", title: "High Fan-Out - Files that import many others" },
] as const;

/** Edge legend items with their associated style class, line style, and description */
const EDGE_ITEMS = [
	{ className: "use", lineStyle: "solid", label: "@use", title: "@use - Modern namespaced import" },
	{ className: "forward", lineStyle: "dashed", label: "@forward", title: "@forward - Re-exports members" },
	{ className: "import", lineStyle: "dotted", label: "@import", title: "@import - Legacy import (deprecated)" },
] as const;

/** Keyboard shortcuts displayed in the legend */
const SHORTCUTS = [
	{ key: "/", label: "Search" },
	{ key: "f", label: "Fit" },
	{ key: "Esc", label: "Clear" },
	{ key: "Shift+Click", label: "Path" },
] as const;

/**
 * Collapsible legend component showing node colors, edge styles, and keyboard shortcuts.
 * @returns Legend panel positioned in the top-left corner of the graph
 */
export function Legend() {
	const [isExpanded, setIsExpanded] = useState(true);

	/**
	 * Toggles the legend's expanded/collapsed state.
	 */
	function handleToggle() {
		setIsExpanded(!isExpanded);
	}

	return (
		<div className={`${styles["legend"]} ${isExpanded ? styles["expanded"] : styles["collapsed"]}`}>
			<button className={styles["toggle"]} onClick={handleToggle} title={isExpanded ? "Collapse legend" : "Expand legend"}>
				<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
					{isExpanded ? <polyline points="18 15 12 9 6 15" /> : <polyline points="6 9 12 15 18 9" />}
				</svg>
				<span>Legend</span>
			</button>

			{isExpanded && (
				<div className={styles["content"]}>
					<div className={styles["sections"]}>
						<div className={styles["section"]}>
							<span className={styles["section-title"]}>Nodes</span>
							<div className={styles["column"]}>
								{NODE_ITEMS.map((item) => (
									<span key={item.className} className={styles["chip"]} title={item.title}>
										<span className={`${styles["swatch"]} ${styles[item.className]}`} />
										{item.label}
									</span>
								))}
							</div>
						</div>
						<div className={styles["section"]}>
							<span className={styles["section-title"]}>Edges</span>
							<div className={styles["column"]}>
								{EDGE_ITEMS.map((item) => (
									<span key={item.className} className={styles["chip"]} title={item.title}>
										<span className={`${styles["line"]} ${styles[item.lineStyle]} ${styles[item.className]}`} />
										{item.label}
									</span>
								))}
							</div>
						</div>
					</div>
					<div className={styles["shortcuts-row"]}>
						{SHORTCUTS.map((s) => (
							<span key={s.key} className={styles["shortcut"]} title={s.label}>
								<kbd>{s.key}</kbd>
								{s.label}
							</span>
						))}
					</div>
				</div>
			)}
		</div>
	);
}
