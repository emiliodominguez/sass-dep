import { useMemo, useState } from "react";

import { ShowMoreButton } from "./ShowMoreButton";
import { useShowMore } from "./useShowMore";
import { type GraphIndex, getNeighborIds } from "../../utils/graphIndex";
import styles from "./Sidebar.module.scss";

/** Direction in which the tree follows edges. */
type TreeDirection = "dependents" | "dependencies";

/** Number of children rendered per tree node before "show more". */
const CHILDREN_PAGE_SIZE = 20;

/** Props for the DependencyTree component. */
interface DependencyTreeProps {
	/** File at the root of the tree */
	rootNodeId: string;
	/** Graph adjacency index */
	index: GraphIndex;
	/** Which edges to follow */
	direction: TreeDirection;
	/** Maximum number of levels below the root */
	maxDepth?: number;
	/** Called when a file in the tree is clicked */
	onFocusNode?: (nodeId: string) => void;
}

/** A node in the breadth-first dependency tree. */
interface TreeNodeData {
	/** File ID */
	id: string;
	/** Children discovered from this node */
	children: TreeNodeData[];
	/** Distance from the root */
	depth: number;
}

/** A built tree and the number of files it contains (excluding the root). */
interface BuiltTree {
	/** Root node */
	root: TreeNodeData;
	/** Number of distinct files below the root */
	size: number;
}

/**
 * Builds a breadth-first spanning tree of a file's (transitive) dependents or dependencies.
 * A single shared visited set guarantees each file appears once (at its shortest distance),
 * keeping the work O(N + E) instead of exponential.
 * @param rootId - Root file ID
 * @param index - Graph adjacency index
 * @param direction - Which edges to follow
 * @param maxDepth - Maximum number of levels below the root
 * @returns The tree and its size
 */
function buildTree(rootId: string, index: GraphIndex, direction: TreeDirection, maxDepth: number): BuiltTree {
	const root: TreeNodeData = { id: rootId, children: [], depth: 0 };
	const visited = new Set<string>([rootId]);
	const queue: TreeNodeData[] = [root];

	for (let head = 0; head < queue.length; head++) {
		const node = queue[head]!;
		if (node.depth >= maxDepth) continue;

		for (const neighborId of getNeighborIds(index, node.id, direction)) {
			if (visited.has(neighborId)) continue;
			visited.add(neighborId);

			const child: TreeNodeData = { id: neighborId, children: [], depth: node.depth + 1 };
			node.children.push(child);
			queue.push(child);
		}
	}

	return { root, size: visited.size - 1 };
}

/** Props for the TreeChildren component. */
interface TreeChildrenProps {
	/** Children to render */
	nodes: TreeNodeData[];
	/** Called when a file is clicked */
	onFocusNode?: (nodeId: string) => void;
}

/**
 * Renders a capped list of sibling tree nodes with a "show more" affordance.
 * @param props - Component props
 * @returns List of tree nodes
 */
function TreeChildren({ nodes, onFocusNode }: TreeChildrenProps) {
	const { limit, remaining, showMore } = useShowMore(nodes.length, CHILDREN_PAGE_SIZE);

	return (
		<>
			{nodes.slice(0, limit).map((child) => (
				<TreeNode key={child.id} node={child} onFocusNode={onFocusNode} />
			))}
			<ShowMoreButton remaining={remaining} pageSize={CHILDREN_PAGE_SIZE} onClick={showMore} />
		</>
	);
}

/** Props for the TreeNode component. */
interface TreeNodeProps {
	/** Node to render */
	node: TreeNodeData;
	/** Called when a file is clicked */
	onFocusNode?: (nodeId: string) => void;
}

/**
 * Renders a single node in the dependency tree.
 * @param props - Component props
 * @returns Tree node element
 */
function TreeNode({ node, onFocusNode }: TreeNodeProps) {
	const [isExpanded, setIsExpanded] = useState(node.depth < 2);
	const hasChildren = node.children.length > 0;

	/**
	 * Toggles the expanded/collapsed state of the tree node.
	 */
	function toggleExpand() {
		setIsExpanded((prev) => !prev);
	}

	/** Focuses this node's file in the graph. */
	function handleFocus() {
		onFocusNode?.(node.id);
	}

	return (
		<div className={styles["tree-node"]}>
			<div className={styles["tree-node-row"]}>
				{hasChildren ? (
					<button className={styles["tree-toggle"]} onClick={toggleExpand} aria-label={isExpanded ? "Collapse" : "Expand"}>
						<svg
							width="10"
							height="10"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							strokeWidth="2"
							style={{ transform: isExpanded ? "rotate(90deg)" : "rotate(0deg)" }}
						>
							<polyline points="9 18 15 12 9 6" />
						</svg>
					</button>
				) : (
					<span className={styles["tree-spacer"]} />
				)}
				<button className={styles["tree-node-label"]} onClick={handleFocus} title={`Focus on ${node.id}`}>
					<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
						<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
						<polyline points="14 2 14 8 20 8" />
					</svg>
					{node.id}
				</button>
				{hasChildren && <span className={styles["tree-count"]}>({node.children.length})</span>}
			</div>
			{hasChildren && isExpanded && (
				<div className={styles["tree-children"]}>
					<TreeChildren nodes={node.children} onFocusNode={onFocusNode} />
				</div>
			)}
		</div>
	);
}

/**
 * Dependency tree visualization showing transitive relationships (each file listed once, at its
 * shortest distance from the root).
 * @param props - Component props
 * @returns Expandable tree view or null if empty
 */
export function DependencyTree({ rootNodeId, index, direction, maxDepth = 4, onFocusNode }: DependencyTreeProps) {
	const { root, size } = useMemo(() => buildTree(rootNodeId, index, direction, maxDepth), [rootNodeId, index, direction, maxDepth]);

	if (root.children.length === 0) {
		return null;
	}

	const title = direction === "dependents" ? "Dependents Tree" : "Dependencies Tree";
	const relation = direction === "dependents" ? "Files that depend on this file" : "Files this file depends on";
	const description = `${relation} (up to ${maxDepth} levels, ${size} file${size === 1 ? "" : "s"})`;

	return (
		<div className={styles["detail-group"]}>
			<label>
				{title}
				<span className={styles["label-count"]}>{root.children.length}</span>
			</label>
			<p className={styles["tree-description"]}>{description}</p>
			<div className={styles["dependency-tree"]}>
				<TreeChildren nodes={root.children} onFocusNode={onFocusNode} />
			</div>
		</div>
	);
}
