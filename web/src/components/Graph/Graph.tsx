import { useCallback, useMemo, forwardRef, useImperativeHandle, useState, useEffect, useRef } from "react";
import { ReactFlow, Background, Controls, MiniMap, useNodesState, useEdgesState, useReactFlow, ReactFlowProvider, getNodesBounds } from "@xyflow/react";
import { toPng } from "html-to-image";
import "@xyflow/react/dist/style.css";

import FileNode from "./FileNode";
import { transformToFlowElements, applyDagreLayout } from "./utils";
import { getNodeColor } from "./styles";
import { usePathHighlight } from "../../hooks/usePathHighlight";
import type { SassDepOutput, OutputNode, OutputEdge } from "../../types/sass-dep";
import "./Graph.css";

const nodeTypes = {
	fileNode: FileNode,
};

export interface GraphHandle {
	focusNode: (nodeId: string) => void;
	exportPng: () => Promise<void>;
	fitView: () => void;
}

interface GraphProps {
	data: SassDepOutput;
	searchQuery: string;
	activeFilters: string[];
	pathSource: string | null;
	pathTarget: string | null;
	onNodeSelect?: (nodeId: string, node: OutputNode, isShiftClick?: boolean) => void;
	onEdgeSelect?: (edge: OutputEdge) => void;
	onClearSelection?: () => void;
}

function GraphInner({ data, searchQuery, activeFilters, pathSource, pathTarget, onNodeSelect, onEdgeSelect, onClearSelection }: GraphProps, ref: React.Ref<GraphHandle>) {
	const { setCenter, getNode, fitView } = useReactFlow();
	const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null);

	// Path highlighting
	const { pathNodeIds, pathEdgeKeys, hasPath } = usePathHighlight(pathSource, pathTarget, data.edges);

	// Transform and layout nodes
	const { initialNodes, initialEdges } = useMemo(() => {
		const { nodes, edges } = transformToFlowElements(data);
		const layoutedNodes = applyDagreLayout(nodes, edges);
		return { initialNodes: layoutedNodes, initialEdges: edges };
	}, [data]);

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const [nodes, , onNodesChange] = useNodesState(initialNodes as any);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const [edges, , onEdgesChange] = useEdgesState(initialEdges as any);

	// Filter nodes based on search and filters
	const filteredNodes = useMemo(() => {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		return nodes.map((node: any) => {
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

			const isVisible = matchesSearch && matchesFilters;
			const isFocused = node.id === focusedNodeId;
			const isPathSource = node.id === pathSource;
			const isPathTarget = node.id === pathTarget;
			const isInPath = hasPath && pathNodeIds.has(node.id);

			return {
				...node,
				hidden: !isVisible,
				data: {
					...node.data,
					isFocused,
					isPathSource,
					isPathTarget,
					isInPath,
				},
			};
		});
	}, [nodes, searchQuery, activeFilters, focusedNodeId, pathSource, pathTarget, hasPath, pathNodeIds]);

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

			return {
				...edge,
				hidden: !visibleNodeIds.has(edge.source) || !visibleNodeIds.has(edge.target),
				style: isInPath
					? { stroke: "var(--color-accent)", strokeWidth: 3 }
					: edge.style,
				animated: isInPath,
			};
		});
	}, [edges, filteredNodes, hasPath, pathEdgeKeys]);

	// Track if this is the initial render
	const isInitialRender = useRef(true);

	// Auto-fit view when filters or search change
	useEffect(() => {
		// Skip the initial render (ReactFlow's fitView prop handles that)
		if (isInitialRender.current) {
			isInitialRender.current = false;
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
				// Use requestAnimationFrame to ensure React Flow has updated
				requestAnimationFrame(() => {
					const node = getNode(nodeId);
					if (node) {
						// Center the view on the node
						const x = node.position.x + (node.measured?.width ?? 180) / 2;
						const y = node.position.y + (node.measured?.height ?? 60) / 2;
						setCenter(x, y, { zoom: 1.5, duration: 500 });

						// Set focused state for visual highlight
						setFocusedNodeId(nodeId);

						// Clear focus after 3 seconds
						setTimeout(() => setFocusedNodeId(null), 3000);
					} else {
						// Retry once after a short delay if node not found
						setTimeout(() => {
							const retryNode = getNode(nodeId);
							if (retryNode) {
								const x = retryNode.position.x + (retryNode.measured?.width ?? 180) / 2;
								const y = retryNode.position.y + (retryNode.measured?.height ?? 60) / 2;
								setCenter(x, y, { zoom: 1.5, duration: 500 });
								setFocusedNodeId(nodeId);
								setTimeout(() => setFocusedNodeId(null), 3000);
							}
						}, 100);
					}
				});
			},
			exportPng,
			fitView: handleFitView,
		}),
		[getNode, setCenter, exportPng, handleFitView],
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
