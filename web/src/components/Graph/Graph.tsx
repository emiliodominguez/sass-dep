import { useCallback, useMemo, forwardRef, useImperativeHandle, useState, useEffect, useRef } from "react";
import { ReactFlow, Background, Controls, MiniMap, useNodesState, useEdgesState, useReactFlow, ReactFlowProvider, getNodesBounds } from "@xyflow/react";
import { toPng, toSvg } from "html-to-image";
import "@xyflow/react/dist/style.css";

import FileNode from "./FileNode";
import GroupNode from "./GroupNode";
import { transformToFlowElements, applyDagreLayout, transformToGroupedFlowElements, applyGroupedDagreLayout } from "./utils";
import { getNodeColor } from "./styles";
import { usePathHighlight } from "../../hooks/usePathHighlight";
import type { SassDepOutput, OutputNode, OutputEdge } from "../../types/sass-dep";
import type { AdvancedFilters } from "../Toolbar/Toolbar";
import "./Graph.css";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const nodeTypes: any = {
	fileNode: FileNode,
	group: GroupNode,
};

export interface GraphHandle {
	focusNode: (nodeId: string) => void;
	exportPng: () => Promise<void>;
	exportSvg: () => Promise<void>;
	exportJson: () => void;
	fitView: () => void;
}

interface GraphProps {
	// Data props
	data: SassDepOutput;
	searchQuery: string;
	activeFilters: string[];
	advancedFilters: AdvancedFilters;
	pathSource: string | null;
	pathTarget: string | null;
	highlightCycles?: boolean;
	groupByFolder?: boolean;
	// Callbacks
	onNodeSelect?: (nodeId: string, node: OutputNode, isShiftClick?: boolean) => void;
	onEdgeSelect?: (edge: OutputEdge) => void;
	onClearSelection?: () => void;
}

function GraphInner({ data, searchQuery, activeFilters, advancedFilters, pathSource, pathTarget, highlightCycles, groupByFolder, onNodeSelect, onEdgeSelect, onClearSelection }: GraphProps, ref: React.Ref<GraphHandle>) {
	const { setCenter, getNode, fitView } = useReactFlow();
	const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null);

	// Path highlighting
	const { pathNodeIds, pathEdgeKeys, hasPath } = usePathHighlight(pathSource, pathTarget, data.edges);

	// Cycle highlighting - build sets of node IDs and edge keys in cycles
	const { cycleNodeIds, cycleEdgeKeys } = useMemo(() => {
		const nodeIds = new Set<string>();
		const edgeKeys = new Set<string>();

		for (const cycle of data.analysis.cycles) {
			for (let i = 0; i < cycle.length; i++) {
				nodeIds.add(cycle[i]);
				// Create edge key for each consecutive pair in the cycle
				const nextIndex = (i + 1) % cycle.length;
				edgeKeys.add(`${cycle[i]}->${cycle[nextIndex]}`);
			}
		}

		return { cycleNodeIds: nodeIds, cycleEdgeKeys: edgeKeys };
	}, [data.analysis.cycles]);

	// Transform and layout nodes
	const { initialNodes, initialEdges } = useMemo(() => {
		if (groupByFolder) {
			const { nodes, edges } = transformToGroupedFlowElements(data);
			const layoutedNodes = applyGroupedDagreLayout(nodes, edges);
			return { initialNodes: layoutedNodes, initialEdges: edges };
		} else {
			const { nodes, edges } = transformToFlowElements(data);
			const layoutedNodes = applyDagreLayout(nodes, edges);
			return { initialNodes: layoutedNodes, initialEdges: edges };
		}
	}, [data, groupByFolder]);

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes as any);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges as any);

	// Update nodes when grouping changes
	useEffect(() => {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		setNodes(initialNodes as any);
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		setEdges(initialEdges as any);
	}, [initialNodes, initialEdges, setNodes, setEdges]);

	// Filter nodes based on search and filters
	const filteredNodes = useMemo(() => {
		const { minDepth, maxDepth, minFanIn, maxFanIn, minFanOut, maxFanOut } = advancedFilters;

		// Track which group nodes have visible children
		const visibleGroupIds = new Set<string>();

		// First pass: determine which file nodes are visible
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		const processedNodes = nodes.map((node: any) => {
			// Group nodes are handled separately
			if (node.type === "group") {
				return { node, isVisible: false }; // Will be updated later
			}

			const matchesSearch =
				!searchQuery || node.id.toLowerCase().includes(searchQuery.toLowerCase()) || node.data.fullPath?.toLowerCase().includes(searchQuery.toLowerCase());

			const matchesFilters =
				activeFilters.length === 0 ||
				activeFilters.some((filter) => {
					if (filter === "no_flags") {
						return !node.data.flags || node.data.flags.length === 0;
					}
					return node.data.flags?.includes(filter);
				});

			// Check advanced filters using node data metrics
			const depth = node.data.depth ?? 0;
			const fanIn = node.data.fanIn ?? 0;
			const fanOut = node.data.fanOut ?? 0;

			const matchesAdvanced =
				(minDepth === null || depth >= minDepth) &&
				(maxDepth === null || depth <= maxDepth) &&
				(minFanIn === null || fanIn >= minFanIn) &&
				(maxFanIn === null || fanIn <= maxFanIn) &&
				(minFanOut === null || fanOut >= minFanOut) &&
				(maxFanOut === null || fanOut <= maxFanOut);

			const isVisible = matchesSearch && matchesFilters && matchesAdvanced;

			// Track visible groups
			if (isVisible && node.parentId) {
				visibleGroupIds.add(node.parentId);
			}

			return { node, isVisible };
		});

		// Second pass: apply visibility and styling
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		return processedNodes.map(({ node, isVisible }: { node: any; isVisible: boolean }) => {
			// Group nodes are visible if they have visible children
			if (node.type === "group") {
				return {
					...node,
					hidden: !visibleGroupIds.has(node.id),
				};
			}

			const isFocused = node.id === focusedNodeId;
			const isPathSource = node.id === pathSource;
			const isPathTarget = node.id === pathTarget;
			const isInPath = hasPath && pathNodeIds.has(node.id);
			const isInCycle = highlightCycles && cycleNodeIds.has(node.id);

			return {
				...node,
				hidden: !isVisible,
				data: {
					...node.data,
					isFocused,
					isPathSource,
					isPathTarget,
					isInPath,
					isInCycle,
				},
			};
		});
	}, [nodes, searchQuery, activeFilters, advancedFilters, focusedNodeId, pathSource, pathTarget, hasPath, pathNodeIds, highlightCycles, cycleNodeIds]);

	// Filter edges to only show connections between visible nodes
	const filteredEdges = useMemo(() => {
		const visibleNodeIds = new Set(
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			filteredNodes.filter((n: any) => !n.hidden).map((n: any) => n.id),
		);

		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		return edges.map((edge: any) => {
			const edgeKey = `${edge.source}->${edge.target}`;
			const isInPath = hasPath && pathEdgeKeys.has(edgeKey);
			const isInCycle = highlightCycles && cycleEdgeKeys.has(edgeKey);

			// Path highlighting takes priority over cycle highlighting
			let style = edge.style;
			let animated = false;

			if (isInPath) {
				style = { stroke: "var(--color-accent)", strokeWidth: 3 };
				animated = true;
			} else if (isInCycle) {
				style = { stroke: "var(--color-flag-in-cycle)", strokeWidth: 3 };
				animated = true;
			}

			return {
				...edge,
				hidden: !visibleNodeIds.has(edge.source) || !visibleNodeIds.has(edge.target),
				style,
				animated,
			};
		});
	}, [edges, filteredNodes, hasPath, pathEdgeKeys, highlightCycles, cycleEdgeKeys]);

	// Track if this is the initial render
	const isInitialRender = useRef(true);
	// Track if we're currently focusing on a node (to skip auto-fit)
	const isFocusingRef = useRef(false);

	// Auto-fit view when filters or search change
	useEffect(() => {
		// Skip the initial render (ReactFlow's fitView prop handles that)
		if (isInitialRender.current) {
			isInitialRender.current = false;
			return;
		}

		// Skip if we're in the middle of focusing on a node
		if (isFocusingRef.current) {
			return;
		}

		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		const visibleNodes = filteredNodes.filter((n: any) => !n.hidden);
		if (visibleNodes.length > 0) {
			// Small delay to let React Flow update the nodes first
			const timeoutId = setTimeout(() => {
				fitView({ nodes: visibleNodes, padding: 0.1, duration: 300 });
			}, 50);
			return () => clearTimeout(timeoutId);
		}
	}, [searchQuery, activeFilters, filteredNodes, fitView]);

	// Export to PNG function
	const exportPng = useCallback(async () => {
		const viewport = document.querySelector(".react-flow__viewport") as HTMLElement;
		if (!viewport) return;

		// Get visible nodes only
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		const visibleNodes = filteredNodes.filter((n: any) => !n.hidden);
		if (visibleNodes.length === 0) return;

		// Get the bounds of all visible nodes
		const nodesBounds = getNodesBounds(visibleNodes);
		const padding = 20;

		// Calculate image dimensions with padding
		const imageWidth = nodesBounds.width + padding * 2;
		const imageHeight = nodesBounds.height + padding * 2;

		// Calculate transform to position content at origin with padding offset
		// We need to translate so the top-left of bounds is at (padding, padding)
		const translateX = -nodesBounds.x + padding;
		const translateY = -nodesBounds.y + padding;

		// Generate HD image (2x scale for retina)
		const scale = 2;
		const dataUrl = await toPng(viewport, {
			backgroundColor: getComputedStyle(document.documentElement).getPropertyValue("--color-graph-bg").trim() || "#f8fafc",
			width: imageWidth * scale,
			height: imageHeight * scale,
			style: {
				width: `${imageWidth}px`,
				height: `${imageHeight}px`,
				transform: `translate(${translateX}px, ${translateY}px) scale(1)`,
			},
		});

		// Create download link
		const link = document.createElement("a");
		link.download = `sass-dep-graph-${new Date().toISOString().slice(0, 10)}.png`;
		link.href = dataUrl;
		link.click();
	}, [filteredNodes]);

	// Export to SVG function
	const exportSvg = useCallback(async () => {
		const viewport = document.querySelector(".react-flow__viewport") as HTMLElement;
		if (!viewport) return;

		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		const visibleNodes = filteredNodes.filter((n: any) => !n.hidden);
		if (visibleNodes.length === 0) return;

		const nodesBounds = getNodesBounds(visibleNodes);
		const padding = 20;
		const imageWidth = nodesBounds.width + padding * 2;
		const imageHeight = nodesBounds.height + padding * 2;
		const translateX = -nodesBounds.x + padding;
		const translateY = -nodesBounds.y + padding;

		const svgDataUrl = await toSvg(viewport, {
			backgroundColor: getComputedStyle(document.documentElement).getPropertyValue("--color-graph-bg").trim() || "#f8fafc",
			width: imageWidth,
			height: imageHeight,
			style: {
				width: `${imageWidth}px`,
				height: `${imageHeight}px`,
				transform: `translate(${translateX}px, ${translateY}px) scale(1)`,
			},
		});

		const link = document.createElement("a");
		link.download = `sass-dep-graph-${new Date().toISOString().slice(0, 10)}.svg`;
		link.href = svgDataUrl;
		link.click();
	}, [filteredNodes]);

	// Export visible subgraph as JSON
	const exportJson = useCallback(() => {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		const visibleNodeIds = new Set(filteredNodes.filter((n: any) => !n.hidden).map((n: any) => n.id));

		// Filter nodes to only visible ones
		const visibleNodes: Record<string, OutputNode> = {};
		for (const [id, node] of Object.entries(data.nodes)) {
			if (visibleNodeIds.has(id)) {
				visibleNodes[id] = node;
			}
		}

		// Filter edges to only those between visible nodes
		const visibleEdges = data.edges.filter((edge) => visibleNodeIds.has(edge.from) && visibleNodeIds.has(edge.to));

		// Create subgraph export
		const subgraph = {
			$schema: data.$schema,
			version: data.version,
			metadata: {
				...data.metadata,
				exported_at: new Date().toISOString(),
				export_type: "subgraph",
				total_nodes: visibleNodeIds.size,
				total_edges: visibleEdges.length,
			},
			nodes: visibleNodes,
			edges: visibleEdges,
			analysis: {
				cycles: data.analysis.cycles.filter((cycle) => cycle.every((nodeId) => visibleNodeIds.has(nodeId))),
				statistics: {
					...data.analysis.statistics,
					total_files: visibleNodeIds.size,
					total_dependencies: visibleEdges.length,
				},
			},
		};

		const jsonString = JSON.stringify(subgraph, null, 2);
		const blob = new Blob([jsonString], { type: "application/json" });
		const url = URL.createObjectURL(blob);

		const link = document.createElement("a");
		link.download = `sass-dep-subgraph-${new Date().toISOString().slice(0, 10)}.json`;
		link.href = url;
		link.click();

		URL.revokeObjectURL(url);
	}, [filteredNodes, data]);

	// Fit view to visible nodes
	const handleFitView = useCallback(() => {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		const visibleNodes = filteredNodes.filter((n: any) => !n.hidden);
		if (visibleNodes.length === 0) return;

		fitView({ nodes: visibleNodes, padding: 0.1, duration: 300 });
	}, [filteredNodes, fitView]);

	// Expose focus, export, and fitView methods via ref
	useImperativeHandle(
		ref,
		() => ({
			focusNode: (nodeId: string) => {
				// Find the node from our nodes array (more reliable than getNode)
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				const node = nodes.find((n: any) => n.id === nodeId);
				if (!node) return;

				// Mark that we're focusing to prevent auto-fit from interfering
				isFocusingRef.current = true;

				// Set focused state to trigger visual highlight
				setFocusedNodeId(nodeId);

				// Use setTimeout to ensure React has processed the state update
				setTimeout(() => {
					// Get fresh node data from React Flow (may have measured dimensions now)
					const flowNode = getNode(nodeId);
					const width = flowNode?.measured?.width ?? 180;
					const height = flowNode?.measured?.height ?? 60;

					// Center on node position
					const x = node.position.x + width / 2;
					const y = node.position.y + height / 2;
					setCenter(x, y, { zoom: 1.5, duration: 500 });

					// Allow auto-fit again after animation completes
					setTimeout(() => {
						isFocusingRef.current = false;
					}, 600);
				}, 50);

				// Clear focus after 3 seconds
				setTimeout(() => setFocusedNodeId(null), 3000);
			},
			exportPng,
			exportSvg,
			exportJson,
			fitView: handleFitView,
		}),
		[nodes, getNode, setCenter, exportPng, exportSvg, exportJson, handleFitView],
	);

	const handleNodeClick = useCallback(
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		(event: React.MouseEvent, node: any) => {
			const nodeData = data.nodes[node.id];
			if (nodeData && onNodeSelect) {
				onNodeSelect(node.id, nodeData, event.shiftKey);
			}
		},
		[data.nodes, onNodeSelect],
	);

	const handleEdgeClick = useCallback(
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		(_event: React.MouseEvent, edge: any) => {
			const edgeData = data.edges.find((e) => e.from === edge.source && e.to === edge.target);
			if (edgeData && onEdgeSelect) {
				onEdgeSelect(edgeData);
			}
		},
		[data.edges, onEdgeSelect],
	);

	const handlePaneClick = useCallback(() => {
		if (onClearSelection) {
			onClearSelection();
		}
	}, [onClearSelection]);

	return (
		<div className="graph-wrapper">
			<ReactFlow
				nodes={filteredNodes}
				edges={filteredEdges}
				onNodesChange={onNodesChange}
				onEdgesChange={onEdgesChange}
				onNodeClick={handleNodeClick}
				onEdgeClick={handleEdgeClick}
				onPaneClick={handlePaneClick}
				nodeTypes={nodeTypes}
				fitView
				minZoom={0.1}
				maxZoom={2}
				defaultEdgeOptions={{
					type: "smoothstep",
				}}
			>
				<Background color="var(--color-graph-dots)" gap={20} />
				<Controls showInteractive={false} />
				<MiniMap
					// eslint-disable-next-line @typescript-eslint/no-explicit-any
					nodeColor={(node: any) => getNodeColor(node.data?.primaryFlag)}
					maskColor="var(--color-minimap-mask)"
					style={{ background: "var(--color-minimap-bg)" }}
				/>
			</ReactFlow>
		</div>
	);
}

const GraphWithRef = forwardRef(GraphInner);

// Wrap with ReactFlowProvider so we can use hooks
export const Graph = forwardRef<GraphHandle, GraphProps>((props, ref) => (
	<ReactFlowProvider>
		<GraphWithRef {...props} ref={ref} />
	</ReactFlowProvider>
));

Graph.displayName = "Graph";
