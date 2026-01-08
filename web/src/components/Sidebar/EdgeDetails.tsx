import type { OutputEdge } from "../../types/sass-dep";
import { EDGE_COLORS } from "../Graph/styles";

interface EdgeDetailsProps {
	edge: OutputEdge;
}

export function EdgeDetails({ edge }: EdgeDetailsProps) {
	const directiveColor = EDGE_COLORS[edge.directive_type];

	return (
		<div className="sidebar-section">
			<h3>Edge Details</h3>

			<div className="detail-group">
				<label>From</label>
				<p className="file-id">{edge.from}</p>
			</div>

			<div className="detail-group">
				<label>To</label>
				<p className="file-id">{edge.to}</p>
			</div>

			<div className="detail-group">
				<label>Directive Type</label>
				<span className="directive-badge" style={{ backgroundColor: directiveColor, color: "white" }}>
					@{edge.directive_type}
				</span>
			</div>

			<div className="detail-group">
				<label>Source Location</label>
				<p className="location">
					Line {edge.location.line}, Column {edge.location.column}
				</p>
			</div>

			{edge.namespace && (
				<div className="detail-group">
					<label>Namespace</label>
					<code className="namespace">{edge.namespace}</code>
				</div>
			)}

			{edge.configured && (
				<div className="detail-group">
					<label>Configuration</label>
					<span className="configured-badge">with(...)</span>
				</div>
			)}
		</div>
	);
}
