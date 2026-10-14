import { useEffect, useRef, useState } from "react";

import { EdgeDetails } from "./EdgeDetails";
import { NodeDetails } from "./NodeDetails";
import { Overview } from "./Overview";
import { PathDetails } from "./PathDetails";
import type { GraphIndex } from "../../utils/graphIndex";
import type { PathHighlightResult } from "../../hooks/usePathHighlight";
import type { OutputEdge, OutputNode, SassDepOutput } from "../../types/sass-dep";
import styles from "./Sidebar.module.scss";

/** Minimum sidebar width in pixels. */
const MIN_WIDTH = 280;
/** Maximum sidebar width in pixels. */
const MAX_WIDTH = 600;
/** Sidebar width used when nothing is stored. */
const DEFAULT_WIDTH = 320;
/** localStorage key for the persisted sidebar width. */
const STORAGE_KEY = "sass-dep-sidebar-width";

/**
 * Retrieves stored sidebar width from localStorage.
 * @returns The stored width or default value
 */
function getStoredWidth(): number {
	if (typeof window === "undefined") return DEFAULT_WIDTH;
	try {
		const stored = localStorage.getItem(STORAGE_KEY);
		if (stored) {
			const width = parseInt(stored, 10);
			if (!isNaN(width) && width >= MIN_WIDTH && width <= MAX_WIDTH) {
				return width;
			}
		}
	} catch {
		// Storage unavailable (e.g. privacy mode) - fall back to the default width
	}
	return DEFAULT_WIDTH;
}

/** Props for the Sidebar component. */
interface SidebarProps {
	/** Normalized sass-dep output */
	data: SassDepOutput;
	/** Graph adjacency index */
	index: GraphIndex;
	/** Selected file, if any */
	selectedNode: { id: string; node: OutputNode } | null;
	/** Selected edge, if any */
	selectedEdge: OutputEdge | null;
	/** First shift-clicked file */
	pathSource: string | null;
	/** Second shift-clicked file */
	pathTarget: string | null;
	/** Result of the path search */
	pathHighlight: PathHighlightResult;
	/** Called when a file should be focused in the graph */
	onFocusNode?: (nodeId: string) => void;
	/** Clears the path endpoints */
	onClearPath: () => void;
}

/**
 * Sidebar component displaying the dependency path, node details, edge details, or the project overview.
 * @param props - Component props
 * @returns Resizable sidebar panel
 */
export function Sidebar({ data, index, selectedNode, selectedEdge, pathSource, pathTarget, pathHighlight, onFocusNode, onClearPath }: SidebarProps) {
	const [width, setWidth] = useState(getStoredWidth);
	const [isResizing, setIsResizing] = useState(false);
	const sidebarRef = useRef<HTMLDivElement>(null);

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
			try {
				localStorage.setItem(STORAGE_KEY, width.toString());
			} catch {
				// Storage unavailable - the width just won't persist
			}
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

	let details: React.ReactNode;
	if (selectedNode) {
		details = (
			<NodeDetails
				key={selectedNode.id}
				nodeId={selectedNode.id}
				node={selectedNode.node}
				index={index}
				totalFiles={data.analysis.statistics.total_files}
				onFocusNode={onFocusNode}
			/>
		);
	} else if (selectedEdge) {
		details = <EdgeDetails edge={selectedEdge} />;
	} else {
		details = <Overview data={data} onFocusNode={onFocusNode} />;
	}

	return (
		<div className={styles["sidebar"]} ref={sidebarRef} style={{ width }}>
			<div className={styles["resize-handle"]} onMouseDown={handleMouseDown} />

			<div className={styles["header"]}>
				<h2>sass-dep</h2>
				<span className={styles["subtitle"]}>Dependency Graph</span>
			</div>

			<div className={styles["content"]}>
				{pathSource && (
					<PathDetails
						key={`${pathSource}->${pathTarget ?? ""}`}
						pathSource={pathSource}
						pathTarget={pathTarget}
						pathHighlight={pathHighlight}
						index={index}
						onFocusNode={onFocusNode}
						onClear={onClearPath}
					/>
				)}
				{details}
			</div>
		</div>
	);
}
