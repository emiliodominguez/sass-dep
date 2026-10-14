import { startTransition, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";

import {
	Background,
	Controls,
	type EdgeMouseHandler,
	MiniMap,
	type NodeMouseHandler,
	type NodeTypes,
	ReactFlow,
	ReactFlowProvider,
	type ReactFlowState,
	useEdgesState,
	useNodesState,
	useReactFlow,
	useStore,
} from "@xyflow/react";
import { toPng, toSvg } from "html-to-image";
import "@xyflow/react/dist/style.css";

import FileNode from "./FileNode";
import GroupNode from "./GroupNode";
import { NODE_HEIGHT, NODE_WIDTH } from "./layout";
import {
	type AppNode,
	type DependencyEdge,
	type EdgeDecoration,
	type NodeDecoration,
	buildEdges,
	buildFileNodes,
	decorateEdge,
	decorateNode,
	getVisibleGroupIds,
} from "./utils";
import { useGraphLayout } from "./useGraphLayout";
import { useTheme } from "../../contexts/useTheme";
import type { GraphIndex } from "../../utils/graphIndex";
import type { PathHighlightResult } from "../../hooks/usePathHighlight";
import type { OutputEdge, OutputNode, SassDepOutput } from "../../types/sass-dep";
import styles from "./Graph.module.scss";

/** Node count above which rendering is tuned for scale (viewport culling, no edge animation). */
export const LARGE_GRAPH_THRESHOLD = 1500;

/** Custom node type registry for React Flow. */
const nodeTypes: NodeTypes = {
	fileNode: FileNode,
	group: GroupNode,
};

/** Zoom level below which large graphs switch to a cheaper, detail-free node rendering. */
const DETAIL_ZOOM_THRESHOLD = 0.35;

/**
 * React Flow store selector reporting whether the viewport is zoomed out past the detail threshold.
 * Returns a boolean so subscribers only re-render when the threshold is crossed.
 * @param state - React Flow store state
 * @returns True when zoomed out beyond DETAIL_ZOOM_THRESHOLD
 */
function selectIsZoomedOut(state: ReactFlowState): boolean {
	return state.transform[2] < DETAIL_ZOOM_THRESHOLD;
}

/** Shared empty set used when a highlight is disabled. */
const EMPTY_SET: ReadonlySet<string> = new Set<string>();

/** Imperative handle for controlling the Graph component. */
export interface GraphHandle {
	/** Centers and highlights a node */
	focusNode: (nodeId: string) => void;
	/** Clears the focus highlight set by `focusNode` */
	clearFocus: () => void;
	/** Downloads the visible nodes as a PNG image */
	exportPng: () => Promise<void>;
	/** Downloads the visible nodes as an SVG image */
	exportSvg: () => Promise<void>;
	/** Downloads the visible subgraph as JSON */
	exportJson: () => void;
	/** Fits the viewport to the visible nodes */
	fitView: () => void;
}

/** Props for the Graph component. */
interface GraphProps {
	/** Imperative handle ref */
	ref?: React.Ref<GraphHandle>;
	/** Normalized sass-dep output */
	data: SassDepOutput;
	/** Adjacency index for `data` */
	index: GraphIndex;
	/** File IDs that pass the current search and filters */
	visibleIds: Set<string>;
	/** First shift-clicked node */
	pathSource: string | null;
	/** Second shift-clicked node */
	pathTarget: string | null;
	/** Path between the shift-clicked nodes */
	pathHighlight: PathHighlightResult;
	/** Whether cycle members are highlighted */
	highlightCycles: boolean;
	/** Whether files using legacy `@import` are highlighted */
	highlightImports: boolean;
	/** Whether files are grouped by folder */
	groupByFolder: boolean;
	/** Called when a file node is clicked */
	onNodeSelect?: (nodeId: string, node: OutputNode, isShiftClick: boolean) => void;
	/** Called when an edge is clicked */
	onEdgeSelect?: (edge: OutputEdge) => void;
	/** Called when the empty canvas is clicked */
	onClearSelection?: () => void;
}

/**
 * Checks whether two sets contain the same elements.
 * @param a - First set
 * @param b - Second set
 * @returns True when both sets have identical contents
 */
function setsEqual(a: ReadonlySet<string>, b: ReadonlySet<string>): boolean {
	if (a === b) return true;
	if (a.size !== b.size) return false;
	for (const value of a) {
		if (!b.has(value)) return false;
	}
	return true;
}

/**
 * Resolves after two animation frames, giving React and React Flow time to render pending changes.
 * @returns Promise resolving after the frames
 */
function waitForFrames(): Promise<void> {
	return new Promise(
		/**
		 * Resolves on the second animation frame.
		 * @param resolve - Promise resolver
		 */
		function waitTwoFrames(resolve) {
			requestAnimationFrame(function afterFirstFrame() {
				requestAnimationFrame(function afterSecondFrame() {
					resolve();
				});
			});
		},
	);
}

/**
 * Triggers a browser download.
 * @param href - URL (data or object URL) to download
 * @param fileName - Suggested file name
 */
function downloadUrl(href: string, fileName: string): void {
	const link = document.createElement("a");
	link.download = fileName;
	link.href = href;
	link.click();
}

/**
 * Returns today's date as YYYY-MM-DD for export file names.
 * @returns Date stamp
 */
function dateStamp(): string {
	return new Date().toISOString().slice(0, 10);
}

/**
 * Internal graph component that renders the React Flow visualization.
 *
 * Structure (layout) and decoration (visibility/highlight) are separated: the dagre layout only re-runs
 * (in a worker) when the data or grouping changes, while decoration changes only replace the node and
 * edge objects whose state actually changed, keeping referential identity for the rest.
 * @param props - Component props including data, visibility, highlight state and callbacks
 * @returns React Flow graph with nodes and edges
 */
function GraphInner({
	ref,
	data,
	index,
	visibleIds,
	pathSource,
	pathTarget,
	pathHighlight,
	highlightCycles,
	highlightImports,
	groupByFolder,
	onNodeSelect,
	onEdgeSelect,
	onClearSelection,
}: GraphProps) {
	const { setCenter, fitView, getInternalNode, getNodesBounds } = useReactFlow<AppNode, DependencyEdge>();
	const { theme } = useTheme();
	const wrapperRef = useRef<HTMLDivElement>(null);
	const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null);
	const [isCapturing, setIsCapturing] = useState(false);

	// Structure: rebuilt only when the dataset changes.
	const fileNodes = useMemo(() => buildFileNodes(data), [data]);
	const isLargeGraph = fileNodes.length > LARGE_GRAPH_THRESHOLD;
	const baseEdges = useMemo(() => buildEdges(data, index, isLargeGraph), [data, index, isLargeGraph]);
	// Animated viewport changes re-cull and re-mount nodes on every frame, which is costly on large graphs.
	const viewportAnimationScale = isLargeGraph ? 0 : 1;
	const isZoomedOut = useStore(selectIsZoomedOut);

	// Layout: re-run (off the main thread) only when the dataset or grouping changes.
	const { layoutNodes, isPending, error: layoutError } = useGraphLayout(data, groupByFolder, fileNodes);

	const [nodes, setNodes, onNodesChange] = useNodesState<AppNode>([]);
	const [edges, setEdges, onEdgesChange] = useEdgesState<DependencyEdge>([]);

	const visibleGroupIds = useMemo(() => (groupByFolder ? getVisibleGroupIds(visibleIds) : EMPTY_SET), [groupByFolder, visibleIds]);

	const nodeDecoration = useMemo<NodeDecoration>(
		() => ({
			visibleIds,
			visibleGroupIds,
			focusedNodeId,
			pathSource,
			pathTarget,
			pathNodeIds: pathHighlight.pathNodeIds,
			cycleNodeIds: highlightCycles ? index.cycleNodeIds : null,
			importerIds: highlightImports ? index.importerIds : null,
		}),
		[visibleIds, visibleGroupIds, focusedNodeId, pathSource, pathTarget, pathHighlight, highlightCycles, highlightImports, index],
	);

	const edgeDecoration = useMemo<EdgeDecoration>(
		() => ({
			visibleIds,
			pathEdgeKeys: pathHighlight.pathEdgeKeys,
			cycleEdgeKeys: highlightCycles ? index.cycleEdgeKeys : null,
			highlightImports,
			isLargeGraph,
		}),
		[visibleIds, pathHighlight, highlightCycles, highlightImports, index, isLargeGraph],
	);

	const appliedLayoutRef = useRef<AppNode[] | null>(null);
	const appliedVisibleIdsRef = useRef<ReadonlySet<string> | null>(null);
	const appliedEdgesRef = useRef<DependencyEdge[] | null>(null);
	const pendingFitRef = useRef(false);

	// Apply decoration to nodes. A new layout replaces every node; otherwise only changed nodes are replaced.
	useEffect(() => {
		const layoutChanged = appliedLayoutRef.current !== layoutNodes;
		const previousVisibleIds = appliedVisibleIdsRef.current;
		appliedLayoutRef.current = layoutNodes;
		appliedVisibleIdsRef.current = visibleIds;

		// Auto-fit only when the visible SET changes or a new layout arrives - never on highlight changes.
		if ((layoutChanged && layoutNodes.length > 0) || (previousVisibleIds !== null && !setsEqual(previousVisibleIds, visibleIds))) {
			pendingFitRef.current = true;
		}

		startTransition(
			/** Applies node decoration as a non-blocking transition. */
			function applyNodeDecoration() {
				setNodes((current) => (layoutChanged ? layoutNodes : current).map((node) => decorateNode(node, nodeDecoration)));
			},
		);
	}, [layoutNodes, visibleIds, nodeDecoration, setNodes]);

	// Apply decoration to edges (only once nodes exist, so React Flow never sees dangling edges).
	const hasLayout = layoutNodes.length > 0;
	useEffect(() => {
		if (!hasLayout) return;
		const baseChanged = appliedEdgesRef.current !== baseEdges;
		appliedEdgesRef.current = baseEdges;

		startTransition(
			/** Applies edge decoration as a non-blocking transition. */
			function applyEdgeDecoration() {
				setEdges((current) => (baseChanged ? baseEdges : current).map((edge) => decorateEdge(edge, edgeDecoration)));
			},
		);
	}, [hasLayout, baseEdges, edgeDecoration, setEdges]);

	// Run a queued auto-fit once the decorated nodes have been committed.
	useEffect(() => {
		if (!pendingFitRef.current || nodes.length === 0) return;
		pendingFitRef.current = false;
		fitView({ padding: 0.1, duration: 300 * viewportAnimationScale });
	}, [nodes, fitView, viewportAnimationScale]);

	/**
	 * Returns the IDs of all visible nodes (including visible folder groups).
	 * @returns Visible node IDs
	 */
	function getVisibleNodeIds(): string[] {
		return [...visibleGroupIds, ...visibleIds];
	}

	/**
	 * Renders the visible part of the graph to an image data URL.
	 * Viewport culling is temporarily disabled so off-screen nodes are included.
	 * @param format - Image format
	 * @returns Data URL, or null when nothing is visible
	 */
	async function captureImage(format: "png" | "svg"): Promise<string | null> {
		const viewport = wrapperRef.current?.querySelector<HTMLElement>(".react-flow__viewport");
		const visibleNodeIds = getVisibleNodeIds();
		if (!viewport || visibleNodeIds.length === 0) return null;

		setIsCapturing(true);
		try {
			await waitForFrames();

			const bounds = getNodesBounds(visibleNodeIds);
			const padding = 20;
			const width = bounds.width + padding * 2;
			const height = bounds.height + padding * 2;
			// HD output for PNG (2x for retina); SVG is resolution independent.
			const scale = format === "png" ? 2 : 1;
			const options = {
				backgroundColor: getComputedStyle(document.documentElement).getPropertyValue("--color-graph-bg").trim() || "#f8fafc",
				width: width * scale,
				height: height * scale,
				style: {
					width: `${width}px`,
					height: `${height}px`,
					transform: `translate(${-bounds.x + padding}px, ${-bounds.y + padding}px) scale(1)`,
				},
			};

			return format === "png" ? await toPng(viewport, options) : await toSvg(viewport, options);
		} finally {
			setIsCapturing(false);
		}
	}

	/** Exports the visible graph as a PNG image. */
	async function exportPng(): Promise<void> {
		const dataUrl = await captureImage("png");
		if (dataUrl) downloadUrl(dataUrl, `sass-dep-graph-${dateStamp()}.png`);
	}

	/** Exports the visible graph as an SVG image. */
	async function exportSvg(): Promise<void> {
		const dataUrl = await captureImage("svg");
		if (dataUrl) downloadUrl(dataUrl, `sass-dep-graph-${dateStamp()}.svg`);
	}

	/** Exports the visible subgraph as a sass-dep JSON document. */
	function exportJson(): void {
		const visibleNodes: Record<string, OutputNode> = {};
		for (const [id, node] of Object.entries(data.nodes)) {
			if (visibleIds.has(id)) visibleNodes[id] = node;
		}

		const visibleEdges = data.edges.filter((edge) => visibleIds.has(edge.from) && visibleIds.has(edge.to));
		const cycles = data.analysis.cycles.filter((cycle) => cycle.every((nodeId) => visibleIds.has(nodeId)));
		const unresolved = data.analysis.unresolved.filter((entry) => visibleIds.has(entry.from));

		const subgraph = {
			$schema: data.$schema,
			version: data.version,
			metadata: {
				...data.metadata,
				exported_at: new Date().toISOString(),
				export_type: "subgraph",
				total_nodes: visibleIds.size,
				total_edges: visibleEdges.length,
			},
			nodes: visibleNodes,
			edges: visibleEdges,
			analysis: {
				cycles,
				unresolved,
				statistics: {
					...data.analysis.statistics,
					total_files: visibleIds.size,
					total_dependencies: visibleEdges.length,
					cycle_count: cycles.length,
					unresolved_imports: unresolved.length,
					import_directives: visibleEdges.filter((edge) => edge.directive_type === "import").length,
				},
			},
		};

		const blob = new Blob([JSON.stringify(subgraph, null, 2)], { type: "application/json" });
		const url = URL.createObjectURL(blob);
		downloadUrl(url, `sass-dep-subgraph-${dateStamp()}.json`);
		URL.revokeObjectURL(url);
	}

	/** Fits the viewport to the visible nodes (hidden nodes are excluded by React Flow). */
	function handleFitView(): void {
		fitView({ padding: 0.1, duration: 300 * viewportAnimationScale });
	}

	/**
	 * Centers the viewport on a node and highlights it, using React Flow's absolute position and measured size.
	 * @param nodeId - Node to focus
	 */
	function focusNode(nodeId: string): void {
		const internalNode = getInternalNode(nodeId);
		if (!internalNode) return;

		setFocusedNodeId(nodeId);

		const { x, y } = internalNode.internals.positionAbsolute;
		const width = internalNode.measured.width ?? NODE_WIDTH;
		const height = internalNode.measured.height ?? NODE_HEIGHT;
		setCenter(x + width / 2, y + height / 2, { zoom: 1.5, duration: 500 * viewportAnimationScale });
	}

	/** Clears the focus highlight. */
	function clearFocus(): void {
		setFocusedNodeId(null);
	}

	/**
	 * Selects the clicked file node (shift-click picks path endpoints).
	 * @param event - Mouse event
	 * @param node - Clicked node
	 */
	function handleNodeClick(event: React.MouseEvent, node: AppNode): void {
		const nodeData = data.nodes[node.id];
		if (nodeData && onNodeSelect) {
			onNodeSelect(node.id, nodeData, event.shiftKey);
		}
	}

	/**
	 * Selects the clicked edge, looked up in O(1) by its stable ID.
	 * @param _event - Mouse event (unused)
	 * @param edge - Clicked edge
	 */
	function handleEdgeClick(_event: React.MouseEvent, edge: DependencyEdge): void {
		const edgeData = index.edgeByKey.get(edge.id);
		if (edgeData && onEdgeSelect) {
			onEdgeSelect(edgeData);
		}
	}

	/** Clears selection and focus when the empty canvas is clicked. */
	function handlePaneClick(): void {
		setFocusedNodeId(null);
		onClearSelection?.();
	}

	useImperativeHandle(ref, () => ({ focusNode, clearFocus, exportPng, exportSvg, exportJson, fitView: handleFitView }));

	const wrapperClassName = [styles["graph-wrapper"], isLargeGraph && styles["large"], isLargeGraph && isZoomedOut && !isCapturing && styles["zoomed-out"]]
		.filter(Boolean)
		.join(" ");
	const onNodeClick: NodeMouseHandler<AppNode> = handleNodeClick;
	const onEdgeClick: EdgeMouseHandler<DependencyEdge> = handleEdgeClick;

	return (
		<div className={wrapperClassName} ref={wrapperRef}>
			<ReactFlow<AppNode, DependencyEdge>
				nodes={nodes}
				edges={edges}
				onNodesChange={onNodesChange}
				onEdgesChange={onEdgesChange}
				onNodeClick={onNodeClick}
				onEdgeClick={onEdgeClick}
				onPaneClick={handlePaneClick}
				nodeTypes={nodeTypes}
				colorMode={theme}
				onlyRenderVisibleElements={isLargeGraph && !isCapturing}
				minZoom={0.05}
				maxZoom={2}
			>
				<Background color="var(--color-graph-dots)" gap={20} />
				<Controls showInteractive={false} />
				<MiniMap pannable zoomable />
			</ReactFlow>

			{(isPending || layoutError) && (
				<div className={styles["layout-overlay"]} role="status" aria-live="polite">
					{layoutError ? (
						<span>Layout failed: {layoutError}</span>
					) : (
						<>
							<span className={styles["spinner"]} />
							<span>Laying out {fileNodes.length.toLocaleString()} files…</span>
						</>
					)}
				</div>
			)}
		</div>
	);
}

/**
 * Graph component wrapper that provides React Flow context.
 * @param props - Component props passed to GraphInner
 * @returns Graph wrapped in ReactFlowProvider
 */
export function Graph(props: GraphProps) {
	return (
		<ReactFlowProvider>
			<GraphInner {...props} />
		</ReactFlowProvider>
	);
}
