import { memo } from "react";

import type { GroupNodeData } from "./utils";
import styles from "./GroupNode.module.scss";

interface GroupNodeProps {
	data: GroupNodeData;
}

/**
 * Group node component for folder grouping visualization.
 * @param props - Component props containing group data
 * @returns A labeled container representing a folder group
 */
function GroupNodeComponent({ data }: GroupNodeProps) {
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

export default memo(GroupNodeComponent);
