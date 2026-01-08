import type { OutputNode, OutputEdge, Statistics } from "../../types/sass-dep";
import { NodeDetails } from "./NodeDetails";
import { EdgeDetails } from "./EdgeDetails";
import "./Sidebar.css";

interface SidebarProps {
	selectedNode: { id: string; node: OutputNode } | null;
	selectedEdge: OutputEdge | null;
	statistics: Statistics;
	onFocusNode?: (nodeId: string) => void;
}

export function Sidebar({ selectedNode, selectedEdge, statistics, onFocusNode }: SidebarProps) {
	return (
		<div className="sidebar">
			<div className="sidebar-header">
				<h2>sass-dep</h2>
				<span className="subtitle">Dependency Graph</span>
			</div>

			<div className="sidebar-content">
				{selectedNode ? (
					<NodeDetails nodeId={selectedNode.id} node={selectedNode.node} onFocusNode={onFocusNode} />
				) : selectedEdge ? (
					<EdgeDetails edge={selectedEdge} />
				) : (
					<div className="sidebar-section">
						<h3>Statistics</h3>
						<dl className="stats-list">
							<div className="stat-item">
								<dt>Total Files</dt>
								<dd>{statistics.total_files}</dd>
							</div>
							<div className="stat-item">
								<dt>Dependencies</dt>
								<dd>{statistics.total_dependencies}</dd>
							</div>
							<div className="stat-item">
								<dt>Entry Points</dt>
								<dd>{statistics.entry_points}</dd>
							</div>
							<div className="stat-item">
								<dt>Orphan Files</dt>
								<dd>{statistics.orphan_files}</dd>
							</div>
							<div className="stat-item">
								<dt>Leaf Files</dt>
								<dd>{statistics.leaf_files}</dd>
							</div>
							<div className="stat-item">
								<dt>Max Depth</dt>
								<dd>{statistics.max_depth}</dd>
							</div>
							<div className="stat-item">
								<dt>Max Fan-In</dt>
								<dd>{statistics.max_fan_in}</dd>
							</div>
							<div className="stat-item">
								<dt>Max Fan-Out</dt>
								<dd>{statistics.max_fan_out}</dd>
							</div>
						</dl>
						<p className="sidebar-hint">Click on a node or edge to see details</p>
					</div>
				)}
			</div>
		</div>
	);
}
