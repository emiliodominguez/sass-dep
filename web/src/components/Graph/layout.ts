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
 * Dagre ranks and orders nodes with recursive depth-first searches and overflows the stack on
 * import chains roughly 2000 files deep. Graphs whose BFS depth exceeds this use the iterative
 * layered fallback directly (dagre failures fall back to it as well).
 */
const MAX_DAGRE_DEPTH = 1000;

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

/** Spacing options for the layered layout. */
interface LayeredSpacing {
	/** Gap between nodes in the same rank */
	nodesep: number;
	/** Gap between ranks */
	ranksep: number;
	/** Outer margin */
	margin: number;
}

/**
 * Assigns every node a rank equal to its BFS distance from the nearest source (in-degree 0).
 * Nodes only reachable through cycles are seeded in index order. Iterative, O(N + E).
 * @param count - Number of nodes
 * @param edges - Flattened `[from, to, ...]` index pairs
 * @returns Rank per node and the largest rank
 */
function computeRanks(count: number, edges: Uint32Array): { ranks: Int32Array; maxRank: number } {
	const offsets = new Uint32Array(count + 1);
	const inDegree = new Uint32Array(count);
	for (let i = 0; i < edges.length; i += 2) {
		offsets[edges[i]! + 1]!++;
		inDegree[edges[i + 1]!]!++;
	}
	for (let i = 0; i < count; i++) offsets[i + 1]! += offsets[i]!;
	const targets = new Uint32Array(edges.length / 2);
	const cursor = offsets.slice(0, count);
	for (let i = 0; i < edges.length; i += 2) targets[cursor[edges[i]!]!++] = edges[i + 1]!;

	const ranks = new Int32Array(count).fill(-1);
	const queue = new Uint32Array(count);
	let head = 0;
	let tail = 0;
	let maxRank = 0;

	/**
	 * Runs BFS from the queued nodes until the queue is drained.
	 */
	function drain(): void {
		while (head < tail) {
			const node = queue[head++]!;
			const next = ranks[node]! + 1;
			for (let e = offsets[node]!; e < offsets[node + 1]!; e++) {
				const target = targets[e]!;
				if (ranks[target] === -1) {
					ranks[target] = next;
					maxRank = Math.max(maxRank, next);
					queue[tail++] = target;
				}
			}
		}
	}

	for (let i = 0; i < count; i++) {
		if (inDegree[i] === 0) {
			ranks[i] = 0;
			queue[tail++] = i;
		}
	}
	drain();
	for (let i = 0; i < count; i++) {
		if (ranks[i] === -1) {
			ranks[i] = 0;
			queue[tail++] = i;
			drain();
		}
	}

	return { ranks, maxRank };
}

/**
 * Places nodes in left-to-right columns by rank, stacking each column vertically.
 * Iterative and O(N + E), so it works for graphs of any depth.
 * @param ranks - Rank per node
 * @param maxRank - Largest rank
 * @param widths - Width per node
 * @param heights - Height per node
 * @param spacing - Gaps and margin
 * @returns Center position per node as `[x0, y0, x1, y1, ...]`
 */
function placeLayered(ranks: Int32Array, maxRank: number, widths: Float64Array, heights: Float64Array, spacing: LayeredSpacing): Float64Array {
	const count = ranks.length;
	const columnWidth = new Float64Array(maxRank + 1);
	for (let i = 0; i < count; i++) columnWidth[ranks[i]!] = Math.max(columnWidth[ranks[i]!]!, widths[i]!);

	const columnX = new Float64Array(maxRank + 1);
	let x = spacing.margin;
	for (let r = 0; r <= maxRank; r++) {
		columnX[r] = x + columnWidth[r]! / 2;
		x += columnWidth[r]! + spacing.ranksep;
	}

	const nextY = new Float64Array(maxRank + 1).fill(spacing.margin);
	const centers = new Float64Array(count * 2);
	for (let i = 0; i < count; i++) {
		const rank = ranks[i]!;
		centers[i * 2] = columnX[rank]!;
		centers[i * 2 + 1] = nextY[rank]! + heights[i]! / 2;
		nextY[rank]! += heights[i]! + spacing.nodesep;
	}

	return centers;
}

/**
 * Computes node centers with dagre, or with the iterative layered layout for very deep graphs
 * and whenever dagre fails (e.g. stack overflow).
 * @param count - Number of nodes
 * @param edges - Flattened `[from, to, ...]` index pairs (no self-loops required)
 * @param widths - Width per node
 * @param heights - Height per node
 * @param spacing - Gaps and margin
 * @returns Center position per node as `[x0, y0, x1, y1, ...]`
 */
function computeCenters(count: number, edges: Uint32Array, widths: Float64Array, heights: Float64Array, spacing: LayeredSpacing): Float64Array {
	const { ranks, maxRank } = computeRanks(count, edges);
	if (maxRank <= MAX_DAGRE_DEPTH) {
		try {
			return layoutWithDagre(count, edges, widths, heights, spacing);
		} catch (error) {
			console.warn("dagre layout failed; using the layered fallback layout.", error);
		}
	}
	return placeLayered(ranks, maxRank, widths, heights, spacing);
}

/**
 * Computes node centers with dagre.
 * @param count - Number of nodes
 * @param edges - Flattened `[from, to, ...]` index pairs
 * @param widths - Width per node
 * @param heights - Height per node
 * @param spacing - Gaps and margin
 * @returns Center position per node as `[x0, y0, x1, y1, ...]`
 */
function layoutWithDagre(count: number, edges: Uint32Array, widths: Float64Array, heights: Float64Array, spacing: LayeredSpacing): Float64Array {
	const graph = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
	graph.setGraph({
		rankdir: "LR",
		nodesep: spacing.nodesep,
		ranksep: spacing.ranksep,
		marginx: spacing.margin,
		marginy: spacing.margin,
		ranker: chooseRanker(count),
	});

	for (let i = 0; i < count; i++) {
		graph.setNode(String(i), { width: widths[i]!, height: heights[i]! });
	}
	for (let i = 0; i < edges.length; i += 2) {
		graph.setEdge(String(edges[i]!), String(edges[i + 1]!));
	}

	Dagre.layout(graph);

	const centers = new Float64Array(count * 2);
	for (let i = 0; i < count; i++) {
		const node = graph.node(String(i));
		centers[i * 2] = node.x;
		centers[i * 2 + 1] = node.y;
	}
	return centers;
}

/**
 * Lays out files hierarchically (left to right) with dagre, or the layered fallback for very deep graphs.
 * @param input - Graph to lay out
 * @returns Positions for every file
 */
function layoutFlat(input: LayoutInput): LayoutResult {
	const count = input.ids.length;
	const centers = computeCenters(count, input.edges, new Float64Array(count).fill(NODE_WIDTH), new Float64Array(count).fill(NODE_HEIGHT), {
		nodesep: 50,
		ranksep: 150,
		margin: 50,
	});

	const positions = new Float64Array(count * 2);
	for (let i = 0; i < count; i++) {
		positions[i * 2] = centers[i * 2]! - NODE_WIDTH / 2;
		positions[i * 2 + 1] = centers[i * 2 + 1]! - NODE_HEIGHT / 2;
	}

	return { positions, groups: [], groupOf: new Int32Array(count).fill(-1) };
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

	const widths = new Float64Array(groups.length);
	const heights = new Float64Array(groups.length);
	const columns: number[] = [];
	for (const [g, group] of groups.entries()) {
		const count = members[g]!.length;
		const cols = Math.max(1, Math.ceil(Math.sqrt(count)));
		const rows = Math.ceil(count / cols);
		columns.push(cols);
		group.fileCount = count;
		group.width = cols * GROUP_CELL_WIDTH + GROUP_PADDING_X * 2;
		group.height = rows * GROUP_CELL_HEIGHT + GROUP_HEADER_HEIGHT + GROUP_PADDING_BOTTOM;
		widths[g] = group.width;
		heights[g] = group.height;
	}

	// Connect groups that have at least one file-level edge between them (deduplicated).
	const seenGroupEdges = new Set<number>();
	const groupEdges: number[] = [];
	for (let i = 0; i < input.edges.length; i += 2) {
		const from = groupOf[input.edges[i]!]!;
		const to = groupOf[input.edges[i + 1]!]!;
		const key = from * groups.length + to;
		if (from !== to && !seenGroupEdges.has(key)) {
			seenGroupEdges.add(key);
			groupEdges.push(from, to);
		}
	}

	const centers = computeCenters(groups.length, Uint32Array.from(groupEdges), widths, heights, { nodesep: 100, ranksep: 200, margin: 50 });
	for (const [g, group] of groups.entries()) {
		group.x = centers[g * 2]! - group.width / 2;
		group.y = centers[g * 2 + 1]! - group.height / 2 + GROUP_LABEL_OFFSET;
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
