import Dagre from "@dagrejs/dagre";

/** Width reserved for a file node in the flat layout. */
export const NODE_WIDTH = 180;
/** Height reserved for a file node in the flat layout. */
export const NODE_HEIGHT = 60;

/** Grid cell width for files inside a folder group. */
const GROUP_CELL_WIDTH = 200;
/** Grid cell height for files inside a folder group. */
const GROUP_CELL_HEIGHT = 90;
/** Space between a group's top edge and its first row of files. */
const GROUP_HEADER_HEIGHT = 60;
/** Horizontal padding inside a group. */
const GROUP_PADDING_X = 30;
/** Bottom padding inside a group. */
const GROUP_PADDING_BOTTOM = 30;
/** Vertical offset leaving room for the group label rendered above the group. */
const GROUP_LABEL_OFFSET = 32;
/**
 * Above this many nodes dagre's default network-simplex ranker becomes the bottleneck
 * (~2s at 300 nodes, >10min at 3000), so the much cheaper tight-tree ranker is used instead.
 */
const NETWORK_SIMPLEX_MAX_NODES = 200;

/**
 * Picks the dagre ranking algorithm for a graph of the given size.
 * @param nodeCount - Number of nodes in the dagre graph
 * @returns Dagre ranker name
 */
function chooseRanker(nodeCount: number): "network-simplex" | "tight-tree" {
	return nodeCount <= NETWORK_SIMPLEX_MAX_NODES ? "network-simplex" : "tight-tree";
}

/** Minimal, structured-clone friendly description of the graph to lay out. */
export interface LayoutInput {
	/** Whether files are grouped into folder containers */
	grouped: boolean;
	/** File IDs; positions in the result are reported by index into this array */
	ids: string[];
	/** Flattened edge list as pairs of indices into `ids`: `[from0, to0, from1, to1, ...]` */
	edges: Uint32Array;
}

/** Layout of one folder group (grouped mode only). */
export interface GroupLayout {
	/** Folder path, e.g. `src/components` (or `/` for the root) */
	folderPath: string;
	/** Top-left x position */
	x: number;
	/** Top-left y position */
	y: number;
	/** Group width */
	width: number;
	/** Group height */
	height: number;
	/** Number of files in the folder */
	fileCount: number;
}

/** Result of a layout run. */
export interface LayoutResult {
	/**
	 * Top-left position of every file as `[x0, y0, x1, y1, ...]`, by index into `LayoutInput.ids`.
	 * In grouped mode positions are relative to the file's group.
	 */
	positions: Float64Array;
	/** Folder groups (empty in flat mode) */
	groups: GroupLayout[];
	/** Index into `groups` for every file, or -1 in flat mode */
	groupOf: Int32Array;
}

/**
 * Extracts the folder path from a file ID.
 * @param filePath - File ID such as `src/components/_button.scss`
 * @returns Folder path (`/` for files at the root)
 */
export function getFolderPath(filePath: string): string {
	const slash = filePath.lastIndexOf("/");
	return slash > 0 ? filePath.slice(0, slash) : "/";
}

/**
 * Lays out files hierarchically (left to right) with dagre.
 * @param input - Graph to lay out
 * @returns Positions for every file
 */
function layoutFlat(input: LayoutInput): LayoutResult {
	const graph = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
	graph.setGraph({ rankdir: "LR", nodesep: 50, ranksep: 150, marginx: 50, marginy: 50, ranker: chooseRanker(input.ids.length) });

	for (const id of input.ids) {
		graph.setNode(id, { width: NODE_WIDTH, height: NODE_HEIGHT });
	}
	for (let i = 0; i < input.edges.length; i += 2) {
		graph.setEdge(input.ids[input.edges[i]!]!, input.ids[input.edges[i + 1]!]!);
	}

	Dagre.layout(graph);

	const positions = new Float64Array(input.ids.length * 2);
	for (const [i, id] of input.ids.entries()) {
		const node = graph.node(id);
		positions[i * 2] = node.x - NODE_WIDTH / 2;
		positions[i * 2 + 1] = node.y - NODE_HEIGHT / 2;
	}

	return { positions, groups: [], groupOf: new Int32Array(input.ids.length).fill(-1) };
}

/**
 * Lays out folder groups with dagre (using inter-folder edges) and places files in a grid inside each group.
 * Runs in O(N + E) apart from dagre itself, using index maps instead of per-edge/per-group scans.
 * @param input - Graph to lay out
 * @returns Group rectangles and group-relative file positions
 */
function layoutGrouped(input: LayoutInput): LayoutResult {
	const groupIndexByFolder = new Map<string, number>();
	const groups: GroupLayout[] = [];
	const members: number[][] = [];
	const groupOf = new Int32Array(input.ids.length);

	for (const [i, id] of input.ids.entries()) {
		const folderPath = getFolderPath(id);
		let groupIndex = groupIndexByFolder.get(folderPath);
		if (groupIndex === undefined) {
			groupIndex = groups.length;
			groupIndexByFolder.set(folderPath, groupIndex);
			groups.push({ folderPath, x: 0, y: 0, width: 0, height: 0, fileCount: 0 });
			members.push([]);
		}
		groupOf[i] = groupIndex;
		members[groupIndex]!.push(i);
	}

	const graph = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
	graph.setGraph({ rankdir: "LR", nodesep: 100, ranksep: 200, marginx: 50, marginy: 50, ranker: chooseRanker(groups.length) });

	const columns: number[] = [];
	for (const [g, group] of groups.entries()) {
		const count = members[g]!.length;
		const cols = Math.max(1, Math.ceil(Math.sqrt(count)));
		const rows = Math.ceil(count / cols);
		columns.push(cols);
		group.fileCount = count;
		group.width = cols * GROUP_CELL_WIDTH + GROUP_PADDING_X * 2;
		group.height = rows * GROUP_CELL_HEIGHT + GROUP_HEADER_HEIGHT + GROUP_PADDING_BOTTOM;
		graph.setNode(String(g), { width: group.width, height: group.height });
	}

	// Connect groups that have at least one file-level edge between them (deduplicated).
	const seenGroupEdges = new Set<number>();
	for (let i = 0; i < input.edges.length; i += 2) {
		const from = groupOf[input.edges[i]!]!;
		const to = groupOf[input.edges[i + 1]!]!;
		const key = from * groups.length + to;
		if (from !== to && !seenGroupEdges.has(key)) {
			seenGroupEdges.add(key);
			graph.setEdge(String(from), String(to));
		}
	}

	Dagre.layout(graph);

	for (const [g, group] of groups.entries()) {
		const node = graph.node(String(g));
		group.x = node.x - group.width / 2;
		group.y = node.y - group.height / 2 + GROUP_LABEL_OFFSET;
	}

	const positions = new Float64Array(input.ids.length * 2);
	for (const [g, files] of members.entries()) {
		const cols = columns[g]!;
		for (const [slot, fileIndex] of files.entries()) {
			positions[fileIndex * 2] = GROUP_PADDING_X + (slot % cols) * GROUP_CELL_WIDTH;
			positions[fileIndex * 2 + 1] = GROUP_HEADER_HEIGHT + Math.floor(slot / cols) * GROUP_CELL_HEIGHT;
		}
	}

	return { positions, groups, groupOf };
}

/**
 * Computes a layout synchronously. Used inside the layout worker and as the main-thread fallback.
 * @param input - Graph to lay out
 * @returns Layout result
 */
export function computeLayout(input: LayoutInput): LayoutResult {
	return input.grouped ? layoutGrouped(input) : layoutFlat(input);
}
