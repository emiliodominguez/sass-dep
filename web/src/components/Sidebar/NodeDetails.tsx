import type { OutputNode, NodeFlag } from "../../types/sass-dep";

interface NodeDetailsProps {
	nodeId: string;
	node: OutputNode;
	onFocusNode?: (nodeId: string) => void;
}

interface Recommendation {
	type: "warning" | "info" | "success";
	message: string;
}

function getRecommendations(node: OutputNode, flags: NodeFlag[]): Recommendation[] {
	const recommendations: Recommendation[] = [];
	const { fan_in, fan_out, transitive_deps } = node.metrics;

	// Flag-based recommendations
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

	// Metric-based recommendations
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

	// No issues found
	if (recommendations.length === 0) {
		recommendations.push({
			type: "success",
			message: "No issues detected. This file has a healthy dependency structure.",
		});
	}

	return recommendations;
}

export function NodeDetails({ nodeId, node, onFocusNode }: NodeDetailsProps) {
	const formatDepth = (depth: number) => {
		// MAX_SAFE_INTEGER indicates orphan
		return depth >= Number.MAX_SAFE_INTEGER - 1000 ? "N/A (orphan)" : depth.toString();
	};

	const recommendations = getRecommendations(node, node.flags);

	return (
		<div className="sidebar-section">
			<div className="section-header">
				<h3>Node Details</h3>
				{onFocusNode && (
					<button className="focus-button" onClick={() => onFocusNode(nodeId)} title="Focus on this node">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<circle cx="12" cy="12" r="10" />
							<circle cx="12" cy="12" r="3" />
						</svg>
						Focus
					</button>
				)}
			</div>

			<div className="detail-group">
				<label>File ID</label>
				<p className="file-id">{nodeId}</p>
			</div>

			<div className="detail-group">
				<label>Full Path</label>
				<p className="full-path" title={node.path}>
					{node.path}
				</p>
			</div>

			<div className="detail-group">
				<label>Metrics</label>
				<dl className="metrics-list">
					<div className="metric-item">
						<dt>Fan In</dt>
						<dd>{node.metrics.fan_in}</dd>
					</div>
					<div className="metric-item">
						<dt>Fan Out</dt>
						<dd>{node.metrics.fan_out}</dd>
					</div>
					<div className="metric-item">
						<dt>Depth</dt>
						<dd>{formatDepth(node.metrics.depth)}</dd>
					</div>
					<div className="metric-item">
						<dt>Transitive Deps</dt>
						<dd>{node.metrics.transitive_deps}</dd>
					</div>
				</dl>
			</div>

			<div className="detail-group">
				<label>Flags</label>
				{node.flags.length > 0 ? (
					<ul className="flags-list">
						{node.flags.map((flag) => (
							<li key={flag} className={`flag-badge flag-${flag}`}>
								{flag.replace(/_/g, " ")}
							</li>
						))}
					</ul>
				) : (
					<p className="no-flags">No flags</p>
				)}
			</div>

			<div className="detail-group">
				<label>Recommendations</label>
				<ul className="recommendations-list">
					{recommendations.map((rec, idx) => (
						<li key={idx} className={`recommendation recommendation-${rec.type}`}>
							<span className="recommendation-icon">
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
							<span className="recommendation-text">{rec.message}</span>
						</li>
					))}
				</ul>
			</div>
		</div>
	);
}
