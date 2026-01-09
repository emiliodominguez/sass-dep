import { useEffect, useMemo, useRef, useState } from "react";

import type { OutputEdge, OutputNode, Statistics } from "../../types/sass-dep";
import { NodeDetails } from "./NodeDetails";
import { EdgeDetails } from "./EdgeDetails";
import styles from "./Sidebar.module.scss";

const MIN_WIDTH = 280;
const MAX_WIDTH = 600;
const DEFAULT_WIDTH = 320;
const STORAGE_KEY = "sass-dep-sidebar-width";

/**
 * Retrieves stored sidebar width from localStorage.
 * @returns The stored width or default value
 */
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
	selectedNode: { id: string; node: OutputNode } | null;
	selectedEdge: OutputEdge | null;
	statistics: Statistics;
	edges: OutputEdge[];
	onFocusNode?: (nodeId: string) => void;
}

/**
 * Sidebar component displaying node details, edge details, or statistics.
 * @param props - Component props
 * @returns Resizable sidebar panel
 */
export function Sidebar({ selectedNode, selectedEdge, statistics, edges, onFocusNode }: SidebarProps) {
	const [width, setWidth] = useState(getStoredWidth);
	const [isResizing, setIsResizing] = useState(false);
	const sidebarRef = useRef<HTMLDivElement>(null);

	const { dependents, dependencies } = useMemo(() => {
		if (!selectedNode) return { dependents: [], dependencies: [] };

		const nodeId = selectedNode.id;
		const nodeDependents = edges.filter((e) => e.to === nodeId).map((e) => e.from);
		const nodeDependencies = edges.filter((e) => e.from === nodeId).map((e) => e.to);

		return { dependents: nodeDependents, dependencies: nodeDependencies };
	}, [selectedNode, edges]);

	/**
	 * Handles mouse down event to initiate resizing
	 * @param e - Mouse event
	 */
	function handleMouseDown(e: React.MouseEvent) {
		e.preventDefault();
		setIsResizing(true);
	}

	useEffect(() => {
		if (!isResizing) return;

		/**
		 * Handles mouse move events to resize the sidebar.
		 * @param e - Mouse event
		 */
		function handleMouseMove(e: MouseEvent) {
			if (!sidebarRef.current) return;
			const newWidth = window.innerWidth - e.clientX;
			const clampedWidth = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, newWidth));
			setWidth(clampedWidth);
		}

		/** Handles mouse up events to stop resizing and persist width. */
		function handleMouseUp() {
			setIsResizing(false);
			localStorage.setItem(STORAGE_KEY, width.toString());
		}

		document.addEventListener("mousemove", handleMouseMove);
		document.addEventListener("mouseup", handleMouseUp);
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
		<div className={styles["sidebar"]} ref={sidebarRef} style={{ width }}>
			<div className={styles["resize-handle"]} onMouseDown={handleMouseDown} />

			<div className={styles["header"]}>
				<h2>sass-dep</h2>
				<span className={styles["subtitle"]}>Dependency Graph</span>
			</div>

			<div className={styles["content"]}>
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
					<div className={styles["section"]}>
						<h3>Statistics</h3>
						<dl className={styles["stats-list"]}>
							<div className={styles["stat-item"]}>
								<dt>Total Files</dt>
								<dd>{statistics.total_files}</dd>
							</div>

							<div className={styles["stat-item"]}>
								<dt>Dependencies</dt>
								<dd>{statistics.total_dependencies}</dd>
							</div>

							<div className={styles["stat-item"]}>
								<dt>Entry Points</dt>
								<dd>{statistics.entry_points}</dd>
							</div>

							<div className={styles["stat-item"]}>
								<dt>Orphan Files</dt>
								<dd>{statistics.orphan_files}</dd>
							</div>

							<div className={styles["stat-item"]}>
								<dt>Leaf Files</dt>
								<dd>{statistics.leaf_files}</dd>
							</div>

							<div className={styles["stat-item"]}>
								<dt>Max Depth</dt>
								<dd>{statistics.max_depth}</dd>
							</div>

							<div className={styles["stat-item"]}>
								<dt>Max Fan-In</dt>
								<dd>{statistics.max_fan_in}</dd>
							</div>

							<div className={styles["stat-item"]}>
								<dt>Max Fan-Out</dt>
								<dd>{statistics.max_fan_out}</dd>
							</div>
						</dl>
						<p className={styles["hint"]}>Click on a node or edge to see details</p>
					</div>
				)}
			</div>
		</div>
	);
}
