import { type ReactNode, memo, useState } from "react";

import { Handle, type NodeProps, Position } from "@xyflow/react";

import type { FileFlowNode } from "./utils";
import type { NodeFlag } from "../../types/sass-dep";
import styles from "./FileNode.module.scss";

/** Maps flag names to CSS module class names (node colours are driven entirely by these classes). */
const FLAG_CLASS_MAP: Readonly<Record<NodeFlag, string>> = {
	entry_point: "entry-point",
	in_cycle: "in-cycle",
	orphan: "orphan",
	leaf: "leaf",
	high_fan_in: "high-fan-in",
	high_fan_out: "high-fan-out",
};

/** Props for the MetricBadge component. */
interface MetricBadgeProps {
	/** Icon rendered before the value */
	icon: ReactNode;
	/** Metric value */
	value: number;
	/** Tooltip heading */
	title: string;
	/** Tooltip explanation */
	description: string;
}

/**
 * Small metric badge with a hover tooltip. The tooltip is only mounted while hovered,
 * which keeps the DOM small when thousands of nodes are rendered.
 * @param props - Component props
 * @returns Badge element
 */
function MetricBadge({ icon, value, title, description }: MetricBadgeProps) {
	const [isHovered, setIsHovered] = useState(false);

	/** Mounts the tooltip. */
	function handleMouseEnter() {
		setIsHovered(true);
	}

	/** Unmounts the tooltip. */
	function handleMouseLeave() {
		setIsHovered(false);
	}

	return (
		<span className={styles["metric-badge"]} onMouseEnter={handleMouseEnter} onMouseLeave={handleMouseLeave}>
			{icon}
			{value}
			{isHovered && (
				<div className={styles["tooltip"]}>
					<strong>
						{title}: {value}
					</strong>
					<p>{description}</p>
				</div>
			)}
		</span>
	);
}

/** Icon for the fan-in badge. */
const FAN_IN_ICON = (
	<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
		<path d="M12 19V5M5 12l7-7 7 7" />
	</svg>
);

/** Icon for the fan-out badge. */
const FAN_OUT_ICON = (
	<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
		<path d="M12 5v14M5 12l7 7 7-7" />
	</svg>
);

/**
 * File node component for the dependency graph visualization.
 * Colours come from flag CSS classes; highlight state comes from decoration flags in `data`.
 * @param props - React Flow node props for a file node
 * @returns A styled node with file name, metrics, and visual indicators
 */
function FileNodeComponent({ data, selected }: NodeProps<FileFlowNode>) {
	const flagClass = data.primaryFlag ? FLAG_CLASS_MAP[data.primaryFlag] : "";

	const classNames = [
		styles["file-node"],
		selected && styles["selected"],
		flagClass && styles[flagClass],
		data.isFocused && styles["focused"],
		data.isPathSource && styles["path-source"],
		data.isPathTarget && styles["path-target"],
		data.isInPath && styles["in-path"],
		data.isInCycle && styles["cycle-highlight"],
		data.isLegacyImport && styles["legacy-import"],
	]
		.filter(Boolean)
		.join(" ");

	return (
		<div className={classNames}>
			<Handle type="target" position={Position.Left} />

			<div className={styles["content"]}>
				<div className={styles["label"]} title={data.fullPath}>
					{data.label}
				</div>
				<div className={styles["metrics"]}>
					<MetricBadge
						icon={FAN_IN_ICON}
						value={data.fanIn}
						title="Fan In"
						description="Number of files that import this file (dependents). High values mean this file is widely used."
					/>
					<MetricBadge
						icon={FAN_OUT_ICON}
						value={data.fanOut}
						title="Fan Out"
						description="Number of files this file imports (dependencies). High values may indicate the file does too much."
					/>
				</div>
			</div>

			<Handle type="source" position={Position.Right} />
		</div>
	);
}

/** Memoized file node; re-renders only when its node object (data/selection) changes. */
const FileNode = memo(FileNodeComponent);

export default FileNode;
