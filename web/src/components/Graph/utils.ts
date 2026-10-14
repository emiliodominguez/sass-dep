import type { Edge, Node } from "@xyflow/react";

import { type EdgeHighlight, getEdgeMarker, getEdgeStyle } from "./styles";
import { type LayoutInput, type LayoutResult, getFolderPath } from "./layout";
import { type GraphIndex, edgePairKey } from "../../utils/graphIndex";
import type { DirectiveType, NodeFlag, SassDepOutput } from "../../types/sass-dep";

/** Data carried by a folder group node. */
export interface GroupNodeData extends Record<string, unknown> {
	/** Last folder segment */
	label: string;
	/** Full folder path */
	folderPath: string;
	/** Number of files in the folder */
	fileCount: number;
}

/** Data carried by a file node: static metrics plus per-render decoration flags. */
export interface FileNodeData extends Record<string, unknown> {
	/** File name shown on the node */
	label: string;
	/** Full file path (tooltip) */
	fullPath: string;
	/** Number of direct dependents */
	fanIn: number;
	/** Number of direct dependencies */
	fanOut: number;
	/** Depth from the nearest entry point, or null if unreachable */
	depth: number | null;
	/** Number of transitive dependencies */
	transitiveDeps: number;
	/** Number of transitive dependents (impact) */
	transitiveDependents: number;
	/** All flags of the file */
	flags: NodeFlag[];
	/** Flag that decides the node colour */
	primaryFlag: NodeFlag | null;
	/** Whether the node was focused from the sidebar */
	isFocused: boolean;
	/** Whether the node is the first shift-clicked path endpoint */
	isPathSource: boolean;
	/** Whether the node is the second shift-clicked path endpoint */
	isPathTarget: boolean;
	/** Whether the node lies on the highlighted dependency path */
	isInPath: boolean;
	/** Whether the node is highlighted as part of a cycle */
	isInCycle: boolean;
	/** Whether the node is highlighted for using legacy `@import` */
	isLegacyImport: boolean;
}

/** Data carried by a dependency edge. */
export interface DependencyEdgeData extends Record<string, unknown> {
	/** Directive type that created the dependency */
	directiveType: DirectiveType;
	/** `from->to` key shared by all edges between the same two files */
	pairKey: string;
	/** Current highlight state */
	highlight: EdgeHighlight;
}

/** React Flow node representing a file. */
export type FileFlowNode = Node<FileNodeData, "fileNode">;

/** React Flow node representing a folder group. */
export type GroupFlowNode = Node<GroupNodeData, "group">;

/** Any node rendered in the graph. */
export type AppNode = FileFlowNode | GroupFlowNode;

/** React Flow edge representing a dependency. */
export type DependencyEdge = Edge<DependencyEdgeData>;

/** Highlight/visibility state applied on top of the laid-out nodes. */
export interface NodeDecoration {
	/** Visible file IDs */
	visibleIds: ReadonlySet<string>;
	/** Visible group node IDs (groups with at least one visible file) */
	visibleGroupIds: ReadonlySet<string>;
	/** Node focused from the sidebar */
	focusedNodeId: string | null;
	/** First path endpoint */
	pathSource: string | null;
	/** Second path endpoint */
	pathTarget: string | null;
	/** Nodes on the found path */
	pathNodeIds: ReadonlySet<string>;
	/** Nodes to highlight as in a cycle, or null when cycle highlighting is off */
	cycleNodeIds: ReadonlySet<string> | null;
	/** Nodes to highlight as using `@import`, or null when that highlight is off */
	importerIds: ReadonlySet<string> | null;
}

/** Highlight/visibility state applied on top of the base edges. */
export interface EdgeDecoration {
	/** Visible file IDs; an edge is visible only if both endpoints are */
	visibleIds: ReadonlySet<string>;
	/** Pair keys of edges on the found path */
	pathEdgeKeys: ReadonlySet<string>;
	/** Pair keys of cycle edges, or null when cycle highlighting is off */
	cycleEdgeKeys: ReadonlySet<string> | null;
	/** Whether `@import` edges are emphasised */
	highlightImports: boolean;
	/** Whether the graph is large (no animation, cheaper dash patterns) */
	isLargeGraph: boolean;
}

/** Priority order used to pick the flag that colours a node. */
const FLAG_PRIORITY: readonly NodeFlag[] = ["entry_point", "in_cycle", "orphan", "high_fan_in", "high_fan_out", "leaf"];

/**
 * Extracts just the file name from a path.
 * @param path - File ID or path
 * @returns Last path segment
 */
function getShortPath(path: string): string {
	return path.slice(path.lastIndexOf("/") + 1) || path;
}

/**
 * Gets the primary flag for colouring (by priority).
 * @param flags - Flags of the node
 * @returns Highest-priority flag, or null when the node has none
 */
function getPrimaryFlag(flags: readonly NodeFlag[]): NodeFlag | null {
	return FLAG_PRIORITY.find((flag) => flags.includes(flag)) ?? null;
}

/**
 * Builds the group node ID for a folder.
 * @param folderPath - Folder path
 * @returns Group node ID
 */
export function getGroupId(folderPath: string): string {
	return `group-${folderPath}`;
}

/**
 * Builds the minimal, transferable description of the graph that the layout worker needs.
 * @param data - Normalized sass-dep output
 * @param grouped - Whether to group files by folder
 * @returns Layout input
 */
export function createLayoutInput(data: SassDepOutput, grouped: boolean): LayoutInput {
	const ids = Object.keys(data.nodes);
	const indexById = new Map<string, number>();
	for (const [i, id] of ids.entries()) indexById.set(id, i);

	const edges = new Uint32Array(data.edges.length * 2);
	for (const [i, edge] of data.edges.entries()) {
		edges[i * 2] = indexById.get(edge.from) ?? 0;
		edges[i * 2 + 1] = indexById.get(edge.to) ?? 0;
	}

	return { grouped, ids, edges };
}

/**
 * Creates undecorated file nodes (at the origin) for every file. Node colours come from CSS classes.
 * @param data - Normalized sass-dep output
 * @returns One file node per entry of `data.nodes`, in the same order as `createLayoutInput` ids
 */
export function buildFileNodes(data: SassDepOutput): FileFlowNode[] {
	return Object.entries(data.nodes).map(([id, node]) => ({
		id,
		type: "fileNode",
		position: { x: 0, y: 0 },
		data: {
			label: getShortPath(id),
			fullPath: node.path,
			fanIn: node.metrics.fan_in,
			fanOut: node.metrics.fan_out,
			depth: node.metrics.depth,
			transitiveDeps: node.metrics.transitive_deps,
			transitiveDependents: node.metrics.transitive_dependents,
			flags: node.flags,
			primaryFlag: getPrimaryFlag(node.flags),
			isFocused: false,
			isPathSource: false,
			isPathTarget: false,
			isInPath: false,
			isInCycle: false,
			isLegacyImport: false,
		},
	}));
}

/**
 * Creates undecorated edges. Colours are CSS variable references, so they follow the theme.
 * @param data - Normalized sass-dep output
 * @param index - Graph index (provides stable edge IDs)
 * @param isLargeGraph - Whether to use the cheaper large-graph edge styles
 * @returns One edge per entry of `data.edges`
 */
export function buildEdges(data: SassDepOutput, index: GraphIndex, isLargeGraph: boolean): DependencyEdge[] {
	return data.edges.map((edge, i) => ({
		id: index.edgeIds[i]!,
		source: edge.from,
		target: edge.to,
		type: "smoothstep",
		style: getEdgeStyle(edge.directive_type, "none", isLargeGraph),
		markerEnd: getEdgeMarker(edge.directive_type, "none"),
		data: { directiveType: edge.directive_type, pairKey: edgePairKey(edge.from, edge.to), highlight: "none" },
	}));
}

/**
 * Combines file nodes with a layout result into positioned React Flow nodes (groups first, as React Flow requires).
 * @param fileNodes - Undecorated file nodes, in layout input order
 * @param layout - Layout result for the same ids
 * @returns Positioned nodes
 */
export function applyLayout(fileNodes: readonly FileFlowNode[], layout: LayoutResult): AppNode[] {
	const groupNodes: GroupFlowNode[] = layout.groups.map((group) => ({
		id: getGroupId(group.folderPath),
		type: "group",
		position: { x: group.x, y: group.y },
		zIndex: -1,
		data: {
			label: group.folderPath.split("/").pop() || group.folderPath || "root",
			folderPath: group.folderPath,
			fileCount: group.fileCount,
		},
		style: {
			width: group.width,
			height: group.height,
			backgroundColor: "var(--color-group-bg)",
			borderRadius: "12px",
			border: "2px dashed var(--color-group-border)",
			padding: "40px 20px 20px 20px",
		},
	}));

	/**
	 * Places one file node at its computed position (inside its folder group, if any).
	 * @param node - Undecorated file node
	 * @param i - Index of the node in the layout input
	 * @returns Positioned file node
	 */
	function positionFileNode(node: FileFlowNode, i: number): FileFlowNode {
		const position = { x: layout.positions[i * 2] ?? 0, y: layout.positions[i * 2 + 1] ?? 0 };
		const group = layout.groups[layout.groupOf[i] ?? -1];
		return group ? { ...node, position, parentId: getGroupId(group.folderPath), extent: "parent" } : { ...node, position };
	}

	const positioned = fileNodes.map(positionFileNode);

	return [...groupNodes, ...positioned];
}

/**
 * Computes which group nodes contain at least one visible file.
 * @param visibleIds - Visible file IDs
 * @returns Set of group node IDs
 */
export function getVisibleGroupIds(visibleIds: ReadonlySet<string>): Set<string> {
	const groups = new Set<string>();
	for (const id of visibleIds) groups.add(getGroupId(getFolderPath(id)));
	return groups;
}

/**
 * Applies visibility/highlight state to a node, returning the SAME object when nothing changed
 * so memoized node components (and React Flow's internals) skip re-rendering.
 * @param node - Current node
 * @param decoration - Desired state
 * @returns The node itself, or an updated copy
 */
export function decorateNode(node: AppNode, decoration: NodeDecoration): AppNode {
	if (node.type === "group") {
		const hidden = !decoration.visibleGroupIds.has(node.id);
		return (node.hidden ?? false) === hidden ? node : { ...node, hidden };
	}

	const { id, data } = node;
	const hidden = !decoration.visibleIds.has(id);
	const isFocused = id === decoration.focusedNodeId;
	const isPathSource = id === decoration.pathSource;
	const isPathTarget = id === decoration.pathTarget;
	const isInPath = decoration.pathNodeIds.has(id);
	const isInCycle = decoration.cycleNodeIds?.has(id) ?? false;
	const isLegacyImport = decoration.importerIds?.has(id) ?? false;

	if (
		(node.hidden ?? false) === hidden &&
		data.isFocused === isFocused &&
		data.isPathSource === isPathSource &&
		data.isPathTarget === isPathTarget &&
		data.isInPath === isInPath &&
		data.isInCycle === isInCycle &&
		data.isLegacyImport === isLegacyImport
	) {
		return node;
	}

	return { ...node, hidden, data: { ...data, isFocused, isPathSource, isPathTarget, isInPath, isInCycle, isLegacyImport } };
}

/**
 * Applies visibility/highlight state to an edge, returning the SAME object when nothing changed.
 * @param edge - Current edge
 * @param decoration - Desired state
 * @returns The edge itself, or an updated copy
 */
export function decorateEdge(edge: DependencyEdge, decoration: EdgeDecoration): DependencyEdge {
	const data = edge.data;
	if (!data) return edge;

	const hidden = !decoration.visibleIds.has(edge.source) || !decoration.visibleIds.has(edge.target);
	let highlight: EdgeHighlight = "none";
	if (decoration.pathEdgeKeys.has(data.pairKey)) {
		highlight = "path";
	} else if (decoration.cycleEdgeKeys?.has(data.pairKey)) {
		highlight = "cycle";
	} else if (decoration.highlightImports && data.directiveType === "import") {
		highlight = "import";
	}
	const animated = !decoration.isLargeGraph && (highlight === "path" || highlight === "cycle");

	if ((edge.hidden ?? false) === hidden && data.highlight === highlight && (edge.animated ?? false) === animated) {
		return edge;
	}

	return {
		...edge,
		hidden,
		animated,
		zIndex: highlight === "none" ? 0 : 1,
		style: getEdgeStyle(data.directiveType, highlight, decoration.isLargeGraph),
		markerEnd: getEdgeMarker(data.directiveType, highlight),
		data: { ...data, highlight },
	};
}
