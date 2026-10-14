import { memo } from "react";

import type { NodeProps } from "@xyflow/react";

import type { GroupFlowNode } from "./utils";
import styles from "./GroupNode.module.scss";

/**
 * Group node component for folder grouping visualization.
 * @param props - React Flow node props for a group node
 * @returns A labeled container representing a folder group
 */
function GroupNodeComponent({ data }: NodeProps<GroupFlowNode>) {
	const folderPath = data.folderPath || "/";
	const fileLabel = data.fileCount === 1 ? "file" : "files";

	return (
		<div className={styles["group-node"]}>
			<div className={styles["label"]} title={folderPath}>
				<span className={styles["path"]}>{folderPath}</span>
				<span className={styles["count"]}>
					{data.fileCount} {fileLabel}
				</span>
			</div>
		</div>
	);
}

/** Memoized group node. */
const GroupNode = memo(GroupNodeComponent);

export default GroupNode;
