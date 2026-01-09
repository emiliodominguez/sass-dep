import type { OutputEdge } from "../../types/sass-dep";
import { EDGE_COLORS } from "../Graph/styles";
import styles from "./Sidebar.module.scss";

interface EdgeDetailsProps {
	edge: OutputEdge;
}

/**
 * Edge details panel showing connection info and directive type.
 * @param props - Component props
 * @returns Edge details view
 */
export function EdgeDetails({ edge }: EdgeDetailsProps) {
	const directiveColor = EDGE_COLORS[edge.directive_type];

	return (
		<div className={styles["section"]}>
			<h3>Edge Details</h3>

			<div className={styles["detail-group"]}>
				<label>From</label>
				<p className={styles["file-id"]}>{edge.from}</p>
			</div>

			<div className={styles["detail-group"]}>
				<label>To</label>
				<p className={styles["file-id"]}>{edge.to}</p>
			</div>

			<div className={styles["detail-group"]}>
				<label>Directive Type</label>
				<span className={styles["directive-badge"]} style={{ backgroundColor: directiveColor, color: "white" }}>
					@{edge.directive_type}
				</span>
			</div>

			<div className={styles["detail-group"]}>
				<label>Source Location</label>
				<p className={styles["location"]}>
					Line {edge.location.line}, Column {edge.location.column}
				</p>
			</div>

			{edge.namespace && (
				<div className={styles["detail-group"]}>
					<label>Namespace</label>
					<code className={styles["namespace"]}>{edge.namespace}</code>
				</div>
			)}

			{edge.configured && (
				<div className={styles["detail-group"]}>
					<label>Configuration</label>
					<span className={styles["configured-badge"]}>with(...)</span>
				</div>
			)}
		</div>
	);
}
