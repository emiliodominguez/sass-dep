import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { FileNodeData, FileNode as FileNodeType } from "./utils";
import "./FileNode.css";

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

function FileNodeComponent({ data, selected }: FileNodeProps) {
	const isFocused = data.isFocused ?? false;
	const isPathSource = data.isPathSource ?? false;
	const isPathTarget = data.isPathTarget ?? false;
	const isInPath = data.isInPath ?? false;
	const isInCycle = data.isInCycle ?? false;
	const flagClass = data.primaryFlag?.replace(/-/g, "_") || "";

	const pathClass = isPathSource ? "path-source" : isPathTarget ? "path-target" : isInPath ? "in-path" : "";

	return (
		<div className={`file-node ${selected ? "selected" : ""} ${flagClass} ${isFocused ? "focused" : ""} ${pathClass} ${isInCycle ? "cycle-highlight" : ""}`}>
			<Handle type="target" position={Position.Left} />

			<div className="file-node-content">
				<div className="file-node-label" title={data.fullPath}>
					{data.label}
				</div>
				<div className="file-node-metrics">
					<span className="metric-badge">
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<path d="M12 19V5M5 12l7-7 7 7" />
						</svg>
						{data.fanIn}
						<div className="metric-tooltip">
							<strong>Fan In: {data.fanIn}</strong>
							<p>Number of files that import this file (dependents). High values mean this file is widely used.</p>
						</div>
					</span>
					<span className="metric-badge">
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<path d="M12 5v14M5 12l7 7 7-7" />
						</svg>
						{data.fanOut}
						<div className="metric-tooltip">
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

// Also export the type for use elsewhere
export type { FileNodeType };
