import { type GraphIndex, edgePairKey } from "../../utils/graphIndex";
import type { PathHighlightResult } from "../../hooks/usePathHighlight";
import styles from "./Sidebar.module.scss";

/** Props for the PathDetails component. */
interface PathDetailsProps {
	/** First shift-clicked file */
	pathSource: string;
	/** Second shift-clicked file, or null while waiting for it */
	pathTarget: string | null;
	/** Result of the path search */
	pathHighlight: PathHighlightResult;
	/** Graph adjacency index (used to label each hop with its directive) */
	index: GraphIndex;
	/** Called when a file is clicked */
	onFocusNode?: (nodeId: string) => void;
	/** Clears both path endpoints */
	onClear: () => void;
}

/** Props for the FileButton component. */
interface FileButtonProps {
	/** File ID */
	fileId: string;
	/** Called when the file is clicked */
	onFocusNode?: (nodeId: string) => void;
}

/**
 * Inline clickable file name that focuses the file in the graph.
 * @param props - Component props
 * @returns Button element
 */
function FileButton({ fileId, onFocusNode }: FileButtonProps) {
	return (
		<button type="button" className={styles["inline-file"]} onClick={() => onFocusNode?.(fileId)} title={`Focus on ${fileId}`}>
			{fileId}
		</button>
	);
}

/**
 * Sidebar section describing the dependency path between two shift-clicked files:
 * its direction and the ordered list of files along it, or a message when there is none.
 * @param props - Component props
 * @returns Path section
 */
export function PathDetails({ pathSource, pathTarget, pathHighlight, index, onFocusNode, onClear }: PathDetailsProps) {
	const { path } = pathHighlight;

	let summary: React.ReactNode;
	if (!pathTarget) {
		summary = (
			<p className={styles["path-summary"]}>
				Start: <FileButton fileId={pathSource} onFocusNode={onFocusNode} />. Shift-click another file to find the dependency path.
			</p>
		);
	} else if (!path) {
		summary = (
			<p className={`${styles["path-summary"]} ${styles["path-none"]}`}>
				No dependency path between <FileButton fileId={pathSource} onFocusNode={onFocusNode} /> and <FileButton fileId={pathTarget} onFocusNode={onFocusNode} />.
			</p>
		);
	} else {
		const dependent = path.direction === "forward" ? pathSource : pathTarget;
		const dependency = path.direction === "forward" ? pathTarget : pathSource;
		const hops = path.nodes.length - 1;
		summary = (
			<p className={styles["path-summary"]}>
				<FileButton fileId={dependent} onFocusNode={onFocusNode} /> depends on <FileButton fileId={dependency} onFocusNode={onFocusNode} />{" "}
				{hops === 1 ? "directly" : `via ${hops - 1} intermediate file${hops - 1 === 1 ? "" : "s"}`}
				{path.direction === "reverse" && <span className={styles["path-direction"]}> (reverse of selection order)</span>}.
			</p>
		);
	}

	return (
		<div className={`${styles["section"]} ${styles["path-section"]}`}>
			<div className={styles["section-header"]}>
				<h3>Dependency Path</h3>
				<button type="button" className={styles["focus-button"]} onClick={onClear} title="Clear path (Esc)">
					Clear
				</button>
			</div>

			{summary}

			{path && (
				<ol className={styles["path-steps"]}>
					{path.nodes.map((fileId, i) => {
						const next = path.nodes[i + 1];
						const edge = next === undefined ? undefined : index.edgeByKey.get(edgePairKey(fileId, next));
						return (
							<li key={fileId} className={styles["path-step"]}>
								<button type="button" className={styles["file-link"]} onClick={() => onFocusNode?.(fileId)} title={`Focus on ${fileId}`}>
									<span className={styles["path-step-index"]}>{i + 1}</span>
									{fileId}
								</button>
								{edge && (
									<span className={styles["path-hop"]}>
										↓ @{edge.directive_type} · line {edge.location.line}
									</span>
								)}
							</li>
						);
					})}
				</ol>
			)}
		</div>
	);
}
