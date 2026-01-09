import type { Edge, Node, XYPosition } from "@xyflow/react";
import Dagre from "@dagrejs/dagre";

import type { NodeFlag, OutputNode, SassDepOutput } from "../../types/sass-dep";
import { getEdgeColor, getEdgeStyle, getNodeColor } from "./styles";

/** Group node data */
export interface GroupNodeData extends Record<string, unknown> {
	label: string;
	folderPath: string;
	fileCount: number;
}

/** Custom node data */
export interface FileNodeData extends Record<string, unknown> {
	label: string;
	fullPath: string;
	fanIn: number;
	fanOut: number;
	depth: number;
	transitiveDeps: number;
	flags: NodeFlag[];
	primaryFlag: NodeFlag | null;
	nodeData: OutputNode;
}

/** Custom edge data */
export interface DependencyEdgeData extends Record<string, unknown> {
	directiveType: string;
	line: number;
	column: number;
	namespace?: string;
	configured?: boolean;
}

/** Node type with FileNodeData */
export type FileNode = Node<FileNodeData>;

/** Edge type with DependencyEdgeData */
export type DependencyEdge = Edge<DependencyEdgeData>;

/** Transform sass-dep output to React Flow elements */
export function transformToFlowElements(data: SassDepOutput): {
	nodes: FileNode[];
	edges: DependencyEdge[];
} {
	const nodes: FileNode[] = [];
	const edges: DependencyEdge[] = [];

	// Transform nodes
	for (const [id, node] of Object.entries(data.nodes)) {
		const primaryFlag = getPrimaryFlag(node.flags);
		nodes.push({
			id,
			type: "fileNode",
			position: { x: 0, y: 0 }, // Will be set by layout
			data: {
				label: getShortPath(id),
				fullPath: node.path,
				fanIn: node.metrics.fan_in,
				fanOut: node.metrics.fan_out,
				depth: node.metrics.depth,
				transitiveDeps: node.metrics.transitive_deps,
				flags: node.flags,
				primaryFlag,
				nodeData: node,
			},
			style: {
				background: getNodeColor(primaryFlag),
			},
		});
	}

	// Transform edges
	for (const [i, edge] of data.edges.entries()) {
		edges.push({
			id: `edge-${i}`,
			source: edge.from,
			target: edge.to,
			type: "smoothstep",
			animated: edge.directive_type === "import",
			style: {
				stroke: getEdgeColor(edge.directive_type),
				strokeWidth: 2,
				strokeDasharray: getEdgeStyle(edge.directive_type) === "dashed" ? "5,5" : undefined,
			},
			markerEnd: {
				type: "arrowclosed" as const,
				color: getEdgeColor(edge.directive_type),
			},
			data: {
				directiveType: edge.directive_type,
				line: edge.location.line,
				column: edge.location.column,
				namespace: edge.namespace,
				configured: edge.configured,
			},
		});
	}

	return { nodes, edges };
}

/** Apply hierarchical layout using dagre */
export function applyDagreLayout(nodes: FileNode[], edges: DependencyEdge[]): FileNode[] {
	const g = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));

	g.setGraph({
		rankdir: "LR",
		nodesep: 50,
		ranksep: 150,
		marginx: 50,
		marginy: 50,
	});

	nodes.forEach((node) => {
		g.setNode(node.id, { width: 180, height: 60 });
	});

	edges.forEach((edge) => {
		g.setEdge(edge.source, edge.target);
	});

	Dagre.layout(g);

	return nodes.map((node) => {
		const position = g.node(node.id);
		return {
			...node,
			position: {
				x: position.x - 90,
				y: position.y - 30,
			},
		};
	});
}

/** Extract just the filename from a path */
function getShortPath(path: string): string {
	const parts = path.split("/");
	return parts.at(-1) ?? path;
}

/** Get the primary flag for coloring (by priority) */
function getPrimaryFlag(flags: NodeFlag[]): NodeFlag | null {
	const priority: NodeFlag[] = ["entry_point", "in_cycle", "orphan", "high_fan_in", "high_fan_out", "leaf"];

	for (const flag of priority) {
		if (flags.includes(flag)) {
			return flag;
		}
	}

	return null;
}

/** Group node type */
export type GroupNode = Node<GroupNodeData>;

/** Extract folder path from a file path */
function getFolderPath(filePath: string): string {
	const parts = filePath.split("/");
	parts.pop(); // Remove filename
	return parts.join("/") || "/";
}

/** Transform nodes to include folder groups */
export function transformToGroupedFlowElements(data: SassDepOutput): {
	nodes: (FileNode | GroupNode)[];
	edges: DependencyEdge[];
} {
	const { nodes: fileNodes, edges } = transformToFlowElements(data);

	// Group files by folder
	const folderMap = new Map<string, FileNode[]>();
	for (const node of fileNodes) {
		const folder = getFolderPath(node.id);
		if (!folderMap.has(folder)) {
			folderMap.set(folder, []);
		}
		folderMap.get(folder)!.push(node);
	}

	// Create group nodes and assign parents
	const groupNodes: GroupNode[] = [];
	const nodesWithParent: FileNode[] = [];

	for (const [folderPath, folderFiles] of folderMap) {
		const groupId = `group-${folderPath}`;
		const folderName = folderPath.split("/").pop() || folderPath || "root";

		// Create group node
		groupNodes.push({
			id: groupId,
			type: "group",
			position: { x: 0, y: 0 },
			zIndex: -1, // Below file nodes but we'll use CSS to position above edges
			data: {
				label: folderName,
				folderPath,
				fileCount: folderFiles.length,
			},
			style: {
				backgroundColor: "var(--color-group-bg)",
				borderRadius: "12px",
				border: "2px dashed var(--color-group-border)",
				padding: "40px 20px 20px 20px",
			},
		});

		// Assign parent to each file node
		for (const fileNode of folderFiles) {
			nodesWithParent.push({
				...fileNode,
				parentId: groupId,
				extent: "parent" as const,
			});
		}
	}

	// Combine all nodes
	const allNodes: (FileNode | GroupNode)[] = [...groupNodes, ...nodesWithParent];

	return { nodes: allNodes, edges };
}

/** Apply layout for grouped nodes */
export function applyGroupedDagreLayout(nodes: (FileNode | GroupNode)[], edges: DependencyEdge[]): (FileNode | GroupNode)[] {
	// Separate group nodes and file nodes
	const groupNodes = nodes.filter((n) => n.type === "group") as GroupNode[];
	const fileNodes = nodes.filter((n) => n.type === "fileNode") as FileNode[];

	// Create a dagre graph for the groups
	const groupGraph = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
	groupGraph.setGraph({
		rankdir: "LR",
		nodesep: 100,
		ranksep: 200,
		marginx: 50,
		marginy: 50,
	});

	// Map to track which groups are connected
	const groupEdges = new Map<string, Set<string>>();

	// Find connections between groups based on file edges
	for (const edge of edges) {
		const sourceNode = fileNodes.find((n) => n.id === edge.source);
		const targetNode = fileNodes.find((n) => n.id === edge.target);
		if (sourceNode?.parentId && targetNode?.parentId && sourceNode.parentId !== targetNode.parentId) {
			const key = sourceNode.parentId;
			if (!groupEdges.has(key)) {
				groupEdges.set(key, new Set());
			}
			groupEdges.get(key)!.add(targetNode.parentId);
		}
	}

	// Estimate group sizes based on file count
	const groupSizes = new Map<string, { width: number; height: number }>();
	const HEADER_HEIGHT = 60; // Space between label and first row of nodes
	const PADDING_X = 30;
	const PADDING_BOTTOM = 30;
	const NODE_WIDTH = 200;
	const NODE_HEIGHT = 90;

	for (const group of groupNodes) {
		const filesInGroup = fileNodes.filter((f) => f.parentId === group.id);
		const cols = Math.ceil(Math.sqrt(filesInGroup.length));
		const rows = Math.ceil(filesInGroup.length / cols);
		const width = cols * NODE_WIDTH + PADDING_X * 2;
		const height = rows * NODE_HEIGHT + HEADER_HEIGHT + PADDING_BOTTOM;
		groupSizes.set(group.id, { width, height });
		groupGraph.setNode(group.id, { width, height });
	}

	// Add edges between groups
	for (const [source, targets] of groupEdges) {
		for (const target of targets) {
			groupGraph.setEdge(source, target);
		}
	}

	// Layout groups
	Dagre.layout(groupGraph);

	// Position groups (add offset to leave room for label above)
	const LABEL_OFFSET = 32;
	const positionedGroups: GroupNode[] = groupNodes.map((group) => {
		const pos = groupGraph.node(group.id);
		const size = groupSizes.get(group.id)!;
		return {
			...group,
			position: {
				x: pos.x - size.width / 2,
				y: pos.y - size.height / 2 + LABEL_OFFSET,
			},
			style: {
				...group.style,
				width: size.width,
				height: size.height,
			},
		};
	});

	// Position files within their groups
	const positionedFiles: FileNode[] = [];
	for (const group of positionedGroups) {
		const filesInGroup = fileNodes.filter((f) => f.parentId === group.id);
		const cols = Math.ceil(Math.sqrt(filesInGroup.length));

		filesInGroup.forEach((file, index) => {
			const col = index % cols;
			const row = Math.floor(index / cols);
			const position: XYPosition = {
				x: PADDING_X + col * NODE_WIDTH,
				y: HEADER_HEIGHT + row * NODE_HEIGHT,
			};
			positionedFiles.push({
				...file,
				position,
			});
		});
	}

	return [...positionedGroups, ...positionedFiles];
}
