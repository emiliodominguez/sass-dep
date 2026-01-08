import { useMemo, useState, useCallback, useEffect, useRef } from "react";
import type { OutputNode, OutputEdge, Statistics } from "../../types/sass-dep";
import { NodeDetails } from "./NodeDetails";
import { EdgeDetails } from "./EdgeDetails";
import "./Sidebar.css";

const MIN_WIDTH = 280;
const MAX_WIDTH = 600;
const DEFAULT_WIDTH = 320;
const STORAGE_KEY = "sass-dep-sidebar-width";

function getStoredWidth(): number {
	if (typeof window === "undefined") return DEFAULT_WIDTH;
	const stored = localStorage.getItem(STORAGE_KEY);
	if (stored) {
		const width = parseInt(stored, 10);
		if (!isNaN(width) && width >= MIN_WIDTH && width <= MAX_WIDTH) {
			return width;
		}
	}
	return DEFAULT_WIDTH;
}

interface SidebarProps {
	// Data props
	selectedNode: { id: string; node: OutputNode } | null;
	selectedEdge: OutputEdge | null;
	statistics: Statistics;
	edges: OutputEdge[];
	// Callbacks
	onFocusNode?: (nodeId: string) => void;
}

export function Sidebar({ selectedNode, selectedEdge, statistics, edges, onFocusNode }: SidebarProps) {
	const [width, setWidth] = useState(getStoredWidth);
	const [isResizing, setIsResizing] = useState(false);
	const sidebarRef = useRef<HTMLDivElement>(null);

	// Compute direct dependents and dependencies for the selected node
	const { dependents, dependencies } = useMemo(() => {
		if (!selectedNode) return { dependents: [], dependencies: [] };

		const nodeId = selectedNode.id;
		// Dependents: files that import this file (edges where this file is the target)
		const dependents = edges.filter((e) => e.to === nodeId).map((e) => e.from);
		// Dependencies: files this file imports (edges where this file is the source)
		const dependencies = edges.filter((e) => e.from === nodeId).map((e) => e.to);

		return { dependents, dependencies };
	}, [selectedNode, edges]);

	const handleMouseDown = useCallback((e: React.MouseEvent) => {
		e.preventDefault();
		setIsResizing(true);
	}, []);

	useEffect(() => {
		if (!isResizing) return;

		const handleMouseMove = (e: MouseEvent) => {
			if (!sidebarRef.current) return;

			// Calculate new width from right edge of window
			const newWidth = window.innerWidth - e.clientX;
			const clampedWidth = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, newWidth));
			setWidth(clampedWidth);
		};

		const handleMouseUp = () => {
			setIsResizing(false);
			// Save width to localStorage
			localStorage.setItem(STORAGE_KEY, width.toString());
		};

		document.addEventListener("mousemove", handleMouseMove);
		document.addEventListener("mouseup", handleMouseUp);

		// Add cursor style to body while resizing
		document.body.style.cursor = "col-resize";
		document.body.style.userSelect = "none";

		return () => {
			document.removeEventListener("mousemove", handleMouseMove);
			document.removeEventListener("mouseup", handleMouseUp);
			document.body.style.cursor = "";
			document.body.style.userSelect = "";
		};
	}, [isResizing, width]);

	return (
		<div className="sidebar" ref={sidebarRef} style={{ width }}>
			<div className="sidebar-resize-handle" onMouseDown={handleMouseDown} />
			<div className="sidebar-header">
				<h2>sass-dep</h2>
				<span className="subtitle">Dependency Graph</span>
			</div>

			<div className="sidebar-content">
				{selectedNode ? (
					<NodeDetails
						nodeId={selectedNode.id}
						node={selectedNode.node}
						dependents={dependents}
						dependencies={dependencies}
						edges={edges}
						onFocusNode={onFocusNode}
					/>
				) : selectedEdge ? (
					<EdgeDetails edge={selectedEdge} />
				) : (
					<div className="sidebar-section">
						<h3>Statistics</h3>
						<dl className="stats-list">
							<div className="stat-item">
								<dt>Total Files</dt>
								<dd>{statistics.total_files}</dd>
							</div>
							<div className="stat-item">
								<dt>Dependencies</dt>
								<dd>{statistics.total_dependencies}</dd>
							</div>
							<div className="stat-item">
								<dt>Entry Points</dt>
								<dd>{statistics.entry_points}</dd>
							</div>
							<div className="stat-item">
								<dt>Orphan Files</dt>
								<dd>{statistics.orphan_files}</dd>
							</div>
							<div className="stat-item">
								<dt>Leaf Files</dt>
								<dd>{statistics.leaf_files}</dd>
							</div>
							<div className="stat-item">
								<dt>Max Depth</dt>
								<dd>{statistics.max_depth}</dd>
							</div>
							<div className="stat-item">
								<dt>Max Fan-In</dt>
								<dd>{statistics.max_fan_in}</dd>
							</div>
							<div className="stat-item">
								<dt>Max Fan-Out</dt>
								<dd>{statistics.max_fan_out}</dd>
							</div>
						</dl>
						<p className="sidebar-hint">Click on a node or edge to see details</p>
					</div>
				)}
			</div>
		</div>
	);
}
