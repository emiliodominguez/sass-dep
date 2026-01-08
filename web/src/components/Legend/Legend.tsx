import { useState } from "react";
import "./Legend.css";

const NODE_ITEMS = [
	{ className: "swatch-entry-point", label: "Entry", title: "Entry Point - Files not imported by any other file" },
	{ className: "swatch-in-cycle", label: "Cycle", title: "In Cycle - Files involved in circular dependencies" },
	{ className: "swatch-orphan", label: "Orphan", title: "Orphan - Files not connected to the graph" },
	{ className: "swatch-leaf", label: "Leaf", title: "Leaf - Files with no dependencies" },
	{ className: "swatch-high-fan-in", label: "Fan-In", title: "High Fan-In - Files imported by many others" },
	{ className: "swatch-high-fan-out", label: "Fan-Out", title: "High Fan-Out - Files that import many others" },
];

const EDGE_ITEMS = [
	{ className: "line-use", style: "solid", label: "@use", title: "@use - Modern namespaced import" },
	{ className: "line-forward", style: "dashed", label: "@forward", title: "@forward - Re-exports members" },
	{ className: "line-import", style: "dotted", label: "@import", title: "@import - Legacy import (deprecated)" },
];

const SHORTCUTS = [
	{ key: "/", label: "Search" },
	{ key: "f", label: "Fit" },
	{ key: "Esc", label: "Clear" },
	{ key: "Shift+Click", label: "Path" },
];

export function Legend() {
	const [isExpanded, setIsExpanded] = useState(true);

	return (
		<div className={`legend ${isExpanded ? "legend-expanded" : "legend-collapsed"}`}>
			<button className="legend-toggle" onClick={() => setIsExpanded(!isExpanded)} title={isExpanded ? "Collapse legend" : "Expand legend"}>
				<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
					{isExpanded ? <polyline points="18 15 12 9 6 15" /> : <polyline points="6 9 12 15 18 9" />}
				</svg>
				<span>Legend</span>
			</button>

			{isExpanded && (
				<div className="legend-content">
					<div className="legend-sections">
						<div className="legend-section">
							<span className="legend-section-title">Nodes</span>
							<div className="legend-column">
								{NODE_ITEMS.map((item) => (
									<span key={item.className} className="legend-chip" title={item.title}>
										<span className={`swatch ${item.className}`} />
										{item.label}
									</span>
								))}
							</div>
						</div>
						<div className="legend-section">
							<span className="legend-section-title">Edges</span>
							<div className="legend-column">
								{EDGE_ITEMS.map((item) => (
									<span key={item.className} className="legend-chip" title={item.title}>
										<span className={`line ${item.style} ${item.className}`} />
										{item.label}
									</span>
								))}
							</div>
						</div>
					</div>
					<div className="legend-shortcuts-row">
						{SHORTCUTS.map((s) => (
							<span key={s.key} className="legend-shortcut" title={s.label}>
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
