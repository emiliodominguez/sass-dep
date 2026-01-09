import { memo } from "react";

import { Handle, Position } from "@xyflow/react";

import type { FileNodeData, FileNode as FileNodeType } from "./utils";
import styles from "./FileNode.module.scss";

interface FileNodeProps {
	data: FileNodeData & {
		isFocused?: boolean;
		isPathSource?: boolean;
		isPathTarget?: boolean;
		isInPath?: boolean;
		isInCycle?: boolean;
	};
	selected: boolean;
}

/** Maps flag names to CSS module class names */
const FLAG_CLASS_MAP: Record<string, string> = {
	entry_point: "entry-point",
	in_cycle: "in-cycle",
	orphan: "orphan",
	leaf: "leaf",
	high_fan_in: "high-fan-in",
	high_fan_out: "high-fan-out",
};

/**
 * File node component for the dependency graph visualization.
 * @param props - Component props containing node data and selection state
 * @returns A styled node with file name, metrics, and visual indicators
 */
function FileNodeComponent({ data, selected }: FileNodeProps) {
	const isFocused = data.isFocused ?? false;
	const isPathSource = data.isPathSource ?? false;
	const isPathTarget = data.isPathTarget ?? false;
	const isInPath = data.isInPath ?? false;
	const isInCycle = data.isInCycle ?? false;
	const flagClass = data.primaryFlag ? (FLAG_CLASS_MAP[data.primaryFlag] ?? "") : "";

	const classNames = [
		styles["file-node"],
		selected && styles["selected"],
		flagClass && styles[flagClass],
		isFocused && styles["focused"],
		isPathSource && styles["path-source"],
		isPathTarget && styles["path-target"],
		isInPath && styles["in-path"],
		isInCycle && styles["cycle-highlight"],
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
					<span className={styles["metric-badge"]}>
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<path d="M12 19V5M5 12l7-7 7 7" />
						</svg>
						{data.fanIn}
						<div className={styles["tooltip"]}>
							<strong>Fan In: {data.fanIn}</strong>
							<p>Number of files that import this file (dependents). High values mean this file is widely used.</p>
						</div>
					</span>
					<span className={styles["metric-badge"]}>
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<path d="M12 5v14M5 12l7 7 7-7" />
						</svg>
						{data.fanOut}
						<div className={styles["tooltip"]}>
							<strong>Fan Out: {data.fanOut}</strong>
							<p>Number of files this file imports (dependencies). High values may indicate the file does too much.</p>
						</div>
					</span>
				</div>
			</div>

			<Handle type="source" position={Position.Right} />
		</div>
	);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const FileNode = memo(FileNodeComponent) as any;

export default FileNode;
export type { FileNodeType };
