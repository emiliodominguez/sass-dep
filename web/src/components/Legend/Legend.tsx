import "./Legend.css";

const NODE_DESCRIPTIONS: Record<string, string> = {
	"entry-point": "Files that are not imported by any other file. These are typically your main stylesheets.",
	"in-cycle": "Files involved in circular dependencies. Cycles can cause build issues and should be resolved.",
	orphan: "Files that neither import nor are imported by other files. Consider removing unused files.",
	leaf: "Files that import other files but are not imported by anyone. Usually utility or base files.",
	"high-fan-in": "Files imported by many others. Changes here have wide impact - handle with care.",
	"high-fan-out": "Files that import many others. May indicate a file doing too much.",
};

const EDGE_DESCRIPTIONS: Record<string, string> = {
	use: "Modern Sass import. Namespaced, explicit, and recommended for new code.",
	forward: "Re-exports members from another module. Used to create public APIs.",
	import: "Legacy import (deprecated). Consider migrating to @use for better encapsulation.",
};

interface LegendItemProps {
	icon: React.ReactNode;
	label: string;
	description: string;
}

function LegendItem({ icon, label, description }: LegendItemProps) {
	return (
		<li className="legend-item">
			{icon}
			<span className="legend-label">{label}</span>
			<span className="legend-info-icon">?</span>
			<div className="legend-tooltip">{description}</div>
		</li>
	);
}

export function Legend() {
	return (
		<div className="legend">
			<div className="legend-section">
				<h4>Node Types</h4>
				<ul className="legend-list">
					<LegendItem icon={<span className="swatch swatch-entry-point" />} label="Entry Point" description={NODE_DESCRIPTIONS["entry-point"]} />
					<LegendItem icon={<span className="swatch swatch-in-cycle" />} label="In Cycle" description={NODE_DESCRIPTIONS["in-cycle"]} />
					<LegendItem icon={<span className="swatch swatch-orphan" />} label="Orphan" description={NODE_DESCRIPTIONS["orphan"]} />
					<LegendItem icon={<span className="swatch swatch-leaf" />} label="Leaf" description={NODE_DESCRIPTIONS["leaf"]} />
					<LegendItem icon={<span className="swatch swatch-high-fan-in" />} label="High Fan-In" description={NODE_DESCRIPTIONS["high-fan-in"]} />
					<LegendItem icon={<span className="swatch swatch-high-fan-out" />} label="High Fan-Out" description={NODE_DESCRIPTIONS["high-fan-out"]} />
				</ul>
			</div>

			<div className="legend-section">
				<h4>Edge Types</h4>
				<ul className="legend-list">
					<LegendItem icon={<span className="line solid line-use" />} label="@use" description={EDGE_DESCRIPTIONS["use"]} />
					<LegendItem icon={<span className="line dashed line-forward" />} label="@forward" description={EDGE_DESCRIPTIONS["forward"]} />
					<LegendItem icon={<span className="line dotted line-import" />} label="@import" description={EDGE_DESCRIPTIONS["import"]} />
				</ul>
			</div>
		</div>
	);
}
