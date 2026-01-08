import { useState, useCallback, useMemo } from "react";
import type { OutputEdge } from "../../types/sass-dep";

interface DependencyTreeProps {
	// Data props
	rootNodeId: string;
	edges: OutputEdge[];
	direction: "dependents" | "dependencies";
	maxDepth?: number;
	// Callbacks
	onFocusNode?: (nodeId: string) => void;
}

interface TreeNodeData {
	id: string;
	children: TreeNodeData[];
	depth: number;
}

function buildTree(
	rootId: string,
	edges: OutputEdge[],
	direction: "dependents" | "dependencies",
	maxDepth: number,
	visited = new Set<string>(),
	currentDepth = 0,
): TreeNodeData {
	const node: TreeNodeData = {
		id: rootId,
		children: [],
		depth: currentDepth,
	};

	if (currentDepth >= maxDepth || visited.has(rootId)) {
		return node;
	}

	visited.add(rootId);

	// Find connected nodes based on direction
	const connectedIds = direction === "dependents" ? edges.filter((e) => e.to === rootId).map((e) => e.from) : edges.filter((e) => e.from === rootId).map((e) => e.to);

	for (const childId of connectedIds) {
		if (!visited.has(childId)) {
			node.children.push(buildTree(childId, edges, direction, maxDepth, new Set(visited), currentDepth + 1));
		}
	}

	return node;
}

function TreeNode({ node, onFocusNode, isRoot = false }: { node: TreeNodeData; onFocusNode?: (nodeId: string) => void; isRoot?: boolean }) {
	const [isExpanded, setIsExpanded] = useState(node.depth < 2);
	const hasChildren = node.children.length > 0;

	const toggleExpand = useCallback(() => {
		setIsExpanded((prev) => !prev);
	}, []);

	return (
		<div className={`tree-node ${isRoot ? "tree-root" : ""}`}>
			<div className="tree-node-row">
				{hasChildren ? (
					<button className="tree-toggle" onClick={toggleExpand} aria-label={isExpanded ? "Collapse" : "Expand"}>
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
					<span className="tree-spacer" />
				)}
				<button className="tree-node-label" onClick={() => onFocusNode?.(node.id)} title={`Focus on ${node.id}`}>
					<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
						<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
						<polyline points="14 2 14 8 20 8" />
					</svg>
					{node.id}
				</button>
				{hasChildren && <span className="tree-count">({node.children.length})</span>}
			</div>
			{hasChildren && isExpanded && (
				<div className="tree-children">
					{node.children.map((child) => (
						<TreeNode key={child.id} node={child} onFocusNode={onFocusNode} />
					))}
				</div>
			)}
		</div>
	);
}

export function DependencyTree({ rootNodeId, edges, direction, maxDepth = 4, onFocusNode }: DependencyTreeProps) {
	const tree = useMemo(() => buildTree(rootNodeId, edges, direction, maxDepth), [rootNodeId, edges, direction, maxDepth]);

	// Don't render if root has no children
	if (tree.children.length === 0) {
		return null;
	}

	const title = direction === "dependents" ? "Dependents Tree" : "Dependencies Tree";
	const description = direction === "dependents" ? "Files that depend on this file (up to 4 levels)" : "Files this file depends on (up to 4 levels)";

	return (
		<div className="detail-group">
			<label>
				{title}
				<span className="label-count">{tree.children.length}</span>
			</label>
			<p className="tree-description">{description}</p>
			<div className="dependency-tree">
				{tree.children.map((child) => (
					<TreeNode key={child.id} node={child} onFocusNode={onFocusNode} />
				))}
			</div>
		</div>
	);
}
