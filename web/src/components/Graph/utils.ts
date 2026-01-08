import type { Node, Edge } from "@xyflow/react";
import Dagre from "@dagrejs/dagre";
import type { SassDepOutput, NodeFlag, OutputNode } from "../../types/sass-dep";
import { getNodeColor, getEdgeColor, getEdgeStyle } from "./styles";

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
	for (let i = 0; i < data.edges.length; i++) {
		const edge = data.edges[i];
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
	return parts[parts.length - 1];
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
