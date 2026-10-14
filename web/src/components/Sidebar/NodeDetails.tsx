import { useMemo } from "react";

import { DependencyTree } from "./DependencyTree";
import { ShowMoreButton } from "./ShowMoreButton";
import { useShowMore } from "./useShowMore";
import { type GraphIndex, getNeighborIds } from "../../utils/graphIndex";
import type { NodeFlag, OutputNode } from "../../types/sass-dep";
import styles from "./Sidebar.module.scss";

/** Number of files rendered per page in the dependents/dependencies lists. */
const FILE_LIST_PAGE_SIZE = 100;

/** Props for the NodeDetails component. */
interface NodeDetailsProps {
	/** Selected file ID */
	nodeId: string;
	/** Selected file data */
	node: OutputNode;
	/** Graph adjacency index */
	index: GraphIndex;
	/** Total number of files in the project (used for impact thresholds) */
	totalFiles: number;
	/** Called when a file is clicked */
	onFocusNode?: (nodeId: string) => void;
}

/** A single recommendation shown for the selected file. */
interface Recommendation {
	/** Severity / tone of the recommendation */
	type: "warning" | "info" | "success";
	/** Message text */
	message: string;
}

/** Maps flag names to CSS module class names */
const FLAG_CLASS_MAP: Readonly<Record<NodeFlag, string>> = {
	entry_point: "entry-point",
	in_cycle: "in-cycle",
	orphan: "orphan",
	leaf: "leaf",
	high_fan_in: "high-fan-in",
	high_fan_out: "high-fan-out",
};

/**
 * Checks whether a file's transitive dependents count is high enough to warrant a warning:
 * more than 20 files, or more than 25% of the project (when at least 5 files are affected).
 * @param transitiveDependents - Number of files that transitively depend on the file
 * @param totalFiles - Total number of files in the project
 * @returns True when the impact is high
 */
function isHighImpact(transitiveDependents: number, totalFiles: number): boolean {
	return transitiveDependents > 20 || (transitiveDependents >= 5 && transitiveDependents > totalFiles * 0.25);
}

/**
 * Computes what share of the project a count represents.
 * @param count - Number of files
 * @param totalFiles - Total number of files in the project
 * @returns Rounded percentage (0 when the project is empty)
 */
function percentOf(count: number, totalFiles: number): number {
	return totalFiles > 0 ? Math.round((count / totalFiles) * 100) : 0;
}

/**
 * Generates recommendations based on node metrics and flags.
 * @param node - The node to analyze
 * @param flags - Node flags
 * @param totalFiles - Total number of files in the project
 * @returns Array of recommendations
 */
function getRecommendations(node: OutputNode, flags: NodeFlag[], totalFiles: number): Recommendation[] {
	const recommendations: Recommendation[] = [];
	const { fan_in, fan_out, transitive_deps, transitive_dependents } = node.metrics;

	if (flags.includes("in_cycle")) {
		recommendations.push({
			type: "warning",
			message: "This file is part of a circular dependency. Consider restructuring to break the cycle.",
		});
	}

	if (flags.includes("orphan")) {
		recommendations.push({
			type: "warning",
			message: "This file is not imported anywhere. Consider removing it if unused, or adding it to an entry point.",
		});
	}

	if (isHighImpact(transitive_dependents, totalFiles)) {
		recommendations.push({
			type: "warning",
			message: `High impact: ${transitive_dependents} files (${percentOf(transitive_dependents, totalFiles)}% of the project) depend on this file directly or indirectly. Keep its public API stable and review changes carefully.`,
		});
	}

	if (flags.includes("high_fan_in")) {
		recommendations.push({
			type: "info",
			message: `High fan-in (${fan_in} dependents). Changes to this file will affect many others. Test thoroughly before modifying.`,
		});
	}

	if (flags.includes("high_fan_out")) {
		recommendations.push({
			type: "info",
			message: `High fan-out (${fan_out} imports). This file may be doing too much. Consider splitting into smaller modules.`,
		});
	}

	if (fan_in === 0 && fan_out === 0 && !flags.includes("orphan") && !flags.includes("entry_point")) {
		recommendations.push({
			type: "warning",
			message: "This file has no connections. It may be dead code.",
		});
	}

	if (transitive_deps > 20) {
		recommendations.push({
			type: "info",
			message: `Large dependency tree (${transitive_deps} transitive deps). This may impact build times and bundle size.`,
		});
	}

	if (fan_in > 0 && fan_out === 0 && !flags.includes("leaf")) {
		recommendations.push({
			type: "success",
			message: "Good leaf node - imported by others but has no dependencies. Easy to maintain.",
		});
	}

	if (flags.includes("entry_point") && fan_out > 10) {
		recommendations.push({
			type: "info",
			message: "Entry point with many direct imports. Consider using barrel files to organize imports.",
		});
	}

	if (recommendations.length === 0) {
		recommendations.push({
			type: "success",
			message: "No issues detected. This file has a healthy dependency structure.",
		});
	}

	return recommendations;
}

/**
 * Formats depth value for display.
 * @param depth - The depth value, or null when the file is unreachable from any entry point
 * @returns Formatted string
 */
function formatDepth(depth: number | null): string {
	return depth === null ? "— (unreachable)" : depth.toString();
}

/** Props for the FileList component. */
interface FileListProps {
	/** File IDs to list */
	fileIds: string[];
	/** Direction of the relationship, used for the icon */
	direction: "dependents" | "dependencies";
	/** Message shown when the list is empty */
	emptyMessage: string;
	/** Called when a file is clicked */
	onFocusNode?: (nodeId: string) => void;
}

/**
 * Capped list of clickable files with a "show more" affordance.
 * @param props - Component props
 * @returns File list, or an empty-state message
 */
function FileList({ fileIds, direction, emptyMessage, onFocusNode }: FileListProps) {
	const { limit, remaining, showMore } = useShowMore(fileIds.length, FILE_LIST_PAGE_SIZE);

	if (fileIds.length === 0) {
		return <p className={styles["no-items"]}>{emptyMessage}</p>;
	}

	return (
		<ul className={styles["file-list"]}>
			{fileIds.slice(0, limit).map((fileId) => (
				<li key={fileId} className={styles["file-list-item"]}>
					<button className={styles["file-link"]} onClick={() => onFocusNode?.(fileId)} title={`Focus on ${fileId}`}>
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<path d={direction === "dependents" ? "M12 19V5M5 12l7-7 7 7" : "M12 5v14M5 12l7 7 7-7"} />
						</svg>
						{fileId}
					</button>
				</li>
			))}
			{remaining > 0 && (
				<li className={styles["file-list-item"]}>
					<ShowMoreButton remaining={remaining} pageSize={FILE_LIST_PAGE_SIZE} onClick={showMore} />
				</li>
			)}
		</ul>
	);
}

/** Props for the RecommendationIcon component. */
interface RecommendationIconProps {
	/** Recommendation tone */
	type: Recommendation["type"];
}

/**
 * Icon matching a recommendation's tone.
 * @param props - Component props
 * @returns SVG icon
 */
function RecommendationIcon({ type }: RecommendationIconProps) {
	if (type === "warning") {
		return (
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
				<path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
				<line x1="12" y1="9" x2="12" y2="13" />
				<line x1="12" y1="17" x2="12.01" y2="17" />
			</svg>
		);
	}

	if (type === "info") {
		return (
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
				<circle cx="12" cy="12" r="10" />
				<line x1="12" y1="16" x2="12" y2="12" />
				<line x1="12" y1="8" x2="12.01" y2="8" />
			</svg>
		);
	}

	return (
		<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
			<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
			<polyline points="22 4 12 14.01 9 11.01" />
		</svg>
	);
}

/** CSS module class for each recommendation tone. */
const RECOMMENDATION_CLASS: Readonly<Record<Recommendation["type"], string>> = {
	warning: "recommendation-warning",
	info: "recommendation-info",
	success: "recommendation-success",
};

/**
 * Node details panel showing file info, metrics, flags, recommendations, and relationships.
 * Relationships come from the prebuilt graph index (no edge scans).
 * @param props - Component props
 * @returns Node details view
 */
export function NodeDetails({ nodeId, node, index, totalFiles, onFocusNode }: NodeDetailsProps) {
	const recommendations = getRecommendations(node, node.flags, totalFiles);
	const dependents = useMemo(() => getNeighborIds(index, nodeId, "dependents"), [index, nodeId]);
	const dependencies = useMemo(() => getNeighborIds(index, nodeId, "dependencies"), [index, nodeId]);

	return (
		<div className={styles["section"]}>
			<div className={styles["section-header"]}>
				<h3>Node Details</h3>
				{onFocusNode && (
					<button className={styles["focus-button"]} onClick={() => onFocusNode(nodeId)} title="Focus on this node">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<circle cx="12" cy="12" r="10" />
							<circle cx="12" cy="12" r="3" />
						</svg>
						Focus
					</button>
				)}
			</div>

			<div className={styles["detail-group"]}>
				<label>File ID</label>
				<p className={styles["file-id"]}>{nodeId}</p>
			</div>

			<div className={styles["detail-group"]}>
				<label>Full Path</label>
				<p className={styles["full-path"]} title={node.path}>
					{node.path}
				</p>
			</div>

			<div className={styles["detail-group"]}>
				<label>Metrics</label>
				<dl className={styles["metrics-list"]}>
					<div className={styles["metric-item"]}>
						<dt>Fan In</dt>
						<dd>{node.metrics.fan_in}</dd>
					</div>
					<div className={styles["metric-item"]}>
						<dt>Fan Out</dt>
						<dd>{node.metrics.fan_out}</dd>
					</div>
					<div className={styles["metric-item"]}>
						<dt>Depth</dt>
						<dd className={node.metrics.depth === null ? styles["metric-muted"] : undefined}>{formatDepth(node.metrics.depth)}</dd>
					</div>
					<div className={styles["metric-item"]}>
						<dt>Transitive Deps</dt>
						<dd>{node.metrics.transitive_deps}</dd>
					</div>
					<div className={`${styles["metric-item"]} ${styles["metric-item-wide"]}`} title="Number of files that depend on this file directly or indirectly">
						<dt>Impact (transitive dependents)</dt>
						<dd>
							{node.metrics.transitive_dependents}
							<span className={styles["metric-suffix"]}>{percentOf(node.metrics.transitive_dependents, totalFiles)}% of files</span>
						</dd>
					</div>
				</dl>
			</div>

			<div className={styles["detail-group"]}>
				<label>Flags</label>
				{node.flags.length > 0 ? (
					<ul className={styles["flags-list"]}>
						{node.flags.map((flag) => (
							<li key={flag} className={`${styles["flag-badge"]} ${styles[FLAG_CLASS_MAP[flag]]}`}>
								{flag.replace(/_/g, " ")}
							</li>
						))}
					</ul>
				) : (
					<p className={styles["no-flags"]}>No flags</p>
				)}
			</div>

			<div className={styles["detail-group"]}>
				<label>Recommendations</label>
				<ul className={styles["recommendations-list"]}>
					{recommendations.map((rec, idx) => (
						<li key={idx} className={`${styles["recommendation"]} ${styles[RECOMMENDATION_CLASS[rec.type]]}`}>
							<span className={styles["recommendation-icon"]}>
								<RecommendationIcon type={rec.type} />
							</span>
							<span className={styles["recommendation-text"]}>{rec.message}</span>
						</li>
					))}
				</ul>
			</div>

			<div className={styles["detail-group"]}>
				<label>
					Dependents
					<span className={styles["label-count"]}>{dependents.length}</span>
				</label>
				<FileList fileIds={dependents} direction="dependents" emptyMessage="No files depend on this file" onFocusNode={onFocusNode} />
			</div>

			<div className={styles["detail-group"]}>
				<label>
					Dependencies
					<span className={styles["label-count"]}>{dependencies.length}</span>
				</label>
				<FileList fileIds={dependencies} direction="dependencies" emptyMessage="This file has no dependencies" onFocusNode={onFocusNode} />
			</div>

			<DependencyTree rootNodeId={nodeId} index={index} direction="dependents" onFocusNode={onFocusNode} />
			<DependencyTree rootNodeId={nodeId} index={index} direction="dependencies" onFocusNode={onFocusNode} />
		</div>
	);
}
