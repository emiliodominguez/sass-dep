import type { NodeFlag, OutputEdge, OutputNode } from "../../types/sass-dep";
import { DependencyTree } from "./DependencyTree";
import styles from "./Sidebar.module.scss";

interface NodeDetailsProps {
	nodeId: string;
	node: OutputNode;
	dependents: string[];
	dependencies: string[];
	edges: OutputEdge[];
	onFocusNode?: (nodeId: string) => void;
}

interface Recommendation {
	type: "warning" | "info" | "success";
	message: string;
}

/** Maps flag names to CSS module class names */
const FLAG_CLASS_MAP: Record<string, string> = {
	entry_point: "entry-point",
	in_cycle: "in-cycle",
	orphan: "orphan",
	leaf: "leaf",
	high_fan_in: "high-fan-in",
	high_fan_out: "high-fan-out",
};

/**
 * Generates recommendations based on node metrics and flags.
 * @param node - The node to analyze
 * @param flags - Node flags
 * @returns Array of recommendations
 */
function getRecommendations(node: OutputNode, flags: NodeFlag[]): Recommendation[] {
	const recommendations: Recommendation[] = [];
	const { fan_in, fan_out, transitive_deps } = node.metrics;

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
 * @param depth - The depth value
 * @returns Formatted string
 */
function formatDepth(depth: number): string {
	return depth >= Number.MAX_SAFE_INTEGER - 1000 ? "N/A (orphan)" : depth.toString();
}

/**
 * Node details panel showing file info, metrics, flags, and recommendations.
 * @param props - Component props
 * @returns Node details view
 */
export function NodeDetails({ nodeId, node, dependents, dependencies, edges, onFocusNode }: NodeDetailsProps) {
	const recommendations = getRecommendations(node, node.flags);

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
						<dd>{formatDepth(node.metrics.depth)}</dd>
					</div>
					<div className={styles["metric-item"]}>
						<dt>Transitive Deps</dt>
						<dd>{node.metrics.transitive_deps}</dd>
					</div>
				</dl>
			</div>

			<div className={styles["detail-group"]}>
				<label>Flags</label>
				{node.flags.length > 0 ? (
					<ul className={styles["flags-list"]}>
						{node.flags.map((flag) => {
							const flagClass = FLAG_CLASS_MAP[flag] ?? "";
							const classNames = [styles["flag-badge"], flagClass && styles[flagClass]].filter(Boolean).join(" ");
							return (
								<li key={flag} className={classNames}>
									{flag.replace(/_/g, " ")}
								</li>
							);
						})}
					</ul>
				) : (
					<p className={styles["no-flags"]}>No flags</p>
				)}
			</div>

			<div className={styles["detail-group"]}>
				<label>Recommendations</label>
				<ul className={styles["recommendations-list"]}>
					{recommendations.map((rec, idx) => {
						const typeClass =
							rec.type === "warning" ? styles["recommendation-warning"] : rec.type === "info" ? styles["recommendation-info"] : styles["recommendation-success"];
						return (
							<li key={idx} className={`${styles["recommendation"]} ${typeClass}`}>
								<span className={styles["recommendation-icon"]}>
									{rec.type === "warning" && (
										<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
											<path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
											<line x1="12" y1="9" x2="12" y2="13" />
											<line x1="12" y1="17" x2="12.01" y2="17" />
										</svg>
									)}
									{rec.type === "info" && (
										<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
											<circle cx="12" cy="12" r="10" />
											<line x1="12" y1="16" x2="12" y2="12" />
											<line x1="12" y1="8" x2="12.01" y2="8" />
										</svg>
									)}
									{rec.type === "success" && (
										<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
											<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
											<polyline points="22 4 12 14.01 9 11.01" />
										</svg>
									)}
								</span>
								<span className={styles["recommendation-text"]}>{rec.message}</span>
							</li>
						);
					})}
				</ul>
			</div>

			<div className={styles["detail-group"]}>
				<label>
					Dependents
					<span className={styles["label-count"]}>{dependents.length}</span>
				</label>
				{dependents.length > 0 ? (
					<ul className={styles["file-list"]}>
						{dependents.map((fileId) => (
							<li key={fileId} className={styles["file-list-item"]}>
								<button className={styles["file-link"]} onClick={() => onFocusNode?.(fileId)} title={`Focus on ${fileId}`}>
									<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
										<path d="M12 19V5M5 12l7-7 7 7" />
									</svg>
									{fileId}
								</button>
							</li>
						))}
					</ul>
				) : (
					<p className={styles["no-items"]}>No files depend on this file</p>
				)}
			</div>

			<div className={styles["detail-group"]}>
				<label>
					Dependencies
					<span className={styles["label-count"]}>{dependencies.length}</span>
				</label>
				{dependencies.length > 0 ? (
					<ul className={styles["file-list"]}>
						{dependencies.map((fileId) => (
							<li key={fileId} className={styles["file-list-item"]}>
								<button className={styles["file-link"]} onClick={() => onFocusNode?.(fileId)} title={`Focus on ${fileId}`}>
									<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
										<path d="M12 5v14M5 12l7 7 7-7" />
									</svg>
									{fileId}
								</button>
							</li>
						))}
					</ul>
				) : (
					<p className={styles["no-items"]}>This file has no dependencies</p>
				)}
			</div>

			<DependencyTree rootNodeId={nodeId} edges={edges} direction="dependents" onFocusNode={onFocusNode} />
			<DependencyTree rootNodeId={nodeId} edges={edges} direction="dependencies" onFocusNode={onFocusNode} />
		</div>
	);
}
