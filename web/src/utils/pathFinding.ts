import type { OutputEdge } from "../types/sass-dep";

/**
 * A dependency path between two files.
 * `nodes` is always ordered along edge direction: `nodes[i]` loads `nodes[i + 1]`.
 */
export interface DependencyPath {
	/**
	 * `"forward"` when the first selected file depends on the second (source → … → target),
	 * `"reverse"` when the second depends on the first (target → … → source).
	 */
	direction: "forward" | "reverse";
	/** Files along the path, in edge direction (dependent first, dependency last) */
	nodes: string[];
}

/**
 * Finds the shortest directed path from one file to another with a breadth-first search
 * following edge direction only. Uses a read pointer instead of `Array.shift` so it stays O(N + E).
 * @param from - Start file ID
 * @param to - Destination file ID
 * @param outgoing - Outgoing adjacency map (file ID → edges leaving it)
 * @returns File IDs from `from` to `to` inclusive, or null if `to` is unreachable
 */
export function findDirectedPath(from: string, to: string, outgoing: ReadonlyMap<string, readonly OutputEdge[]>): string[] | null {
	if (from === to) return [from];

	const parent = new Map<string, string | null>([[from, null]]);
	const queue: string[] = [from];

	for (let head = 0; head < queue.length; head++) {
		const current = queue[head]!;
		const edges = outgoing.get(current);
		if (!edges) continue;

		for (const edge of edges) {
			const next = edge.to;
			if (parent.has(next)) continue;

			parent.set(next, current);
			if (next === to) {
				const path: string[] = [];
				for (let node: string | null = to; node !== null; node = parent.get(node) ?? null) {
					path.push(node);
				}
				return path.reverse();
			}
			queue.push(next);
		}
	}

	return null;
}

/**
 * Finds a dependency path between two files, trying `source → target` first and then `target → source`.
 * @param source - First selected file ID
 * @param target - Second selected file ID
 * @param outgoing - Outgoing adjacency map
 * @returns The path and its direction, or null when neither file (transitively) depends on the other
 */
export function findDependencyPath(source: string, target: string, outgoing: ReadonlyMap<string, readonly OutputEdge[]>): DependencyPath | null {
	const forward = findDirectedPath(source, target, outgoing);
	if (forward) return { direction: "forward", nodes: forward };

	const reverse = findDirectedPath(target, source, outgoing);
	if (reverse) return { direction: "reverse", nodes: reverse };

	return null;
}
